/**
 * 全书大纲：保存、拆章、续拆、落盘建章、确认后自动按纲写
 * 代码路径: kk_novel_ai/src/services/bookOutlineQueue.js
 */
import { appState } from "../stores/appState.js";
import { aiPanelForm, noteBookOutlineSaved } from "../stores/aiPanelState.js";
import { isChapterBodyEmpty } from "../utils/chapterStatus.js";
import { runWriting } from "./llmClient.js";
import { withBranchContext } from "./draftAccept.js";
import {
  createChapter,
  getProject,
  loadChapter,
  saveProjectMeta,
  updateChapterMeta,
} from "./projectClient.js";
import { invoke } from "./tauri.js";
import { appConfirm } from "./confirmDialog.js";
import {
  cancelOutlineQueue,
  outlineQueueState,
  runOutlineQueue,
} from "./outlineQueue.js";
import {
  composeMustNot,
  isPlaceholderBookTitle,
  seedTitleFromOutline,
} from "../utils/outlineContinuity.js";
import {
  createGenJob,
  discardJob,
  visibleGenJobs,
} from "../stores/genJobs.js";
import { t, tLocale } from "../i18n/index.js";
import {
  describeSplitFailure,
  estimateSplitMaxTokens,
  parseOutlineToChapters,
} from "../utils/outlineChapters.js";

export { parseOutlineToChapters } from "../utils/outlineChapters.js";

function writingT(key, values) {
  return tLocale(appState.settings?.writing_locale || "zh-CN", key, values);
}

export { isChapterBodyEmpty };

/**
 * @param {string} [text]
 */
export async function saveBookOutline(text) {
  if (!appState.projectRoot || !appState.project) {
    throw new Error(t("project.needProject"));
  }
  const outline = String(text != null ? text : aiPanelForm.bookOutline || "").trim();
  const next = { ...appState.project, book_outline: outline };
  if (isPlaceholderBookTitle(next.title) && outline) {
    const seeded = seedTitleFromOutline(outline);
    if (seeded) next.title = seeded;
  }
  await saveProjectMeta(next);
  noteBookOutlineSaved(outline);
  appState.statusMessage =
    next.title && next.title !== appState.project.title
      ? t("bookQ.savedTitle", { title: next.title })
      : t("bookQ.saved");
  return outline;
}

/**
 * @param {object} chapter
 */
async function chapterIsReusableEmpty(chapter) {
  if (!chapter) return false;
  if (String(chapter.summary || "").trim()) return false;
  if (Array.isArray(chapter.beats) && chapter.beats.length > 0) return false;
  if (chapter.status === "done" || chapter.status === "outline_complete") return false;
  try {
    const r = await invoke("chapter_read", {
      root: appState.projectRoot,
      chapterId: chapter.id,
    });
    return isChapterBodyEmpty(r.content || "", chapter.title || "");
  } catch {
    return false;
  }
}

/**
 * 拆章用的「创作提示 / 全书大纲」：优先上方大纲框，其次底部指令
 * @param {{ bookOutline?: string, instruction?: string }} [opts]
 */
export function resolveBookOutlineSeed(opts = {}) {
  const fromOpt = String(opts.bookOutline ?? "").trim();
  if (fromOpt) return fromOpt;
  const fromPanel = String(aiPanelForm.bookOutline || "").trim();
  if (fromPanel) return fromPanel;
  return String(opts.instruction ?? aiPanelForm.instruction ?? "").trim();
}

/**
 * 拆章 / 写作 IPC 需要 chapter_id。目录被删空时：
 * - full：自动建空「第一章」占位，再拆纲覆盖/追加
 * - append：目录空则提示先按纲生成
 * @param {{ allowCreate?: boolean }} [opts]
 * @returns {Promise<string>}
 */
export async function ensureChapterContext(opts = {}) {
  if (!appState.projectRoot) {
    throw new Error(t("project.needProject"));
  }
  const allowCreate = opts.allowCreate !== false;
  const chapters = (appState.project && appState.project.chapters) || [];
  if (appState.chapterId && chapters.some((c) => c.id === appState.chapterId)) {
    return appState.chapterId;
  }
  if (chapters.length) {
    await loadChapter(chapters[0].id);
    return chapters[0].id;
  }
  if (!allowCreate) {
    throw new Error(t("bookQ.noChapters"));
  }
  const r = await createChapter(writingT("editor.chapterN", { n: 1 }), "", { load: true });
  const id = (r.chapter && r.chapter.id) || appState.chapterId;
  if (!id) throw new Error(t("bookQ.createFailed"));
  return id;
}

/**
 * @param {{ instruction?: string, bookOutline?: string, mode?: "full"|"append", skipSaveOutline?: boolean }} opts
 */
export async function runSplitChapters(opts = {}) {
  if (!appState.projectRoot) {
    throw new Error(t("project.needProject"));
  }
  const mode = opts.mode === "append" ? "append" : "full";
  await ensureChapterContext({ allowCreate: mode === "full" });
  if (outlineQueueState.running) {
    throw new Error(t("outlineQ.alreadyRunning"));
  }
  if (visibleGenJobs.value.length) {
    throw new Error(t("bookQ.waitDraft"));
  }

  if (mode === "full") {
    const outline = resolveBookOutlineSeed(opts);
    if (!outline) {
      throw new Error(t("bookQ.needPrompt"));
    }
    await saveBookOutline(outline);
  } else if (!opts.skipSaveOutline) {
    // 续拆也落盘当前全书大纲（可空，靠已有章上下文）
    const bo = resolveBookOutlineSeed(opts);
    if (bo) await saveBookOutline(bo);
  }

  const userInstr = String(opts.instruction ?? aiPanelForm.instruction ?? "").trim();
  const outlineSeed = resolveBookOutlineSeed(opts);
  const splitMaxTokens = estimateSplitMaxTokens(outlineSeed || userInstr);
  outlineQueueState.phase = "splitting_chapters";
  outlineQueueState.running = true;
  outlineQueueState.cancelled = false;
  outlineQueueState.error = "";
  appState.statusMessage =
    mode === "append" ? t("bookQ.appending") : t("bookQ.splitting");

  const splitJob = createGenJob({
    label: mode === "append" ? t("bookQ.labelAppend") : t("bookQ.labelSplit"),
  });
  splitJob.draftPlacement = "";
  let splitResult;
  try {
    splitResult = await runWriting(
      withBranchContext(
        {
          project_root: appState.projectRoot,
          chapter_id: appState.chapterId,
          task: "outline_to_chapters",
          instruction: userInstr,
          selection: "",
          split_mode: mode,
          max_tokens: splitMaxTokens,
        },
        "continue",
        ""
      ),
      { job: splitJob, label: splitJob.label }
    );
  } catch (e) {
    outlineQueueState.running = false;
    outlineQueueState.phase = "error";
    outlineQueueState.error = String(e.message || e);
    discardJob(splitJob);
    throw e;
  } finally {
    discardJob(splitJob);
  }

  if (outlineQueueState.cancelled) {
    outlineQueueState.running = false;
    outlineQueueState.phase = "cancelled";
    appState.statusMessage = t("bookQ.cancelled");
    throw new Error(t("bookQ.cancelled"));
  }

  const planText =
    (splitResult && (splitResult.raw_text || splitResult.text)) ||
    splitJob.previewRawText ||
    splitJob.previewText ||
    "";
  const parsed = parseOutlineToChapters(planText);
  const { chapters, reason } = parsed;
  outlineQueueState.running = false;
  outlineQueueState.phase = "";

  if (!chapters.length) {
    throw new Error(describeSplitFailure(mode, planText, parsed));
  }

  aiPanelForm.chapterPlan = chapters;
  appState.statusMessage = reason
    ? t("bookQ.splitDoneReason", { n: chapters.length, reason })
    : t("bookQ.splitDone", { n: chapters.length });
  return { chapters, reason, mode };
}

/**
 * 若模型从「第2章」起跳，把标题纠正为从第1章连续编号（仅当整队像从2起跳时）
 * @param {Array<{title:string, summary:string, must_do?:string, selected?:boolean}>} rows
 */
export function normalizeChapterPlanTitles(rows) {
  const list = Array.isArray(rows) ? rows : [];
  if (!list.length) return list;
  const first = String(list[0].title || "").trim();
  // 首条已是第1章 → 不改
  if (/^第\s*[1一]\s*章/.test(first)) return list;
  const startsAtTwo = /^第\s*[2二]\s*章/.test(first);
  if (!startsAtTwo) return list;
  const cn = ["一", "二", "三", "四", "五", "六", "七", "八", "九", "十"];
  return list.map((row, i) => {
    const n = i + 1;
    const label = n <= 10 ? `第${cn[n - 1]}章` : `第${n}章`;
    const old = String(row.title || "").trim();
    const rest = old.replace(/^第\s*[0-9一二三四五六七八九十百千]+\s*章\s*[：:\-]?\s*/, "");
    return {
      ...row,
      title: rest ? `${label}：${rest}` : label,
    };
  });
}

/**
 * @param {Array<{title:string, summary:string, must_do?:string, selected?:boolean}>} [plan]
 * @param {{ mode?: "full"|"append", skipConfirm?: boolean, startWriting?: boolean, instruction?: string }} [opts]
 */
export async function applyChapterPlan(plan, opts = {}) {
  if (!appState.projectRoot || !appState.project) {
    throw new Error(t("project.needProject"));
  }
  const mode = opts.mode === "append" ? "append" : "full";
  let rows = (Array.isArray(plan) ? plan : aiPanelForm.chapterPlan || []).filter(
    (c) => c && c.selected !== false && (c.title || c.summary)
  );
  if (!rows.length) {
    throw new Error(t("bookQ.noSelected"));
  }
  if (mode === "full") {
    rows = normalizeChapterPlanTitles(rows);
  }

  if (!opts.skipConfirm) {
    const titles = rows.map((c, i) => `${i + 1}. ${c.title || t("bookQ.untitled")}`).join("\n");
    const tip =
      mode === "append"
        ? t("bookQ.confirmAppend", { n: rows.length, titles })
        : t("bookQ.confirmFull", { n: rows.length, titles });
    const ok = await appConfirm(tip, {
      title: mode === "append" ? t("bookQ.confirmAppendTitle") : t("bookQ.confirmFullTitle"),
      confirmText: t("bookQ.startWrite"),
      cancelText: t("common.cancel"),
    });
    if (!ok) throw new Error(t("bookQ.cancelledWrite"));
  }

  const createdIds = [];
  const updatedIds = [];
  let rowIndex = 0;

  if (mode !== "append") {
    const ordered = (appState.project.chapters || []).slice();
    for (const ch of ordered) {
      if (rowIndex >= rows.length) break;
      const reusable = await chapterIsReusableEmpty(ch);
      if (!reusable) continue;
      const row = rows[rowIndex];
      rowIndex += 1;
      await updateChapterMeta(ch.id, {
        title: row.title || ch.title,
        summary: row.summary || "",
        status: "pending",
        patch: {
          must_do: row.must_do || "",
          must_not: composeMustNot(row, appState.project.book_outline),
        },
      });
      updatedIds.push(ch.id);
    }
  }

  for (; rowIndex < rows.length; rowIndex++) {
    const row = rows[rowIndex];
    const r = await createChapter(row.title || writingT("editor.chapterN", { n: rowIndex + 1 }), row.summary || "", {
      load: false,
    });
    const id = r.chapter && r.chapter.id;
    if (id) {
      createdIds.push(id);
      await updateChapterMeta(id, {
        status: "pending",
        patch: {
          ...(row.must_do ? { must_do: row.must_do } : {}),
          must_not: composeMustNot(row, appState.project.book_outline),
        },
      });
    }
  }

  await getProject(appState.projectRoot);
  const writtenIds = [...updatedIds, ...createdIds];
  const startChapterId = writtenIds[0] || appState.chapterId;
  if (startChapterId && startChapterId !== appState.chapterId) {
    await loadChapter(startChapterId);
  }

  aiPanelForm.chapterPlan = [];
  const startWriting = opts.startWriting !== false;
  let writingCancelled = false;
  if (startWriting && writtenIds.length) {
    appState.statusMessage = t("bookQ.writing", { n: writtenIds.length });
    await runOutlineQueue({
      instruction: String(opts.instruction ?? aiPanelForm.instruction ?? "").trim(),
      startChapterId,
      onlyChapterIds: writtenIds,
    });
    writingCancelled = outlineQueueState.phase === "cancelled";
    if (writingCancelled) {
      appState.statusMessage = t("bookQ.writtenCancelled", { n: writtenIds.length });
    }
  } else {
    appState.statusMessage =
      t("bookQ.writtenPending", { n: writtenIds.length });
  }
  return { createdIds, updatedIds, startChapterId, writingCancelled };
}

export function findFirstPendingOutlineChapter() {
  const project = appState.project;
  if (!project) return null;
  const chapters = project.chapters || [];
  const volumes = project.volumes || [];
  const ordered = [];
  const seen = new Set();
  for (const vol of volumes) {
    for (const id of vol.chapter_ids || []) {
      if (!seen.has(id)) {
        ordered.push(id);
        seen.add(id);
      }
    }
  }
  for (const ch of chapters) {
    if (!seen.has(ch.id)) ordered.push(ch.id);
  }
  for (const id of ordered) {
    const ch = chapters.find((c) => c.id === id);
    if (!ch) continue;
    if (ch.status === "done" || ch.status === "outline_complete") continue;
    const hasOutline =
      String(ch.summary || "").trim() ||
      (Array.isArray(ch.beats) && ch.beats.length > 0);
    if (hasOutline) return ch;
  }
  return null;
}

/**
 * 续拆后续章：拆章 → 确认「开始写」→ 追加到目录并按这些章开写
 * @param {{ instruction?: string }} [opts]
 */
export async function runContinueOutline(opts = {}) {
  const { chapters, reason } = await runSplitChapters({
    mode: "append",
    instruction: opts.instruction ?? aiPanelForm.instruction,
    bookOutline: aiPanelForm.bookOutline,
  });
  const applied = await applyChapterPlan(chapters, {
    mode: "append",
    instruction: opts.instruction ?? aiPanelForm.instruction,
  });
  return { ...applied, reason, chapters };
}

/**
 * 拆章写入目录并按这些章开写（full）
 */
export async function runSplitAndApply(opts = {}) {
  const { chapters, reason } = await runSplitChapters({
    mode: "full",
    instruction: opts.instruction,
    bookOutline: opts.bookOutline,
  });
  const applied = await applyChapterPlan(chapters, {
    mode: "full",
    instruction: opts.instruction ?? aiPanelForm.instruction,
  });
  return { ...applied, reason, chapters };
}

/**
 * 只按纲写一章
 * @param {string} chapterId
 * @param {{ instruction?: string }} [opts]
 */
export async function runSingleChapterOutline(chapterId, opts = {}) {
  const id = String(chapterId || "").trim();
  if (!id) throw new Error(t("bookQ.needChapterId"));
  await runOutlineQueue({
    startChapterId: id,
    stopAfterOneChapter: true,
    instruction: opts.instruction ?? aiPanelForm.instruction,
  });
}

/**
 * @param {{ instruction?: string, applyPlanFirst?: boolean }} opts
 */
export async function runFullOutlinePipeline(opts = {}) {
  const userInstr = String(opts.instruction ?? aiPanelForm.instruction ?? "").trim();

  if (opts.applyPlanFirst && (aiPanelForm.chapterPlan || []).length) {
    await applyChapterPlan(aiPanelForm.chapterPlan, {
      mode: "full",
      instruction: userInstr,
    });
    return;
  }

  let start = findFirstPendingOutlineChapter();
  if (!start) {
    if (!(aiPanelForm.chapterPlan || []).length) {
      throw new Error(t("bookQ.noPending"));
    }
    await applyChapterPlan(aiPanelForm.chapterPlan, {
      mode: "full",
      instruction: userInstr,
    });
    return;
  }

  await runOutlineQueue({
    instruction: userInstr,
    startChapterId: start.id,
  });
}

export { cancelOutlineQueue };
