/**
 * GUI 常驻桥：接收 CLI IPC / 后端 emit，按 request_id 路由到 genJobs
 * 代码路径: kk_novel_ai/src/services/guiBridge.js
 */
import { invoke, listen } from "./tauri.js";
import { appState } from "../stores/appState.js";
import { openProject, loadChapter, saveChapter, loadGenLogs, applyBranchDoc } from "./projectClient.js";
import { blocksFromContent } from "../utils/genBlock.js";
import { migrateBlocksToBranchDoc, parseSidecarToBranchDoc, isBranchDoc } from "../utils/branchModel.js";
import { looksIncomplete } from "../utils/previewText.js";
import { calcGenProgressPct, estimateTargetChars } from "../utils/genProgress.js";
import {
  deepseekGeneratingStatusSuffix,
  notifyDeepseekPeakIfNeeded,
} from "../utils/deepseekPricing.js";
import { toastWarning } from "./toast.js";
import { isCancelledMsg, t } from "../i18n/index.js";
import {
  activeJobCount,
  appendJobDelta,
  bindJobRequestId,
  createGenJob,
  discardJob,
  findJobByRequestId,
  finishJobError,
  finishJobOk,
  genJobState,
  hasUserFacingStorySyncJob,
  refreshLegacyFromJobs,
  syncGeneratingFromJobs,
} from "../stores/genJobs.js";

function applyChapterPayload(content, blocksSidecar) {
  if (isBranchDoc(blocksSidecar) || (blocksSidecar && typeof blocksSidecar === "object" && !Array.isArray(blocksSidecar) && blocksSidecar.nodes)) {
    applyBranchDoc(parseSidecarToBranchDoc(blocksSidecar));
    return;
  }
  if (Array.isArray(blocksSidecar) && blocksSidecar.length) {
    applyBranchDoc(migrateBlocksToBranchDoc(blocksSidecar));
    return;
  }
  const blocks = blocksFromContent(content || "", blocksSidecar);
  applyBranchDoc(migrateBlocksToBranchDoc(blocks));
}

let started = false;
let activeRequestId = "";
let progressHideTimer = null;

function clearProgressHideTimer() {
  if (progressHideTimer) {
    clearTimeout(progressHideTimer);
    progressHideTimer = null;
  }
}

function resetGenProgress(statusMsg) {
  clearProgressHideTimer();
  const mt = appState.settings && appState.settings.max_tokens;
  const tc = appState.settings && appState.settings.writing_target_chars;
  let target = estimateTargetChars(mt, tc);
  if (appState.draftTask === "same_slot_variant") {
    const base = Math.max(200, Number(tc) || target || 1800);
    appState.genTargetChars = base;
  } else {
    appState.genTargetChars = target;
  }
  appState.genStreamChars = 0;
  appState.genProgressPct = calcGenProgressPct(0, appState.genTargetChars, true, false);
  if (statusMsg) appState.statusMessage = statusMsg;
}

function finishGenProgress() {
  if (activeJobCount() > 0) return;
  appState.genProgressPct = 100;
  clearProgressHideTimer();
  progressHideTimer = setTimeout(() => {
    if (!appState.generating) {
      appState.genProgressPct = 0;
      appState.genStreamChars = 0;
    }
    progressHideTimer = null;
  }, 900);
}

import { isBackgroundAnalysisTask, shouldMuteLlmUi } from "../utils/writingTasks.js";

function muteLlmStream(task) {
  return shouldMuteLlmUi(task, hasUserFacingStorySyncJob());
}

function resolveJob(requestId) {
  if (!requestId) return null;
  return findJobByRequestId(requestId) || bindJobRequestId(requestId);
}

/**
 * 在 App 挂载时调用一次；面板 runWriting 也可复用同一套预览状态。
 */
export async function startGuiBridge() {
  if (started) return;
  started = true;

  await listen("cli-writing-start", async (event) => {
    const p = event.payload || {};
    const task = p.task || "continue";
    const metaOnly = isBackgroundAnalysisTask(task);
    appState.previewText = "";
    appState.previewRawText = "";
    appState.lastTruncated = false;
    appState.lastIncomplete = false;
    appState.draftPlacement = metaOnly ? "" : "editor";
    appState.draftTask = task;
    appState.draftSelection = "";
    appState.draftRewriteBlockKey = "";
    appState.draftAnchorBlockKey = "";
    appState.draftBranchMode = "";
    appState.draftBranchNodeId = "";
    appState.draftForkFromVariantId = "";
    if (metaOnly) {
      appState.statusMessage =
        task === "outline_to_beats" || task === "split_beats"
          ? t("gui.splitBeats")
          : task === "outline_to_chapters" || task === "split_chapters"
            ? t("gui.splitChapters")
            : task === "outline_to_mindmap" || task === "mindmap_outline"
              ? t("gui.mindmap")
              : task === "chapter_summary" || task === "summarize"
                ? t("gui.chapterSummary")
              : task === "story_sync" || task === "sync_story"
                ? t("gui.syncStory")
              : task === "beats_to_storyboard" || task === "storyboard_from_beats"
                ? t("gui.storyboard")
              : task === "content_to_image_prompt" || task === "image_prompt"
                ? t("gui.imagePrompt")
            : t("gui.background");
    } else {
      resetGenProgress(t("gui.cliGenerating"));
      try {
        createGenJob({ label: task });
      } catch (e) {
        appState.statusMessage = String(e.message || e);
      }
    }
    try {
      if (appState.dirty) await saveChapter();
      const root = p.project_root || "";
      const chapterId = p.chapter_id || "";
      if (root && root !== appState.projectRoot) {
        await openProject(root);
      }
      if (chapterId && chapterId !== appState.chapterId) {
        await loadChapter(chapterId);
      } else if (chapterId && root === appState.projectRoot && !appState.chapterContent) {
        await loadChapter(chapterId);
      }
      appState.activeNav = "editor";
    } catch (e) {
      appState.statusMessage = String(e.message || e);
    } finally {
      if (p.prepare_id) {
        try {
          await invoke("ipc_prepare_ack", { prepareId: p.prepare_id });
        } catch {
          /* ignore */
        }
      }
    }
  });

  await listen("project-focus", async (event) => {
    const p = event.payload || {};
    try {
      if (appState.dirty) await saveChapter();
      if (p.root) {
        if (p.root !== appState.projectRoot) await openProject(p.root);
        if (p.chapter_id) await loadChapter(p.chapter_id);
        appState.activeNav = "editor";
      }
    } catch (e) {
      appState.statusMessage = String(e.message || e);
    }
  });

  await listen("llm-start", (event) => {
    const p = event.payload || {};
    if (p.request_id) {
      activeRequestId = p.request_id;
      appState.lastRequestId = p.request_id;
    }
    if (muteLlmStream(p.task)) return;
    if (!p.request_id) return;
    const notice = p.deepseek_peak_notice || "";
    if (notice) {
      appState.deepseekPeakNow = true;
      appState.deepseekPeakNotice = notice;
      notifyDeepseekPeakIfNeeded(appState.settings || {}, { toastFn: toastWarning });
    }
    const job = bindJobRequestId(p.request_id);
    if (job && job.status === "pending") job.status = "streaming";
    syncGeneratingFromJobs();
    const peakSuffix = deepseekGeneratingStatusSuffix(appState.settings || {});
    if (peakSuffix && !muteLlmStream(p.task)) {
      appState.statusMessage = t("status.generatingPeak", { suffix: peakSuffix });
    }
  });

  await listen("llm-chunk", (event) => {
    const p = event.payload || {};
    if (muteLlmStream(p.task)) return;
    const rid = p.request_id || "";
    if (rid) {
      activeRequestId = rid;
      appState.lastRequestId = rid;
    }
    const job = resolveJob(rid);
    if (job) {
      if (p.delta) appendJobDelta(job, p.delta);
      return;
    }
    appState.generating = true;
    if (p.delta) {
      appState.previewText += p.delta;
      appState.previewRawText += p.delta;
      const n = (appState.previewRawText || appState.previewText || "").length;
      appState.genStreamChars = n;
      appState.genProgressPct = calcGenProgressPct(
        n,
        appState.genTargetChars,
        true,
        false
      );
      appState.statusMessage = t("status.generatingChars", {
        n,
        pct: appState.genProgressPct,
        suffix: deepseekGeneratingStatusSuffix(appState.settings || {}),
      });
    }
  });

  await listen("llm-done", (event) => {
    const p = event.payload || {};
    if (muteLlmStream(p.task)) {
      void loadGenLogs(50).catch(() => {});
      const rid = p.request_id || "";
      const bgJob = rid ? findJobByRequestId(rid) : null;
      if (bgJob) discardJob(bgJob);
      syncGeneratingFromJobs();
      if (p.task === "outline_to_beats" || p.task === "split_beats") {
        appState.statusMessage = t("gui.beatsDone");
      } else if (p.task === "outline_to_chapters" || p.task === "split_chapters") {
        appState.statusMessage = t("gui.chaptersDone");
      } else if (p.task === "outline_to_mindmap" || p.task === "mindmap_outline") {
        appState.statusMessage = t("gui.mindmapDone");
      } else if (p.task === "chapter_summary" || p.task === "summarize") {
        appState.statusMessage = t("gui.summaryDone");
      }
      return;
    }
    const rid = p.request_id || "";
    if (rid) {
      activeRequestId = rid;
      appState.lastRequestId = rid;
    }
    const job = resolveJob(rid);
    const streamedLen = job
      ? (job.previewRawText || job.previewText || "").length
      : (appState.previewRawText || appState.previewText || "").length;
    const raw =
      typeof p.raw_text === "string" && p.raw_text
        ? p.raw_text
        : typeof p.text === "string" && p.text
          ? p.text
          : job
            ? job.previewRawText || job.previewText || ""
            : appState.previewRawText || appState.previewText || "";

    if (job) {
      finishJobOk(job, {
        raw_text: raw,
        text: raw,
        truncated: !!p.truncated,
        model_used: p.model_used || "",
        usage: p.usage || null,
        log_id: p.log_id || "",
        cost_cny: typeof p.cost_cny === "number" ? p.cost_cny : 0,
        context_sources: p.context_sources || null,
      });
      job.lastIncomplete = looksIncomplete(job.previewText || "");
    } else {
      appState.previewRawText = raw || appState.previewRawText;
      appState.previewText = appState.previewRawText;
      appState.lastTruncated = !!p.truncated;
      appState.lastIncomplete = looksIncomplete(appState.previewText);
      appState.lastModelUsed = p.model_used || "";
      appState.lastUsage = p.usage || null;
      appState.lastLogId = p.log_id || "";
      appState.lastCostCny = typeof p.cost_cny === "number" ? p.cost_cny : 0;
      if (p.context_sources) appState.lastContextSources = p.context_sources;
      appState.generating = false;
    }

    const finalLen = job
      ? (job.previewText || "").length
      : (appState.previewText || "").length;
    const rawLen = finalLen || streamedLen;
    const model = p.model_used || "?";
    if (p.truncated) {
      appState.statusMessage = t("gui.doneRepeat", { model });
    } else if (!finalLen && rawLen > 0) {
      appState.statusMessage = t("gui.doneEmpty");
    } else if ((job && job.lastIncomplete) || (!job && appState.lastIncomplete)) {
      appState.statusMessage = t("gui.doneTrunc", { model });
    } else if (activeJobCount() > 0) {
      appState.statusMessage = t("gui.doneStill", { n: activeJobCount() });
    } else {
      appState.statusMessage = p.model_used
        ? t("gui.doneModel", { model: p.model_used })
        : t("gui.done");
    }

    syncGeneratingFromJobs();
    finishGenProgress();
    void loadGenLogs(50).catch(() => {});
    if (job && job.draftPlacement === "editor" && !job.accepted) {
      void import("./draftAccept.js").then((m) => m.autoAcceptJobIfNeeded(job));
    } else if (!job && appState.draftPlacement === "editor") {
      void import("./draftAccept.js").then((m) => m.autoAcceptDraftIfNeeded());
    }
  });

  await listen("llm-error", (event) => {
    const p = event.payload || {};
    if (muteLlmStream(p.task)) {
      return;
    }
    const rid = p.request_id || "";
    const job = rid ? findJobByRequestId(rid) || bindJobRequestId(rid) : null;
    const err = p.error || t("status.failed");
    const cancelled = isCancelledMsg(err);
    if (job) {
      finishJobError(job, err, cancelled);
      if (job.draftPlacement === "editor") {
        discardJob(job);
        refreshLegacyFromJobs();
      }
    } else {
      appState.generating = false;
      appState.genProgressPct = 0;
      appState.genStreamChars = 0;
      if (appState.draftPlacement === "editor") {
        void import("./draftAccept.js").then((m) => {
          if (typeof m.clearDraftPreview === "function") m.clearDraftPreview();
        });
      }
    }
    syncGeneratingFromJobs();
    appState.statusMessage = cancelled
      ? activeJobCount() > 0
        ? t("gui.cancelledOne", { n: activeJobCount() })
        : t("status.cancelled")
      : err;
  });

  await listen("chapter-external-update", (event) => {
    const p = event.payload || {};
    if (p.root && appState.projectRoot && p.root !== appState.projectRoot) return;

    const applyExternal = async () => {
      if (p.chapter_id && appState.chapterId && p.chapter_id !== appState.chapterId) {
        await loadChapter(p.chapter_id);
      }
      if (typeof p.content === "string") {
        applyChapterPayload(p.content, p.blocks);
        appState.dirty = !p.saved;
        appState.statusMessage = p.saved ? t("gui.cliSaved") : t("gui.cliUpdated");
      }
    };

    if (appState.dirty && typeof p.content === "string") {
      appState.externalConflict = {
        content: p.content,
        saved: !!p.saved,
        chapter_id: p.chapter_id || appState.chapterId,
        root: p.root || appState.projectRoot,
      };
      appState.statusMessage = t("gui.conflict");
      return;
    }

    void applyExternal();
  });
}

export function resolveExternalConflict(keepLocal) {
  const c = appState.externalConflict;
  if (!c) return;
  if (!keepLocal && typeof c.content === "string") {
    if (c.chapter_id && c.chapter_id !== appState.chapterId) {
      appState.chapterId = c.chapter_id;
    }
    appState.chapterContent = c.content;
    applyChapterPayload(c.content, c.blocks);
    appState.dirty = !c.saved;
    appState.statusMessage = t("gui.acceptedExternal");
  } else {
    appState.statusMessage = t("gui.keptLocal");
  }
  appState.externalConflict = null;
}

/** 兼容旧调用：不再清空其它路预览 */
export function beginLocalGeneration() {
  activeRequestId = "";
  resetGenProgress(t("status.generating"));
}

export function endLocalGeneration() {
  syncGeneratingFromJobs();
  finishGenProgress();
}

/** 当前进行中的写作 request_id（可能尚无） */
export function getActiveRequestId() {
  const active = genJobState.jobs.find(
    (j) => (j.status === "pending" || j.status === "streaming") && j.requestId
  );
  return active?.requestId || activeRequestId || appState.lastRequestId || "";
}
