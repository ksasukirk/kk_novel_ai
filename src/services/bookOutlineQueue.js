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

export { isChapterBodyEmpty };

/**
 * @param {string} text
 */
export function parseOutlineToChapters(text) {
  const raw = String(text || "").trim();
  if (!raw) return { chapters: [], reason: "" };
  let body = raw;
  const fence = raw.match(/```(?:json)?\s*([\s\S]*?)```/i);
  if (fence) body = fence[1].trim();
  const start = body.indexOf("{");
  const end = body.lastIndexOf("}");
  if (start < 0 || end <= start) return { chapters: [], reason: "" };
  let data;
  try {
    data = JSON.parse(body.slice(start, end + 1));
  } catch {
    return { chapters: [], reason: "" };
  }
  const list = Array.isArray(data.chapters) ? data.chapters : [];
  const chapters = [];
  for (const item of list) {
    if (!item || typeof item !== "object") continue;
    chapters.push({
      title: String(item.title || "").trim(),
      summary: String(item.summary || "").trim(),
      must_do: String(item.must_do || item.mustDo || "").trim(),
      must_not: String(item.must_not || item.mustNot || "").trim(),
      selected: true,
    });
  }
  return {
    chapters: chapters.filter((c) => c.title || c.summary).slice(0, 30),
    reason: String(data.reason || "").trim(),
  };
}

/**
 * @param {string} [text]
 */
export async function saveBookOutline(text) {
  if (!appState.projectRoot || !appState.project) {
    throw new Error("请先打开作品");
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
      ? `全书大纲已保存，书名暂用「${next.title}」`
      : "全书大纲已保存";
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
    throw new Error("请先打开作品");
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
    throw new Error("目录里还没有章节，请先用「按纲生成」写入章节队列");
  }
  const r = await createChapter("第一章", "", { load: true });
  const id = (r.chapter && r.chapter.id) || appState.chapterId;
  if (!id) throw new Error("创建占位章节失败");
  return id;
}

/**
 * @param {{ instruction?: string, bookOutline?: string, mode?: "full"|"append", skipSaveOutline?: boolean }} opts
 */
export async function runSplitChapters(opts = {}) {
  if (!appState.projectRoot) {
    throw new Error("请先打开作品");
  }
  const mode = opts.mode === "append" ? "append" : "full";
  await ensureChapterContext({ allowCreate: mode === "full" });
  if (outlineQueueState.running) {
    throw new Error("按纲生成已在进行中");
  }
  if (visibleGenJobs.value.length) {
    throw new Error("请先等当前草稿写完或取消，再拆章");
  }

  if (mode === "full") {
    const outline = resolveBookOutlineSeed(opts);
    if (!outline) {
      throw new Error("请先填写创作提示或全书大纲（上方框或底部指令均可）");
    }
    await saveBookOutline(outline);
  } else if (!opts.skipSaveOutline) {
    // 续拆也落盘当前全书大纲（可空，靠已有章上下文）
    const bo = resolveBookOutlineSeed(opts);
    if (bo) await saveBookOutline(bo);
  }

  const userInstr = String(opts.instruction ?? aiPanelForm.instruction ?? "").trim();
  outlineQueueState.phase = "splitting_chapters";
  outlineQueueState.running = true;
  outlineQueueState.cancelled = false;
  outlineQueueState.error = "";
  appState.statusMessage =
    mode === "append" ? "正在续拆后续章节…" : "正在根据提示生成章节队列…";

  const splitJob = createGenJob({
    label: mode === "append" ? "续拆后续章" : "生成章节队列",
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
    appState.statusMessage = "已取消拆章";
    throw new Error("已取消拆章");
  }

  const planText =
    (splitResult && (splitResult.raw_text || splitResult.text)) ||
    splitJob.previewRawText ||
    splitJob.previewText ||
    "";
  const { chapters, reason } = parseOutlineToChapters(planText);
  outlineQueueState.running = false;
  outlineQueueState.phase = "";

  if (!chapters.length) {
    throw new Error(
      mode === "append"
        ? "未能续拆出后续章节，请补充创作提示或微调指令"
        : "未能拆出有效章节，请检查提示词后重试"
    );
  }

  aiPanelForm.chapterPlan = chapters;
  appState.statusMessage = reason
    ? `已拆出 ${chapters.length} 章 · ${reason}`
    : `已拆出 ${chapters.length} 章，确认后写入目录并开始写`;
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
    throw new Error("请先打开作品");
  }
  const mode = opts.mode === "append" ? "append" : "full";
  let rows = (Array.isArray(plan) ? plan : aiPanelForm.chapterPlan || []).filter(
    (c) => c && c.selected !== false && (c.title || c.summary)
  );
  if (!rows.length) {
    throw new Error("没有勾选要写入的章节");
  }
  if (mode === "full") {
    rows = normalizeChapterPlanTitles(rows);
  }

  if (!opts.skipConfirm) {
    const titles = rows.map((c, i) => `${i + 1}. ${c.title || "（无标题）"}`).join("\n");
    const tip =
      mode === "append"
        ? `将追加 ${rows.length} 个待写章节到目录，并开始按纲写正文：\n${titles}`
        : `将写入 ${rows.length} 个章节到目录并开始按纲写正文（空首章会更新为第1条，其余追加；已有正文/章纲的章不覆盖）：\n${titles}`;
    const ok = await appConfirm(tip, {
      title: mode === "append" ? "确认续拆写入" : "确认拆章写入",
      confirmText: "开始写",
      cancelText: "取消",
    });
    if (!ok) throw new Error("已取消写入章节");
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
    const r = await createChapter(row.title || `第${rowIndex + 1}章`, row.summary || "", {
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
    appState.statusMessage = `已写入目录 ${writtenIds.length} 章，正在按纲开写…`;
    await runOutlineQueue({
      instruction: String(opts.instruction ?? aiPanelForm.instruction ?? "").trim(),
      startChapterId,
      onlyChapterIds: writtenIds,
    });
    writingCancelled = outlineQueueState.phase === "cancelled";
    if (writingCancelled) {
      appState.statusMessage = `已写入目录 ${writtenIds.length} 章，写作已取消`;
    }
  } else {
    appState.statusMessage =
      `已写入目录 ${writtenIds.length} 章（待写）。` +
      `请在左侧改章名/章纲，再点「写」或「全部按纲写」`;
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
  if (!id) throw new Error("缺少章节 id");
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
      throw new Error("尚无待写章纲：请先「拆成章节」写入目录，或在目录中填写章纲");
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
