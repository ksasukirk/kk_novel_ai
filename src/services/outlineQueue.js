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
  getMemory,
  peekChapterBlocks,
} from "./projectClient.js";
import {
  bodyHasSubstantialProse,
  snapshotLooksValid,
} from "../utils/outlineSnapshot.js";
import { runBlockDigestAndWait } from "./blockDigest.js";
import {
  canStartMoreJobs,
  createGenJob,
  discardJob,
  visibleGenJobs,
} from "../stores/genJobs.js";
import { createPlainBlock } from "../utils/genBlock.js";
import { migrateBlocksToBranchDoc } from "../utils/branchModel.js";
import { isChapterBodyEmpty } from "../utils/chapterStatus.js";
import { continuityWriteHint } from "../utils/outlineContinuity.js";
import { isCancelledMsg, t, tLocale } from "../i18n/index.js";

function writingT(key, values) {
  return tLocale(appState.settings?.writing_locale || "zh-CN", key, values);
}

export const outlineQueueState = reactive({
  running: false,
  cancelled: false,
  /** "" | "splitting_chapters" | "writing" | "summarizing" | "switching" | "done" | "cancelled" | "error" */
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
  if (s.phase === "splitting_chapters") return t("outlineQ.splitChapters");
  if (s.phase === "switching") return t("outlineQ.switching", { title: s.chapterTitle });
  if (s.phase === "writing") {
    return t("outlineQ.writing", { title: s.chapterTitle || t("editor.thisChapter") });
  }
  if (s.phase === "summarizing") {
    return t("outlineQ.summarizing", { title: s.chapterTitle || t("editor.thisChapter") });
  }
  if (s.phase === "done") {
    return s.chaptersDone > 1
      ? t("outlineQ.doneN", { n: s.chaptersDone })
      : t("outlineQ.done");
  }
  if (s.phase === "cancelled") return t("outlineQ.cancelled");
  if (s.phase === "error") return s.error || t("outlineQ.failed");
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
    if (outlineQueueState.cancelled) throw new Error(t("outlineQ.cancelled"));
    await sleep(350);
  }
}

function throwIfCancelled() {
  if (outlineQueueState.cancelled) throw new Error(t("outlineQ.cancelled"));
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
  const title = String(chapter.title || "").trim();
  const summary = String(chapter.summary || "").trim();
  const parts = [
    writingT("outlineQ.instrWrap", { title }),
    writingT("outlineQ.instrCover"),
    continuityWriteHint(),
  ];
  if (summary) {
    parts.push(writingT("outlineQ.chOutline", { summary }));
  }
  if (user) {
    parts.push(writingT("outlineQ.userTweak", { user }));
  }
  return parts.join("\n");
}

function chapterHasSubstantialBody(chapter) {
  if (!chapter || appState.chapterId !== chapter.id) return false;
  const title = chapter.title || "";
  if (isChapterBodyEmpty(appState.chapterContent, title)) return false;
  return bodyHasSubstantialProse(appState.chapterContent, 80);
}

async function chapterDiskHasBody(chapterId) {
  const blocks = await peekChapterBlocks(chapterId);
  const raw = (blocks || []).map((b) => String(b.text || "")).join("");
  return bodyHasSubstantialProse(raw, 80);
}

async function chapterHasWrittenSnapshot(chapterId) {
  try {
    const memory = await getMemory();
    const snaps = (memory && memory.chapter_snapshots) || [];
    const hit = snaps.find((s) => s && s.chapter_id === chapterId);
    if (!snapshotLooksValid((hit && hit.summary) || "")) return false;
    if (!(await chapterDiskHasBody(chapterId))) return false;
    return true;
  } catch {
    return false;
  }
}

/**
 * 写后章节总结：只写入 memory 快照，禁止覆盖章纲。失败则抛错停队列。
 */
async function runChapterWrittenSummary(chapter) {
  outlineQueueState.phase = "summarizing";
  outlineQueueState.chapterId = chapter.id;
  outlineQueueState.chapterTitle = chapter.title || "";
  appState.statusMessage = outlineQueueStatusLine();
  throwIfCancelled();
  await waitForSlot();

  const prevPlacement = appState.draftPlacement;
  appState.draftPlacement = "";
  const job = createGenJob({
    label: t("outlineQ.labelSummary", { title: chapter.title || t("editor.thisChapter") }),
  });
  job.draftPlacement = "";
  job.draftTask = "chapter_summary";

  try {
    const result = await runWriting(
      {
        project_root: appState.projectRoot,
        chapter_id: chapter.id,
        task: "chapter_summary",
        instruction: "",
        selection: "",
      },
      { job, label: job.label }
    );
    throwIfCancelled();
    const text = String(
      (result && (result.text || result.raw_text)) ||
        job.previewRawText ||
        job.previewText ||
        ""
    ).trim();
    if (!snapshotLooksValid(text)) {
      throw new Error(
        t("outlineQ.sumShort", { title: chapter.title || t("editor.thisChapter") })
      );
    }
    return text;
  } catch (e) {
    const msg = String(e.message || e);
    if (outlineQueueState.cancelled || isCancelledMsg(msg)) {
      throw new Error(t("outlineQ.cancelled"));
    }
    throw new Error(
      t("outlineQ.sumFailed", {
        title: chapter.title || t("editor.thisChapter"),
        msg,
      })
    );
  } finally {
    discardJob(job);
    appState.draftPlacement = prevPlacement;
  }
}

async function runChapterOutlineQueue(chapterId, userInstr) {
  if (appState.chapterId !== chapterId) {
    await saveChapter();
    await loadChapter(chapterId);
  }

  const chapter = chapterById(chapterId);
  if (!chapter) throw new Error(t("outlineQ.noChapter"));

  const title = String(chapter.title || "").trim();
  if (!title) {
    throw new Error(t("outlineQ.needTitle"));
  }
  const summary = String(chapter.summary || "").trim();
  if (!summary && !(Array.isArray(chapter.beats) && chapter.beats.length)) {
    throw new Error(t("outlineQ.needSummary", { title }));
  }

  outlineQueueState.chapterId = chapter.id;
  outlineQueueState.chapterTitle = title;
  outlineQueueState.beatIndex = 1;
  outlineQueueState.beatTotal = 1;
  outlineQueueState.beatTitle = "";
  appState.statusMessage = outlineQueueStatusLine();

  throwIfCancelled();
  if (appState.dirty) await saveChapter();

  const skipRewrite = chapterHasSubstantialBody(chapter);

  if (!skipRewrite) {
    outlineQueueState.phase = "writing";
    appState.statusMessage = outlineQueueStatusLine();
    await waitForSlot();

    if (appState.chapterId !== chapter.id) {
      await loadChapter(chapter.id);
    }

    // 空章或残稿才清空；已有实质正文（总结失败重跑）禁止抹掉
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
      label: t("outlineQ.labelChapter", { title }),
      targetChapterId: chapter.id,
      skipAutoAccept: true,
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
            outline_run: true,
          },
          "continue",
          ""
        ),
        { job, label: job.label }
      );
    } catch (e) {
      const msg = String(e.message || e);
      if (outlineQueueState.cancelled || isCancelledMsg(msg)) {
        throw new Error(t("outlineQ.cancelled"));
      }
      throw e;
    }
    throwIfCancelled();

    if (job.status === "done" && !job.accepted) {
      const acc = await acceptDraft(job);
      if (!acc.ok) throw new Error(acc.error || t("draft.writeFailed"));
    }

    await saveChapter();
    if (!(await chapterDiskHasBody(chapter.id))) {
      throw new Error(t("outlineQ.emptyBody", { title }));
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
  }

  if (!(await chapterHasWrittenSnapshot(chapter.id))) {
    await runChapterWrittenSummary(chapter);
  }

  if (!(await chapterHasWrittenSnapshot(chapter.id))) {
    throw new Error(t("outlineQ.sumMissing", { title: title || t("editor.thisChapter") }));
  }

  if (!(await chapterDiskHasBody(chapter.id))) {
    throw new Error(t("outlineQ.emptyBody", { title }));
  }

  await updateChapterMeta(chapter.id, {
    patch: { status: "outline_complete" },
  });
}

function assertCanStartOutlineQueue() {
  if (!appState.projectRoot || !appState.chapterId) {
    throw new Error(t("outlineQ.needOpen"));
  }
  if (outlineQueueState.running) {
    throw new Error(t("outlineQ.alreadyRunning"));
  }
  if (visibleGenJobs.value.length) {
    throw new Error(t("outlineQ.waitDraft"));
  }
}

function chapterHasWritableOutline(chapter) {
  if (!chapter) return false;
  const st = String(chapter.status || "").toLowerCase();
  if (st === "outline_complete" || st === "done" || st === "completed") return false;
  return (
    String(chapter.summary || "").trim() ||
    (Array.isArray(chapter.beats) && chapter.beats.length > 0)
  );
}

function nextOnlyChapterId(onlyIds, currentId) {
  const idx = onlyIds.indexOf(currentId);
  const start = idx >= 0 ? idx + 1 : 0;
  for (let i = start; i < onlyIds.length; i += 1) {
    if (chapterHasWritableOutline(chapterById(onlyIds[i]))) return onlyIds[i];
  }
  return "";
}

/**
 * 按纲生成（跨章）：从指定章（或当前章）起，每章整章写一次，完成后切下一章
 * @param {{ instruction?: string, startChapterId?: string, stopAfterOneChapter?: boolean, onlyChapterIds?: string[] }} opts
 */
export async function runOutlineQueue(opts = {}) {
  assertCanStartOutlineQueue();
  const userInstr = String(opts.instruction || "").trim();
  const onlyIds = Array.isArray(opts.onlyChapterIds)
    ? opts.onlyChapterIds.map((id) => String(id || "").trim()).filter(Boolean)
    : [];
  const startId = String(opts.startChapterId || "").trim() || onlyIds[0] || appState.chapterId;
  const stopAfterOne = !!opts.stopAfterOneChapter;

  resetOutlineQueue();
  outlineQueueState.running = true;
  appState.statusMessage = stopAfterOne ? t("outlineQ.startOne") : t("outlineQ.start");

  let chapterId = startId;
  if (onlyIds.length && !chapterHasWritableOutline(chapterById(chapterId))) {
    chapterId = nextOnlyChapterId(onlyIds, "") || onlyIds[0];
  }
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

      if (onlyIds.length) {
        chapterId = nextOnlyChapterId(onlyIds, chapterId);
        continue;
      }

      const next = findNextOutlineChapter(chapterId);
      if (!next) break;
      chapterId = next.id;
    }

    outlineQueueState.phase = "done";
    outlineQueueState.running = false;
    appState.statusMessage = outlineQueueStatusLine();
  } catch (e) {
    const msg = String(e.message || e);
    const cancelled = outlineQueueState.cancelled || isCancelledMsg(msg);
    outlineQueueState.running = false;
    outlineQueueState.phase = cancelled ? "cancelled" : "error";
    outlineQueueState.error = cancelled ? "" : msg;
    appState.statusMessage = cancelled ? t("outlineQ.cancelled") : msg;
    if (!cancelled) throw e;
  }
}
