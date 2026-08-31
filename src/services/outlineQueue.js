/**
 * 按纲生成队列：每章整章一次续写 + 跨章衔接（不再拆小节/节拍）
 * 代码路径: kk_novel_ai/src/services/outlineQueue.js
 */
import { reactive } from "vue";
import { appState } from "../stores/appState.js";
import { runWriting } from "./llmClient.js";
import { acceptDraft, withBranchContext } from "./draftAccept.js";
import {
  loadChapter,
  saveChapter,
  updateChapterMeta,
  applyBranchDoc,
} from "./projectClient.js";
import { runBlockDigestAndWait } from "./blockDigest.js";
import {
  canStartMoreJobs,
  createGenJob,
  discardJob,
  visibleGenJobs,
} from "../stores/genJobs.js";
import { createPlainBlock } from "../utils/genBlock.js";
import { migrateBlocksToBranchDoc } from "../utils/branchModel.js";
import { invoke } from "./tauri.js";

export const outlineQueueState = reactive({
  running: false,
  cancelled: false,
  /** "" | "splitting_chapters" | "writing" | "switching" | "done" | "cancelled" | "error" */
  phase: "",
  chapterId: "",
  chapterTitle: "",
  beatIndex: 0,
  beatTotal: 0,
  beatTitle: "",
  chaptersDone: 0,
  error: "",
});

export function outlineQueueStatusLine() {
  const s = outlineQueueState;
  if (!s.running && s.phase !== "done") return "";
  if (s.phase === "splitting_chapters") return "正在从全书大纲拆成章节…";
  if (s.phase === "switching") return `切换章节 · ${s.chapterTitle}`;
  if (s.phase === "writing") {
    return `按纲生成整章 · ${s.chapterTitle || "本章"}`;
  }
  if (s.phase === "done") {
    return s.chaptersDone > 1
      ? `按纲生成已完成 ${s.chaptersDone} 章`
      : "按纲生成已完成";
  }
  if (s.phase === "cancelled") return "已取消按纲生成";
  if (s.phase === "error") return s.error || "按纲生成失败";
  return "";
}

export function cancelOutlineQueue() {
  outlineQueueState.cancelled = true;
}

function resetOutlineQueue() {
  outlineQueueState.running = false;
  outlineQueueState.cancelled = false;
  outlineQueueState.phase = "";
  outlineQueueState.chapterId = "";
  outlineQueueState.chapterTitle = "";
  outlineQueueState.beatIndex = 0;
  outlineQueueState.beatTotal = 0;
  outlineQueueState.beatTitle = "";
  outlineQueueState.chaptersDone = 0;
  outlineQueueState.error = "";
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function waitForSlot() {
  while (!canStartMoreJobs(1)) {
    if (outlineQueueState.cancelled) throw new Error("已取消按纲生成");
    await sleep(350);
  }
}

function throwIfCancelled() {
  if (outlineQueueState.cancelled) throw new Error("已取消按纲生成");
}

function chapterById(id) {
  const list = (appState.project && appState.project.chapters) || [];
  return list.find((c) => c.id === id) || null;
}

function orderedChapterIds(project) {
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
  return ordered;
}

function findNextOutlineChapter(currentId) {
  const project = appState.project;
  if (!project) return null;
  const ids = orderedChapterIds(project);
  const idx = ids.indexOf(currentId);
  const start = idx >= 0 ? idx + 1 : 0;
  for (let i = start; i < ids.length; i += 1) {
    const ch = chapterById(ids[i]);
    if (!ch) continue;
    const st = String(ch.status || "").toLowerCase();
    if (st === "outline_complete" || st === "done" || st === "completed") continue;
    const hasOutline =
      String(ch.summary || "").trim() ||
      (Array.isArray(ch.beats) && ch.beats.length > 0);
    if (hasOutline) return ch;
  }
  return null;
}

/**
 * 兼容旧调用：章纲拆节拍已取消，恒返回空
 * @param {string} _text
 */
export function parseOutlineToBeats(_text) {
  return { beats: [], reason: "" };
}

/** @deprecated 已取消按小节拆拍 */
export function resolveMaxSectionsPerChapter() {
  return 1;
}

function wrapChapterInstruction(chapter, userInstr) {
  const user = String(userInstr || "").trim();
  const title = String(chapter.title || "").trim() || "本章";
  const summary = String(chapter.summary || "").trim();
  const parts = [
    `【按纲生成 · 整章一次写完】章节「${title}」。本章正文只生成一整段完整内容，不要拆小节、不要分段标拍、不要写「第一节/第一拍」之类标题。`,
    "须达到或超出规定字数后再停；覆盖章纲中的冲突、推进与结尾钩子；承接上章收束（若有），人称性别与设定一致。",
  ];
  if (summary) {
    parts.push(`本章纲：\n${summary}`);
  }
  if (user) {
    parts.push(`用户微调（不得覆盖章纲主线）：\n${user}`);
  }
  return parts.join("\n");
}

async function runChapterOutlineQueue(chapterId, userInstr) {
  const chapter = chapterById(chapterId);
  if (!chapter) throw new Error("章节不存在");

  const summary = String(chapter.summary || "").trim();
  if (!summary && !(Array.isArray(chapter.beats) && chapter.beats.length)) {
    throw new Error(`章节「${chapter.title}」缺少章纲 summary，无法按纲写正文`);
  }

  outlineQueueState.chapterId = chapter.id;
  outlineQueueState.chapterTitle = chapter.title || "";
  outlineQueueState.beatIndex = 1;
  outlineQueueState.beatTotal = 1;
  outlineQueueState.beatTitle = "";
  outlineQueueState.phase = "writing";
  appState.statusMessage = outlineQueueStatusLine();

  throwIfCancelled();
  await waitForSlot();
  if (appState.dirty) await saveChapter();

  // 整章一块：生成前清空旧正文，避免叠出多个生成块
  const hasBody =
    String(appState.chapterContent || "").trim() ||
    (appState.chapterBlocks || []).some((b) => String(b.text || "").trim());
  if (hasBody) {
    const empty = [createPlainBlock("")];
    applyBranchDoc(migrateBlocksToBranchDoc(empty));
    await saveChapter();
  }

  const wrapped = wrapChapterInstruction(chapter, userInstr);

  appState.draftPlacement = "editor";
  appState.draftTask = "continue";
  appState.draftSelection = "";
  appState.draftInstruction = wrapped;
  appState.draftPersistInstruction = wrapped;
  appState.draftActiveBeatId = "";
  appState.draftRewriteBlockKey = "";
  appState.draftAnchorBlockKey = "";
  appState.draftBranchMode = "";
  appState.draftBranchNodeId = "";
  appState.draftForkFromVariantId = "";

  const job = createGenJob({
    label: `整章 · ${chapter.title || "本章"}`,
  });
  job.draftActiveBeatId = "";

  try {
    await runWriting(
      withBranchContext(
        {
          project_root: appState.projectRoot,
          chapter_id: chapter.id,
          task: "continue",
          instruction: wrapped,
          selection: "",
        },
        "continue",
        ""
      ),
      { job, label: job.label }
    );
  } catch (e) {
    const msg = String(e.message || e);
    if (outlineQueueState.cancelled || /取消/.test(msg)) {
      throw new Error("已取消按纲生成");
    }
    throw e;
  }
  throwIfCancelled();

  if (job.status === "done" && !job.accepted) {
    const acc = await acceptDraft(job);
    if (!acc.ok) throw new Error(acc.error || "写入失败");
  }

  const blockKey =
    job.lastWrittenBlockKey ||
    (appState.chapterBlocks || [])
      .slice()
      .reverse()
      .find((b) => b.type === "gen")?.key ||
    "";
  const blockText =
    (appState.chapterBlocks || []).find((b) => b.key === blockKey)?.text || "";

  if (blockKey && blockText.trim()) {
    await runBlockDigestAndWait(
      { blockKey, text: blockText, instruction: wrapped },
      { timeoutMs: 120000, force: true }
    );
  }

  const chDone = chapterById(chapter.id) || chapter;
  try {
    await invoke("memory_sync_chapter_snapshot", {
      root: appState.projectRoot,
      chapterId: chapter.id,
      fallback: String(chDone.summary || chDone.title || ""),
    });
  } catch (e) {
    console.warn("[outlineQueue] sync chapter snapshot failed", e);
  }

  await updateChapterMeta(chapter.id, {
    patch: { status: "outline_complete" },
  });
}

function assertCanStartOutlineQueue() {
  if (!appState.projectRoot || !appState.chapterId) {
    throw new Error("请先打开作品并选择章节");
  }
  if (outlineQueueState.running) {
    throw new Error("按纲生成已在进行中");
  }
  if (visibleGenJobs.value.length) {
    throw new Error("请先等当前草稿写完或取消，再开按纲生成");
  }
}

/**
 * 按纲生成（跨章）：从指定章（或当前章）起，每章整章写一次，完成后切下一章
 * @param {{ instruction?: string, startChapterId?: string, stopAfterOneChapter?: boolean }} opts
 */
export async function runOutlineQueue(opts = {}) {
  assertCanStartOutlineQueue();
  const userInstr = String(opts.instruction || "").trim();
  const startId = String(opts.startChapterId || "").trim() || appState.chapterId;
  const stopAfterOne = !!opts.stopAfterOneChapter;

  resetOutlineQueue();
  outlineQueueState.running = true;
  appState.statusMessage = stopAfterOne ? "单章按纲生成启动…" : "按纲生成启动…";

  let chapterId = startId;
  let chaptersDone = 0;

  try {
    while (chapterId) {
      throwIfCancelled();
      if (chapterId !== appState.chapterId) {
        outlineQueueState.phase = "switching";
        const chMeta = chapterById(chapterId);
        outlineQueueState.chapterTitle = (chMeta && chMeta.title) || "";
        appState.statusMessage = outlineQueueStatusLine();
        await saveChapter();
        await loadChapter(chapterId);
      }
      await runChapterOutlineQueue(chapterId, userInstr);
      chaptersDone += 1;
      outlineQueueState.chaptersDone = chaptersDone;

      if (stopAfterOne) break;

      const next = findNextOutlineChapter(chapterId);
      if (!next) break;
      chapterId = next.id;
    }

    outlineQueueState.phase = "done";
    outlineQueueState.running = false;
    appState.statusMessage = outlineQueueStatusLine();
  } catch (e) {
    const msg = String(e.message || e);
    const cancelled = outlineQueueState.cancelled || /取消/.test(msg);
    outlineQueueState.running = false;
    outlineQueueState.phase = cancelled ? "cancelled" : "error";
    outlineQueueState.error = cancelled ? "" : msg;
    appState.statusMessage = cancelled ? "已取消按纲生成" : msg;
    if (!cancelled) throw e;
  }
}
