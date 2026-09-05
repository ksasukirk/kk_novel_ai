/**
 * 正文区生成：自动写入 / 删除 / 重写（支持多路 job）
 * 代码路径: kk_novel_ai/src/services/draftAccept.js
 */
import { nextTick } from "vue";
import { appState } from "../stores/appState.js";
import { cancelAllGenerations, cancelJob, runWriting } from "./llmClient.js";
import { pushAiUndo } from "./aiUndo.js";
import { isBackgroundAnalysisTask } from "../utils/writingTasks.js";
import { saveChapter, applyBranchDoc, syncBranchDocFromEditor } from "./projectClient.js";
import {
  contentFromBlocks,
  createGenBlock,
  createPlainBlock,
} from "../utils/genBlock.js";
import {
  addVariant,
  appendOnActivePath,
  branchContextText,
  deleteVariantOrNode,
  findNodeByBlockKey,
  forkChild,
  migrateBlocksToBranchDoc,
  previousSectionDigest,
  replaceVariantText,
  switchVariant,
  variantFromGenBlock,
} from "../utils/branchModel.js";
import {
  activeJobCount,
  canStartMoreJobs,
  clearJobPreview,
  createGenJob,
  discardJob,
  findJobById,
  jobsForAnchor,
  MAX_PARALLEL_GEN,
  refreshLegacyFromJobs,
  trailingVisibleJobs,
  visibleGenJobs,
} from "../stores/genJobs.js";

/** 用户停止滚动多久后才自动写入（避免正看着预览时被换壳打断） */
const SCROLL_IDLE_MS = 1400;

let lastUserScrollAt = 0;
/** 用户主动滚轮/触控/翻页（不含程序改 scrollTop） */
let lastScrollIntentAt = 0;
let deferTimer = null;
/** @type {Set<string>} */
const acceptedRequestIds = new Set();
/** 串行化 accept，避免双重 scroll freeze */
let acceptChain = Promise.resolve();

function enqueueAccept(fn) {
  const run = acceptChain.then(fn, fn);
  acceptChain = run.catch(() => {});
  return run;
}

export function draftBody(job) {
  if (job) {
    return (job.previewRawText || job.previewText || "").trim();
  }
  return (appState.previewRawText || appState.previewText || "").trim();
}

/**
 * 生成中，或已结束但预览尚未写入/清空时，都保持草稿占位。
 */
export function isEditorDraftVisible() {
  if (visibleGenJobs.value.length > 0) return true;
  if (appState.draftPlacement !== "editor") return false;
  if (appState.generating) return true;
  return !!(appState.previewRawText || appState.previewText);
}

/** 草稿是否嵌在指定块内前台显示 */
export function isDraftAnchoredTo(blockKey) {
  if (!blockKey) return false;
  if (jobsForAnchor(blockKey).length > 0) return true;
  if (!isEditorDraftVisible()) return false;
  return appState.draftAnchorBlockKey === blockKey;
}

/** 指定块上的可见草稿 jobs */
export function anchoredJobsFor(blockKey) {
  return jobsForAnchor(blockKey);
}

/** 章末挂载的尾部草稿（续写等无锚点时） */
export function isTrailingEditorDraft() {
  if (trailingVisibleJobs().length > 0) return true;
  return isEditorDraftVisible() && !appState.draftAnchorBlockKey;
}

export function trailingDraftJobs() {
  return trailingVisibleJobs();
}

/** 正文区用户滚动：推迟自动写入（冻结目标位由 withEditorScrollFrozen 独占，避免钳位污染） */
export function noteEditorUserScroll() {
  lastUserScrollAt = Date.now();
}

/**
 * 用户主动意图滚动（滚轮 / 触控 / 翻页键），生成跟随不得抢视口
 * 代码路径: kk_novel_ai/src/services/draftAccept.js
 */
export function noteEditorScrollIntent() {
  const now = Date.now();
  lastScrollIntentAt = now;
  lastUserScrollAt = now;
}

/** 最近是否有主动滚动意图（用于流式预览停止追尾） */
export function recentlyEditorScrollIntent(withinMs = 2000) {
  if (!lastScrollIntentAt) return false;
  return Date.now() - lastScrollIntentAt < withinMs;
}

function cancelDeferredAutoAccept() {
  if (deferTimer) {
    clearTimeout(deferTimer);
    deferTimer = null;
  }
}

export function clearDraftPreview() {
  cancelDeferredAutoAccept();
  for (const j of [...visibleGenJobs.value]) {
    clearJobPreview(j);
  }
  refreshLegacyFromJobs();
  appState.previewText = "";
  appState.previewRawText = "";
  appState.lastTruncated = false;
  appState.lastIncomplete = false;
  appState.lastUsage = null;
  appState.lastLogId = "";
  appState.lastCostCny = 0;
  appState.draftPlacement = "";
  appState.draftTask = "";
  appState.draftSelection = "";
  appState.draftInstruction = "";
  appState.draftPersistInstruction = "";
  appState.draftRewriteBlockKey = "";
  appState.draftAnchorBlockKey = "";
  appState.draftBranchMode = "";
  appState.draftBranchNodeId = "";
  appState.draftForkFromVariantId = "";
  appState.lastContextSources = null;
}

/** 取消并丢弃某一路草稿 */
export async function rejectJob(jobOrId) {
  cancelDeferredAutoAccept();
  const job = typeof jobOrId === "string" ? findJobById(jobOrId) : jobOrId;
  if (!job) {
    await rejectDraft();
    return;
  }
  try {
    await cancelJob(job);
  } catch {
    discardJob(job);
    refreshLegacyFromJobs();
  }
  appState.statusMessage = "已取消生成";
}

/** 取消全部编辑区生成（面板「取消」） */
export async function rejectDraft() {
  cancelDeferredAutoAccept();
  try {
    const { cancelSectionQueue } = await import("./sectionQueue.js");
    cancelSectionQueue();
  } catch {
    /* ignore */
  }
  try {
    const { cancelOutlineQueue } = await import("./outlineQueue.js");
    cancelOutlineQueue();
  } catch {
    /* ignore */
  }
  try {
    await cancelAllGenerations();
  } catch {
    /* ignore */
  }
  clearDraftPreview();
  appState.genProgressPct = 0;
  appState.genStreamChars = 0;
  appState.statusMessage = "已取消生成";
}

function ensureBlockList() {
  if (Array.isArray(appState.chapterBlocks) && appState.chapterBlocks.length) {
    return appState.chapterBlocks.map((b) => ({ ...b }));
  }
  const existing = (appState.chapterContent || "").trim();
  return existing ? [createPlainBlock(appState.chapterContent || "")] : [];
}

/** 写入正文后落盘；失败不回滚正文，只提示 */
async function saveAfterWrite(okMessage) {
  try {
    await saveChapter();
    appState.statusMessage = okMessage;
  } catch (e) {
    appState.statusMessage = `${okMessage}，但保存失败：${e.message || e}`;
  }
}

function editorScroller() {
  return typeof document !== "undefined"
    ? document.querySelector(".editor-scroll")
    : null;
}

function doubleRaf() {
  return new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
}

function settleFrames() {
  return nextTick().then(() => doubleRaf());
}

function scheduleCastThenStorySync(writtenKey, text, instruction) {
  const body = String(text || "").trim();
  if (!writtenKey || !body) return;
  void (async () => {
    try {
      const cast = await import("./castExtract.js");
      await cast.runCastExtract({
        blockKey: writtenKey,
        text: body,
        instruction,
      });
    } catch {
      /* 抽角色失败仍尝试总谱 */
    }
    try {
      const sync = await import("./storySync.js");
      await sync.runStorySync({
        blockKey: writtenKey,
        text: body,
        instruction,
      });
    } catch {
      /* runStorySync 内部已提示 */
    }
  })();
}

/** 写入直前快照 scrollTop（勿在 pushAiUndo 等 await 之前拍，会过期） */
function captureEditorScroll() {
  const scroller = editorScroller();
  return {
    scroller,
    scrollTop: scroller ? scroller.scrollTop : 0,
  };
}

/** 草稿在滚动容器中的视口锚点（换壳后把新块对齐到同一位置） */
function measureDraftViewportAnchor(scroller) {
  if (!scroller) return null;
  const draft = scroller.querySelector(".editor-draft");
  if (!draft) return null;
  const sRect = scroller.getBoundingClientRect();
  const dRect = draft.getBoundingClientRect();
  return {
    viewOffset: dRect.top - sRect.top,
  };
}

function alignBlockToViewportOffset(scroller, blockKey, viewOffset) {
  if (!scroller || !blockKey || viewOffset == null) return false;
  const safeKey =
    typeof CSS !== "undefined" && typeof CSS.escape === "function"
      ? CSS.escape(blockKey)
      : String(blockKey).replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  const el = scroller.querySelector(`.chapter-block[data-block-key="${safeKey}"]`);
  if (!el) return false;
  const sRect = scroller.getBoundingClientRect();
  const bRect = el.getBoundingClientRect();
  const next = scroller.scrollTop + (bRect.top - sRect.top) - viewOffset;
  const max = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
  scroller.scrollTop = Math.min(Math.max(0, next), max);
  return true;
}

/**
 * 草稿↔生成块切换时稳住视口。
 * - 目标 scrollTop 固定为换壳前快照；高度塌陷钳位不得改写目标
 * - 仅 wheel / touch / 翻页键视为用户接管，scroll 事件本身不算
 */
async function withEditorScrollFrozen(work, opts = {}) {
  const snap = captureEditorScroll();
  const scroller = snap.scroller;
  if (!scroller) {
    await work();
    return snap;
  }

  const desiredTop = Math.max(0, snap.scrollTop);
  appState.editorScrollFreezeTop = desiredTop;
  let userTookOver = false;
  let applying = false;

  const applyDesired = () => {
    if (userTookOver) return;
    const t = appState.editorScrollFreezeTop;
    if (t == null) return;
    applying = true;
    const max = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    // 显示层可钳位，但 desired 仍保留快照，等高回来后再还原
    const next = Math.min(Math.max(0, t), max);
    if (scroller.scrollTop !== next) scroller.scrollTop = next;
    requestAnimationFrame(() => {
      applying = false;
    });
  };

  const markUserTookOver = () => {
    if (applying || userTookOver) return;
    userTookOver = true;
    appState.editorScrollFreezeTop = scroller.scrollTop;
    noteEditorScrollIntent();
  };

  const onWheel = () => markUserTookOver();
  const onTouchMove = () => markUserTookOver();
  const onKeyDown = (e) => {
    if (
      e.key === "ArrowUp" ||
      e.key === "ArrowDown" ||
      e.key === "PageUp" ||
      e.key === "PageDown" ||
      e.key === "Home" ||
      e.key === "End" ||
      e.key === " "
    ) {
      markUserTookOver();
    }
  };

  scroller.addEventListener("wheel", onWheel, { passive: true });
  scroller.addEventListener("touchmove", onTouchMove, { passive: true });
  window.addEventListener("keydown", onKeyDown, true);

  const draftAnchor = measureDraftViewportAnchor(scroller);
  const alignRef = opts.alignBlockKeyRef || { current: opts.alignBlockKey || "" };

  try {
    await work();
    if (!userTookOver) {
      await settleFrames();
      applyDesired();
      await doubleRaf();
      applyDesired();
      const alignKey = alignRef.current || "";
      // 续写换壳：把新块顶对齐到原草稿在视口中的位置，避免落在「上一块底部」
      if (alignKey && draftAnchor) {
        applying = true;
        if (alignBlockToViewportOffset(scroller, alignKey, draftAnchor.viewOffset)) {
          appState.editorScrollFreezeTop = scroller.scrollTop;
        }
        requestAnimationFrame(() => {
          applying = false;
        });
      }
      await new Promise((r) => setTimeout(r, 40));
      applyDesired();
    }
  } finally {
    scroller.removeEventListener("wheel", onWheel);
    scroller.removeEventListener("touchmove", onTouchMove);
    window.removeEventListener("keydown", onKeyDown, true);
    if (!userTookOver) applyDesired();
    appState.editorScrollFreezeTop = null;
  }
  return snap;
}

/**
 * 换壳写入：先挂正文块，等布局后再清草稿，减少高度塌陷窗口
 * @param {() => string|void|Promise<string|void>} mutate 返回要对齐的 blockKey（可选）
 * @param {string} okMessage
 * @param {object} [job] 只清这一路；不传则清全部
 */
async function commitEditorWrite(mutate, okMessage, job = null) {
  const alignRef = { current: "" };
  await withEditorScrollFrozen(
    async () => {
      const key = await mutate();
      if (typeof key === "string" && key) alignRef.current = key;
      appState.dirty = true;
      await settleFrames();
      if (job) {
        job.accepted = true;
        clearJobPreview(job);
        refreshLegacyFromJobs();
      } else {
        clearDraftPreview();
      }
      await settleFrames();
      await saveAfterWrite(okMessage);
    },
    { alignBlockKeyRef: alignRef }
  );
  return alignRef.current;
}

function ensureBranchDoc() {
  syncBranchDocFromEditor();
  if (!appState.chapterBranchDoc) {
    appState.chapterBranchDoc = migrateBlocksToBranchDoc(ensureBlockList());
  }
  return appState.chapterBranchDoc;
}

function applyDocAndProject(doc) {
  applyBranchDoc(doc);
  appState.dirty = true;
}

/**
 * 将预览写入正文（默认自动调用）
 * @param {object} [jobOrNull] 指定 job；不传则用全局预览（兼容）
 * @returns {Promise<{ ok: boolean, error?: string }>}
 */
export async function acceptDraft(jobOrNull = null) {
  return enqueueAccept(() => acceptDraftInner(jobOrNull));
}

async function acceptDraftInner(jobOrNull) {
  const job = jobOrNull || null;
  if (job) {
    if (job.status === "pending" || job.status === "streaming") {
      return { ok: false, error: "还在生成中。" };
    }
    if (job.accepted) return { ok: true };
  } else if (appState.generating && activeJobCount() > 0) {
    return { ok: false, error: "还在生成中。" };
  }

  const body = draftBody(job);
  if (!body) return { ok: false, error: "没有可写入的正文。" };

  const task = (job ? job.draftTask : appState.draftTask) || "continue";
  const sel = ((job ? job.draftSelection : appState.draftSelection) || "").trim();
  const rewriteKey = (job ? job.draftRewriteBlockKey : appState.draftRewriteBlockKey) || "";
  const branchMode = (job ? job.draftBranchMode : appState.draftBranchMode) || "";
  const branchNodeId = (job ? job.draftBranchNodeId : appState.draftBranchNodeId) || "";
  const forkFromVariantId =
    (job ? job.draftForkFromVariantId : appState.draftForkFromVariantId) || "";
  const instruction =
    (job
      ? job.draftPersistInstruction || job.draftInstruction
      : appState.draftPersistInstruction || appState.draftInstruction) || "";
  const activateVariant = job ? job.activateVariant !== false : true;
  const u = job
    ? job.lastUsage
      ? { ...job.lastUsage }
      : {}
    : appState.lastUsage
      ? { ...appState.lastUsage }
      : {};
  const modelUsed = (job ? job.lastModelUsed : appState.lastModelUsed) || "";
  const logId = (job ? job.lastLogId : appState.lastLogId) || "";
  const costCny = job ? job.lastCostCny : appState.lastCostCny;
  const sources = job ? job.lastContextSources : appState.lastContextSources;
  const rid = (job && job.requestId) || appState.lastRequestId || "";

  if (task === "polish" && sel && !rewriteKey) {
    await pushAiUndo("润色写入");
    let polishedKey = "";
    let polishedText = "";
    await commitEditorWrite(() => {
      let replaced = false;
      const blocks = ensureBlockList();
      for (let i = 0; i < blocks.length; i++) {
        const t = blocks[i].text || "";
        if (t.includes(sel)) {
          const next = t.replace(sel, body);
          polishedKey = blocks[i].type === "gen" ? blocks[i].key : "";
          polishedText = next;
          blocks[i] = {
            ...blocks[i],
            text: next,
            chars: blocks[i].type === "gen" ? [...next].length : blocks[i].chars,
            digest: blocks[i].type === "gen" ? "" : blocks[i].digest,
          };
          replaced = true;
          break;
        }
      }
      if (!replaced) {
        appState.chapterContent = (appState.chapterContent || "").replace(sel, body);
        appState.chapterBlocks = [createPlainBlock(appState.chapterContent)];
        appState.chapterBranchDoc = migrateBlocksToBranchDoc(appState.chapterBlocks);
      } else {
        appState.chapterBlocks = blocks;
        appState.chapterContent = contentFromBlocks(blocks);
        syncBranchDocFromEditor();
      }
      return polishedKey;
    }, "润色已写入并保存", job);
    if (rid) acceptedRequestIds.add(rid);
    if (polishedKey) {
      void import("./blockDigest.js").then(async (m) => {
        await m.removeBlockNote(polishedKey);
        if (polishedText.trim()) {
          await m.runBlockDigest({
            blockKey: polishedKey,
            text: polishedText,
            instruction,
          });
        }
      });
      scheduleCastThenStorySync(polishedKey, polishedText, instruction);
    }
    return { ok: true };
  }

  const meta = {
    id: logId || undefined,
    task,
    model: modelUsed,
    chars: [...body].length,
    tokens:
      u.total_tokens ||
      (u.prompt_tokens || 0) + (u.completion_tokens || 0) ||
      undefined,
    cost: costCny || undefined,
    usageSource: u.source || undefined,
    instruction,
    sources,
  };

  // 同节点新变体
  if (branchMode === "variant" && branchNodeId) {
    await pushAiUndo("生成变体");
    let writtenKey = "";
    await commitEditorWrite(() => {
      const doc = ensureBranchDoc();
      const { doc: next, variant } = addVariant(
        doc,
        branchNodeId,
        variantFromGenBlock(createGenBlock(meta, body), {}),
        { activate: activateVariant }
      );
      writtenKey = variant?.key || "";
      applyDocAndProject(next);
      return writtenKey;
    }, "变体已写入并保存", job);
    if (rid) acceptedRequestIds.add(rid);
    if (writtenKey) {
      void import("./blockDigest.js").then((m) =>
        m.runBlockDigest({ blockKey: writtenKey, text: body, instruction })
      );
      scheduleCastThenStorySync(writtenKey, body, instruction);
    }
    return { ok: true };
  }

  // 从当前变体岔开子节点
  if (branchMode === "fork" && branchNodeId) {
    await pushAiUndo("岔开分支");
    let writtenKey = "";
    await commitEditorWrite(() => {
      const doc = ensureBranchDoc();
      const { doc: next, variant } = forkChild(
        doc,
        branchNodeId,
        forkFromVariantId || null,
        variantFromGenBlock(createGenBlock(meta, body), { label: "变体1" })
      );
      writtenKey = variant?.key || "";
      applyDocAndProject(next);
      return writtenKey;
    }, "分支已岔开并保存", job);
    if (rid) acceptedRequestIds.add(rid);
    if (writtenKey) {
      void import("./blockDigest.js").then((m) =>
        m.runBlockDigest({ blockKey: writtenKey, text: body, instruction })
      );
      scheduleCastThenStorySync(writtenKey, body, instruction);
    }
    return { ok: true };
  }

  if (rewriteKey) {
    await pushAiUndo("重写生成块");
    const hit = findNodeByBlockKey(ensureBranchDoc(), rewriteKey);
    if (hit) {
      let writtenKey = rewriteKey;
      await commitEditorWrite(() => {
        const doc = ensureBranchDoc();
        const next = replaceVariantText(
          doc,
          hit.node.id,
          hit.variant.id,
          createGenBlock(meta, body)
        );
        writtenKey = hit.variant.key;
        applyDocAndProject(next);
        return writtenKey;
      }, "已重写该段并保存", job);
      if (rid) acceptedRequestIds.add(rid);
      void import("./blockDigest.js").then(async (m) => {
        await m.removeBlockNote(writtenKey);
        await m.runBlockDigest({
          blockKey: writtenKey,
          text: body,
          instruction,
        });
      });
      scheduleCastThenStorySync(writtenKey, body, instruction);
      return { ok: true };
    }
    const blocks = ensureBlockList();
    const idx = blocks.findIndex((b) => b.key === rewriteKey);
    if (idx >= 0) {
      let writtenKey = rewriteKey;
      await commitEditorWrite(() => {
        const prev = blocks[idx];
        const nextKey = prev.key;
        writtenKey = nextKey;
        blocks[idx] = {
          ...createGenBlock(meta, body),
          key: nextKey,
          digest: "",
        };
        appState.chapterBlocks = blocks;
        appState.chapterContent = contentFromBlocks(blocks);
        appState.chapterBranchDoc = migrateBlocksToBranchDoc(blocks);
        return writtenKey;
      }, "已重写该段并保存", job);
      if (rid) acceptedRequestIds.add(rid);
      void import("./blockDigest.js").then(async (m) => {
        await m.removeBlockNote(writtenKey);
        await m.runBlockDigest({
          blockKey: writtenKey,
          text: body,
          instruction,
        });
      });
      scheduleCastThenStorySync(writtenKey, body, instruction);
      return { ok: true };
    }
  }

  const gen = createGenBlock(meta, body);
  await pushAiUndo("生成写入");
  let writtenKey = gen.key;
  await commitEditorWrite(() => {
    const doc = ensureBranchDoc();
    const { doc: next } = appendOnActivePath(
      doc,
      variantFromGenBlock(gen, { label: "变体1" })
    );
    applyDocAndProject(next);
    writtenKey = gen.key;
    return gen.key;
  }, "生成已写入并保存", job);
  if (rid) acceptedRequestIds.add(rid);
  if (job) job.lastWrittenBlockKey = writtenKey;
  void import("./blockDigest.js").then((m) =>
    m.runBlockDigest({
      blockKey: gen.key,
      text: body,
      instruction,
    })
  );
  scheduleCastThenStorySync(gen.key, body, instruction);
  return { ok: true, blockKey: writtenKey };
}

function scheduleDeferredAutoAccept(waitMs, job = null) {
  cancelDeferredAutoAccept();
  deferTimer = setTimeout(() => {
    deferTimer = null;
    if (job) void autoAcceptJobIfNeeded(job);
    else void autoAcceptDraftIfNeeded();
  }, Math.max(120, waitMs));
}

/** @deprecated 兼容：无 job 时走全局预览 */
export async function autoAcceptDraftIfNeeded() {
  const jobs = visibleGenJobs.value.filter((j) => j.status === "done" && !j.accepted);
  if (jobs.length) {
    for (const j of jobs) {
      await autoAcceptJobIfNeeded(j);
    }
    return;
  }
  if (appState.draftPlacement !== "editor") return;
  if (isBackgroundAnalysisTask(appState.draftTask)) return;
  if (activeJobCount() > 0) return;
  const rid = appState.lastRequestId || "";
  if (rid && acceptedRequestIds.has(rid)) {
    if (draftBody() || appState.draftPlacement === "editor") clearDraftPreview();
    return;
  }
  if (!draftBody()) {
    if (appState.draftPlacement === "editor") clearDraftPreview();
    return;
  }
  const sinceScroll = lastUserScrollAt ? Date.now() - lastUserScrollAt : SCROLL_IDLE_MS;
  if (sinceScroll < SCROLL_IDLE_MS) {
    const wait = SCROLL_IDLE_MS - sinceScroll + 40;
    appState.statusMessage = "生成完毕，阅读中…停滚后自动写入";
    scheduleDeferredAutoAccept(wait);
    return;
  }
  cancelDeferredAutoAccept();
  if (rid) acceptedRequestIds.add(rid);
  await acceptDraft(null);
}

/** 生成结束后按 job 自动写入 */
export async function autoAcceptJobIfNeeded(job) {
  if (!job || job.accepted) return;
  if (job.draftPlacement !== "editor") return;
  if (isBackgroundAnalysisTask(job.draftTask)) return;
  if (job.status === "pending" || job.status === "streaming") return;
  const rid = job.requestId || "";
  if (rid && acceptedRequestIds.has(rid)) {
    clearJobPreview(job);
    refreshLegacyFromJobs();
    return;
  }
  if (!draftBody(job)) {
    clearJobPreview(job);
    refreshLegacyFromJobs();
    return;
  }

  const sinceScroll = lastUserScrollAt ? Date.now() - lastUserScrollAt : SCROLL_IDLE_MS;
  if (sinceScroll < SCROLL_IDLE_MS) {
    const wait = SCROLL_IDLE_MS - sinceScroll + 40;
    appState.statusMessage = "生成完毕，阅读中…停滚后自动写入";
    scheduleDeferredAutoAccept(wait, job);
    return;
  }

  cancelDeferredAutoAccept();
  await acceptDraft(job);
}

export async function deleteGenBlock(blockKey) {
  if (!blockKey) return false;
  await pushAiUndo("删除生成块");
  const doc = ensureBranchDoc();
  const hit = findNodeByBlockKey(doc, blockKey);
  if (hit) {
    const { doc: next, removedKeys } = deleteVariantOrNode(
      doc,
      hit.node.id,
      hit.variant.id
    );
    applyDocAndProject(next);
    try {
      const { removeBlockNote } = await import("./blockDigest.js");
      for (const k of removedKeys) {
        await removeBlockNote(k);
      }
    } catch {
      /* ignore */
    }
  } else {
    const blocks = ensureBlockList().filter((b) => b.key !== blockKey);
    appState.chapterBlocks = blocks.length ? blocks : [createPlainBlock("")];
    appState.chapterContent = contentFromBlocks(appState.chapterBlocks);
    appState.chapterBranchDoc = migrateBlocksToBranchDoc(appState.chapterBlocks);
    appState.dirty = true;
    try {
      const { removeBlockNote } = await import("./blockDigest.js");
      await removeBlockNote(blockKey);
    } catch {
      /* ignore */
    }
  }
  appState.statusMessage = "已删除生成块（记忆已同步清除，未保存）";
  return true;
}

/** 组装写作请求时附带分支前缀 */
export function withBranchContext(request, mode, nodeId) {
  const doc = appState.chapterBranchDoc || migrateBlocksToBranchDoc(appState.chapterBlocks || []);
  const text = branchContextText(doc, mode || "continue", nodeId || "");
  return {
    ...request,
    branch_context_text: text || undefined,
  };
}

/**
 * 同位置变体：有本块指令则完全按指令从零重写；无指令才用上一节总结
 * 代码路径: kk_novel_ai/src/services/draftAccept.js
 * @param {string} baseInstr
 * @param {{ prevDigest?: string }} [opts]
 */
export function buildSameSlotVariantInstruction(baseInstr, opts = {}) {
  const base = String(baseInstr || "").trim();
  const prevDigest = String((opts && opts.prevDigest) || "").trim();
  const parts = [
    "【任务类型：同位置完全重写】不要参照、改写、扩写或模仿已有正文/旧变体；仅按本块创作指令从零重写本节。",
    "禁止续写后续、禁止推进到下一场；禁止把旧稿当底稿润色。",
    "须达到或超出规定字数，禁止短于目标。",
  ];
  if (base) {
    parts.push(
      "创作依据：仅使用下方「本块创作指令」。不要阅读、复述或沿用任何已有正文、旧变体或上一节总结。"
    );
    parts.push(`本块创作指令（须严格遵守，按此从零重写）：\n${base}`);
  } else if (prevDigest) {
    parts.push(
      "本块没有独立创作指令：请依据「上一节总结」在同一故事位置从零另写一节，人物与文风一致，细节可不同；仍禁止续写下一场。"
    );
    parts.push(`上一节总结：\n${prevDigest}`);
  } else {
    parts.push(
      "本块无指令、亦无上一节总结：在同位置从零另写一版，人物与文风一致，细节与走向可以不同；禁止续写下一场。"
    );
  }
  return parts.join("\n");
}

/**
 * 重写指定生成块：再跑一轮，结果替换该块
 * @param {string} blockKey
 * @param {{ instruction?: string }} [opts]
 */
export async function rewriteGenBlock(blockKey, opts = {}) {
  if (!blockKey || !appState.projectRoot || !appState.chapterId) {
    throw new Error("无法重写：缺少作品或章节");
  }
  if (!canStartMoreJobs(1)) {
    throw new Error(`最多同时 ${MAX_PARALLEL_GEN} 路生成，请等一路完成或取消`);
  }
  const block = (appState.chapterBlocks || []).find((b) => b.key === blockKey);
  if (!block || block.type !== "gen") {
    throw new Error("未找到该生成块");
  }
  if (appState.dirty) await saveChapter();

  const hit = findNodeByBlockKey(ensureBranchDoc(), blockKey);
  const nodeId = hit?.node?.id || "";
  const instr = String((opts && opts.instruction) || "").trim();
  const ownInstr = String(instr || block.instruction || "").trim();
  const prevDigest = ownInstr
    ? ""
    : previousSectionDigest(ensureBranchDoc(), nodeId);

  appState.draftPlacement = "editor";
  appState.draftTask = "same_slot_variant";
  appState.draftSelection = ownInstr ? "" : prevDigest;
  appState.draftRewriteBlockKey = blockKey;
  appState.draftAnchorBlockKey = blockKey;
  appState.draftBranchMode = "";
  appState.draftBranchNodeId = nodeId;
  appState.draftForkFromVariantId = "";
  appState.draftPersistInstruction =
    ownInstr || block.instruction || instr || "";
  appState.draftInstruction = buildSameSlotVariantInstruction(ownInstr, {
    prevDigest,
  });
  appState.pendingScrollBlockKey = blockKey;

  await runWriting({
    project_root: appState.projectRoot,
    chapter_id: appState.chapterId,
    task: "same_slot_variant",
    instruction: appState.draftInstruction,
    selection: appState.draftSelection,
    branch_context_text: "",
  });
}

/**
 * 同节点再生成变体（不覆盖当前）；count>1 时并发生成多路
 * @param {string} blockKey
 * @param {{ instruction?: string, count?: number, activateVariant?: boolean }} [opts]
 */
export async function generateVariantBlock(blockKey, opts = {}) {
  if (!blockKey || !appState.projectRoot || !appState.chapterId) {
    throw new Error("无法生成变体：缺少作品或章节");
  }
  const want = Math.max(1, Math.min(MAX_PARALLEL_GEN, Number(opts.count) || 1));
  if (!canStartMoreJobs(want)) {
    throw new Error(
      `最多同时 ${MAX_PARALLEL_GEN} 路（当前已有 ${activeJobCount()}），请等一路完成或取消`
    );
  }
  const hit = findNodeByBlockKey(ensureBranchDoc(), blockKey);
  if (!hit) throw new Error("未找到该生成块节点");
  if (appState.dirty) await saveChapter();

  const instr = String((opts && opts.instruction) || "").trim();
  const ownInstr = String(instr || hit.variant.instruction || "").trim();
  const prevDigest = ownInstr
    ? ""
    : previousSectionDigest(ensureBranchDoc(), hit.node.id);

  appState.draftPlacement = "editor";
  appState.draftTask = "same_slot_variant";
  appState.draftSelection = ownInstr ? "" : prevDigest;
  appState.draftRewriteBlockKey = "";
  appState.draftAnchorBlockKey = blockKey;
  appState.draftBranchMode = "variant";
  appState.draftBranchNodeId = hit.node.id;
  appState.draftForkFromVariantId = "";
  appState.draftPersistInstruction = ownInstr || hit.variant.instruction || "";
  appState.draftInstruction = buildSameSlotVariantInstruction(ownInstr, {
    prevDigest,
  });
  appState.pendingScrollBlockKey = blockKey;

  const request = {
    project_root: appState.projectRoot,
    chapter_id: appState.chapterId,
    task: "same_slot_variant",
    instruction: appState.draftInstruction,
    selection: appState.draftSelection,
    branch_context_text: "",
  };

  if (want === 1) {
    await runWriting(request, {
      label: "变体",
      activateVariant: opts.activateVariant !== false,
    });
    return;
  }

  const tasks = [];
  for (let i = 0; i < want; i++) {
    const job = createGenJob({
      label: `变体${i + 1}`,
      activateVariant: i === want - 1,
    });
    tasks.push(
      runWriting(request, {
        job,
        label: job.label,
        activateVariant: i === want - 1,
      })
    );
  }
  const results = await Promise.allSettled(tasks);
  const failed = results.filter((r) => r.status === "rejected");
  if (failed.length === want) {
    throw failed[0].reason || new Error("并发生成全部失败");
  }
  if (failed.length) {
    appState.statusMessage = `并发生成：${want - failed.length} 路成功，${failed.length} 路失败`;
  }
}

/**
 * 从当前变体岔开：生成子节点
 * @param {string} blockKey
 * @param {{ instruction?: string }} [opts]
 */
export async function forkFromBlock(blockKey, opts = {}) {
  if (!blockKey || !appState.projectRoot || !appState.chapterId) {
    throw new Error("无法岔开：缺少作品或章节");
  }
  if (!canStartMoreJobs(1)) {
    throw new Error(`最多同时 ${MAX_PARALLEL_GEN} 路生成，请等一路完成或取消`);
  }
  const hit = findNodeByBlockKey(ensureBranchDoc(), blockKey);
  if (!hit) throw new Error("未找到该生成块节点");
  if (appState.dirty) await saveChapter();

  const instr = String((opts && opts.instruction) || "").trim();
  appState.draftPlacement = "editor";
  appState.draftTask = "continue";
  appState.draftSelection = "";
  appState.draftRewriteBlockKey = "";
  appState.draftAnchorBlockKey = blockKey;
  appState.draftBranchMode = "fork";
  appState.draftBranchNodeId = hit.node.id;
  appState.draftForkFromVariantId = hit.variant.id;
  appState.draftInstruction =
    instr ||
    "从此处岔开续写：承接上文，写出与主线可并行的下一段。禁止复述前文已写句子。";
  appState.pendingScrollBlockKey = blockKey;

  await runWriting(
    withBranchContext(
      {
        project_root: appState.projectRoot,
        chapter_id: appState.chapterId,
        task: "continue",
        instruction: appState.draftInstruction,
        selection: "",
      },
      "fork",
      hit.node.id
    )
  );
}

/**
 * 润色指定生成块：用 polish 任务，结果替换该块
 * @param {string} blockKey
 * @param {{ instruction?: string }} [opts]
 */
export async function polishGenBlock(blockKey, opts = {}) {
  if (!blockKey || !appState.projectRoot || !appState.chapterId) {
    throw new Error("无法润色：缺少作品或章节");
  }
  if (!canStartMoreJobs(1)) {
    throw new Error(`最多同时 ${MAX_PARALLEL_GEN} 路生成，请等一路完成或取消`);
  }
  const block = (appState.chapterBlocks || []).find((b) => b.key === blockKey);
  if (!block || block.type !== "gen") {
    throw new Error("未找到该生成块");
  }
  if (appState.dirty) await saveChapter();

  const instr = String((opts && opts.instruction) || "").trim();
  appState.draftPlacement = "editor";
  appState.draftTask = "polish";
  appState.draftSelection = block.text || "";
  appState.draftRewriteBlockKey = blockKey;
  appState.draftAnchorBlockKey = blockKey;
  appState.draftBranchMode = "";
  appState.draftBranchNodeId = "";
  appState.pendingScrollBlockKey = blockKey;
  appState.draftForkFromVariantId = "";
  appState.draftInstruction =
    instr || "润色本段：保持情节与人物不变，理顺语句，增强画面感，不要扩写新情节。";

  await runWriting({
    project_root: appState.projectRoot,
    chapter_id: appState.chapterId,
    task: "polish",
    instruction: appState.draftInstruction,
    selection: block.text || "",
  });
}

/**
 * 切换变体并刷新编辑器投影
 */
export function switchBlockVariant(nodeId, variantId) {
  if (!nodeId || !variantId) return;
  const doc = ensureBranchDoc();
  applyDocAndProject(switchVariant(doc, nodeId, variantId));
  appState.statusMessage = "已切换变体";
}
