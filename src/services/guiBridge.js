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

import { isBackgroundAnalysisTask } from "../utils/writingTasks.js";

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
          ? "正在拆分节拍…"
          : task === "outline_to_chapters" || task === "split_chapters"
            ? "正在拆成章节…"
            : task === "outline_to_mindmap" || task === "mindmap_outline"
              ? "正在整理思维导图…"
              : task === "chapter_summary" || task === "summarize"
                ? "正在生成章节总结…"
            : "后台处理中…";
    } else {
      resetGenProgress("CLI 生成中…");
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
    if (isBackgroundAnalysisTask(p.task)) return;
    if (!p.request_id) return;
    activeRequestId = p.request_id;
    appState.lastRequestId = p.request_id;
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
    if (peakSuffix && !isBackgroundAnalysisTask(p.task)) {
      appState.statusMessage = `生成中${peakSuffix}…`;
    }
  });

  await listen("llm-chunk", (event) => {
    const p = event.payload || {};
    if (isBackgroundAnalysisTask(p.task)) return;
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
      appState.statusMessage = `生成中… ${n} 字 · ${appState.genProgressPct}%${deepseekGeneratingStatusSuffix(appState.settings || {})}`;
    }
  });

  await listen("llm-done", (event) => {
    const p = event.payload || {};
    if (isBackgroundAnalysisTask(p.task)) {
      void loadGenLogs(50).catch(() => {});
      const rid = p.request_id || "";
      const bgJob = rid ? findJobByRequestId(rid) : null;
      if (bgJob) discardJob(bgJob);
      syncGeneratingFromJobs();
      if (p.task === "outline_to_beats" || p.task === "split_beats") {
        appState.statusMessage = "节拍拆分完成（结果未写入正文，请在总谱确认 beats）";
      } else if (p.task === "outline_to_chapters" || p.task === "split_chapters") {
        appState.statusMessage = "拆章完成（结果未写入正文，请在按纲生成面板确认）";
      } else if (p.task === "outline_to_mindmap" || p.task === "mindmap_outline") {
        appState.statusMessage = "思维导图已整理";
      } else if (p.task === "chapter_summary" || p.task === "summarize") {
        appState.statusMessage = "章节总结已写入记忆快照";
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
      appState.statusMessage = `生成完成（检测到疑似复读，已保留全文未截断；模型 ${model}）`;
    } else if (!finalLen && rawLen > 0) {
      appState.statusMessage = "生成完成，但未拿到正文。";
    } else if ((job && job.lastIncomplete) || (!job && appState.lastIncomplete)) {
      appState.statusMessage = `生成完成，但正文疑似半截（可提高 max_tokens 后重试；模型 ${model}）`;
    } else if (activeJobCount() > 0) {
      appState.statusMessage = `一路已完成，仍有 ${activeJobCount()} 路生成中`;
    } else {
      appState.statusMessage = `生成完成${p.model_used ? ` · ${p.model_used}` : ""}`;
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
    if (isBackgroundAnalysisTask(p.task)) {
      return;
    }
    const rid = p.request_id || "";
    const job = rid ? findJobByRequestId(rid) || bindJobRequestId(rid) : null;
    const err = p.error || "生成失败";
    const cancelled = /取消/.test(err);
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
        ? `已取消一路，仍有 ${activeJobCount()} 路`
        : "已取消生成"
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
        appState.statusMessage = p.saved ? "章节已由 CLI 写入" : "章节已更新（未保存）";
      }
    };

    if (appState.dirty && typeof p.content === "string") {
      appState.externalConflict = {
        content: p.content,
        saved: !!p.saved,
        chapter_id: p.chapter_id || appState.chapterId,
        root: p.root || appState.projectRoot,
      };
      appState.statusMessage = "外部写入与本地未保存编辑冲突，请选择";
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
    appState.statusMessage = "已接受外部覆盖";
  } else {
    appState.statusMessage = "已保留本地编辑";
  }
  appState.externalConflict = null;
}

/** 兼容旧调用：不再清空其它路预览 */
export function beginLocalGeneration() {
  activeRequestId = "";
  resetGenProgress("生成中…");
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
