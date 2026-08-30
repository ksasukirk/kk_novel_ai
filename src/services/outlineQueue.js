/**
 * 按纲生成队列：beats 逐步推进 + 跨章衔接
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
  getBeatProgress,
  advanceBeatProgress,
} from "./projectClient.js";
import { runBlockDigestAndWait } from "./blockDigest.js";
import {
  canStartMoreJobs,
  createGenJob,
  discardJob,
  visibleGenJobs,
} from "../stores/genJobs.js";
import { newId } from "../services/storyClient.js";
import { invoke } from "./tauri.js";

export const outlineQueueState = reactive({
  running: false,
  cancelled: false,
  /** "" | "splitting_chapters" | "splitting" | "writing" | "switching" | "done" | "cancelled" | "error" */
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
  if (s.phase === "splitting") return "正在从章纲拆分节拍…";
  if (s.phase === "switching") return `切换章节 · ${s.chapterTitle}`;
  if (s.phase === "writing") {
    const bit = s.beatTitle ? ` · ${s.beatTitle}` : "";
    return `按纲生成 节拍 ${s.beatIndex}/${s.beatTotal}${bit}`;
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

/** @param {import('../stores/appState.js').appState.project} project */
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
  const pos = ids.indexOf(currentId);
  if (pos < 0) return null;
  for (let i = pos + 1; i < ids.length; i++) {
    const ch = chapterById(ids[i]);
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
 * @param {string} text
 */
export function parseOutlineToBeats(text) {
  const raw = String(text || "").trim();
  if (!raw) return { beats: [], reason: "" };
  let body = raw;
  const fence = raw.match(/```(?:json)?\s*([\s\S]*?)```/i);
  if (fence) body = fence[1].trim();
  const start = body.indexOf("{");
  const end = body.lastIndexOf("}");
  if (start < 0 || end <= start) return { beats: [], reason: "" };
  let data;
  try {
    data = JSON.parse(body.slice(start, end + 1));
  } catch {
    return { beats: [], reason: "" };
  }
  const list = Array.isArray(data.beats) ? data.beats : [];
  const beats = [];
  for (const item of list) {
    if (!item || typeof item !== "object") continue;
    beats.push({
      id: newId(),
      title: String(item.title || "").trim(),
      purpose: String(item.purpose || item.task || "").trim(),
      conflict: String(item.conflict || "").trim(),
      emotion: String(item.emotion || "").trim(),
      location: String(item.location || "").trim() || null,
    });
  }
  return {
    beats: beats.filter((b) => b.title || b.purpose),
    reason: String(data.reason || "").trim(),
  };
}

function wrapBeatInstruction(beat, index, total, userInstr) {
  const user = String(userInstr || "").trim();
  const parts = [
    `【按纲生成 节拍 ${index}/${total} · ${beat.title || `第${index}拍`}】只写本节拍；须达到或超出规定字数后再停；禁止跳拍、禁止复述已完成节拍。`,
    `本节拍：标题=${beat.title}；目的=${beat.purpose}；冲突=${beat.conflict}；情绪=${beat.emotion}；地点=${beat.location || "（未设）"}`,
  ];
  if (index === 1) {
    parts.push(
      "若存在上章收束：本拍必须承接上章时间/地点/人物状态，禁止冷开场重置；人称性别须与上章及设定一致（女生用「她」，禁止改成表弟/他）。"
    );
  }
  if (user) parts.push(`用户微调（不得覆盖本节拍）：\n${user}`);
  return parts.join("\n");
}

async function ensureChapterBeats(chapter, userInstr) {
  if (Array.isArray(chapter.beats) && chapter.beats.length > 0) {
    return chapter.beats;
  }
  const summary = String(chapter.summary || "").trim();
  if (!summary) {
    throw new Error(`章节「${chapter.title}」缺少章纲 summary，无法拆分节拍`);
  }
  outlineQueueState.phase = "splitting";
  appState.statusMessage = "正在从章纲拆分节拍…";
  throwIfCancelled();
  await waitForSlot();
  if (appState.dirty) await saveChapter();

  const splitJob = createGenJob({ label: "拆分节拍" });
  splitJob.draftPlacement = "";
  let splitResult;
  try {
    splitResult = await runWriting(
      withBranchContext(
        {
          project_root: appState.projectRoot,
          chapter_id: chapter.id,
          task: "outline_to_beats",
          instruction: userInstr || "",
          selection: "",
        },
        "continue",
        ""
      ),
      { job: splitJob, label: "拆分节拍" }
    );
  } finally {
    discardJob(splitJob);
  }
  throwIfCancelled();

  const planText =
    (splitResult && (splitResult.raw_text || splitResult.text)) ||
    splitJob.previewRawText ||
    splitJob.previewText ||
    "";
  const { beats, reason } = parseOutlineToBeats(planText);
  if (!beats.length) {
    throw new Error("未能从章纲拆出有效节拍，请检查章纲或手动在总谱页填写 beats");
  }
  // 按纲生成：小节/节拍直接落盘，不弹确认
  await updateChapterMeta(chapter.id, { patch: { beats } });
  const tip = reason
    ? `已拆 ${beats.length} 拍 · ${reason}，继续写正文…`
    : `已拆 ${beats.length} 拍，继续写正文…`;
  appState.statusMessage = tip;
  const updated = chapterById(chapter.id);
  return (updated && updated.beats) || beats;
}

async function resolveActiveBeat(chapter) {
  const progress = await getBeatProgress(chapter.id);
  const beats = chapter.beats || [];
  if (!beats.length) return null;
  const map = progress.beats || {};
  if (progress.current_beat_id) {
    const b = beats.find((x) => x.id === progress.current_beat_id);
    if (b && map[b.id] === "in_progress") return b;
  }
  for (const b of beats) {
    if (map[b.id] === "in_progress") return b;
  }
  for (const b of beats) {
    if (!map[b.id] || map[b.id] === "pending") return b;
  }
  return null;
}

function beatIndexOf(beats, beat) {
  if (!beat) return 0;
  const i = beats.findIndex((b) => b.id === beat.id);
  return i >= 0 ? i + 1 : 0;
}

async function runChapterOutlineQueue(chapterId, userInstr) {
  const chapter = chapterById(chapterId);
  if (!chapter) throw new Error("章节不存在");

  outlineQueueState.chapterId = chapter.id;
  outlineQueueState.chapterTitle = chapter.title || "";

  const beats = await ensureChapterBeats(chapter, userInstr);

  while (true) {
    throwIfCancelled();
    const chNow = chapterById(chapter.id) || chapter;
    const beatList = chNow.beats || beats;
    outlineQueueState.beatTotal = beatList.length;
    const active = await resolveActiveBeat(chNow);
    if (!active) break;

    outlineQueueState.beatIndex = beatIndexOf(beatList, active);
    outlineQueueState.beatTitle = active.title || "";
    outlineQueueState.phase = "writing";
    appState.statusMessage = outlineQueueStatusLine();

    await waitForSlot();
    if (appState.dirty) await saveChapter();

    const wrapped = wrapBeatInstruction(
      active,
      outlineQueueState.beatIndex,
      beatList.length,
      userInstr
    );

    appState.draftPlacement = "editor";
    appState.draftTask = "continue";
    appState.draftSelection = "";
    appState.draftInstruction = wrapped;
    appState.draftPersistInstruction = wrapped;
    appState.draftActiveBeatId = active.id;
    appState.draftRewriteBlockKey = "";
    appState.draftAnchorBlockKey = "";
    appState.draftBranchMode = "";
    appState.draftBranchNodeId = "";
    appState.draftForkFromVariantId = "";

    const job = createGenJob({
      label: `节拍 ${outlineQueueState.beatIndex}/${beatList.length}`,
    });
    job.draftActiveBeatId = active.id;

    try {
      await runWriting(
        withBranchContext(
          {
            project_root: appState.projectRoot,
            chapter_id: chNow.id,
            task: "continue",
            instruction: wrapped,
            selection: "",
            active_beat_id: active.id,
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

    await advanceBeatProgress(chNow.id, active.id);
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
 * 按纲生成（跨章）：从指定章（或当前章）起，逐节拍生成，章完成后切下一章
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
