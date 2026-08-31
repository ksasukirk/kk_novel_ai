<!--
  写作编辑器 + Ctrl+K 行内幽灵文本（分块 UI）
  代码路径: kk_novel_ai/src/views/EditorView.vue
-->
<script setup>
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import * as project from "../services/projectClient.js";
import { runWriting, cancelGeneration, saveSettings } from "../services/llmClient.js";
import { pushAiUndo, undoLastAi } from "../services/aiUndo.js";
import { refreshCharacterNameIndex } from "../services/characterIndex.js";
import AiPanel from "../components/AiPanel.vue";
import ChapterBlockEditor from "../components/ChapterBlockEditor.vue";
import ContinuousChapterRead from "../components/ContinuousChapterRead.vue";
import EditorDraftPreview from "../components/EditorDraftPreview.vue";
import { isTrailingEditorDraft, noteEditorUserScroll, noteEditorScrollIntent, rejectDraft, deleteGenBlock, withBranchContext, trailingDraftJobs, anchoredJobsFor } from "../services/draftAccept.js";
import { visibleGenJobs } from "../stores/genJobs.js";
import { appConfirmDelete } from "../services/confirmDialog.js";
import {
  blocksFromContent,
  contentFromBlocks,
  createPlainBlock,
  genBlocksToc,
} from "../utils/genBlock.js";
import {
  activatePathToNode,
  branchTocTree,
  migrateBlocksToBranchDoc,
} from "../utils/branchModel.js";
import { applyBranchDoc, syncBranchDocFromEditor } from "../services/projectClient.js";
import { switchBlockVariant } from "../services/draftAccept.js";
import BranchTreePanel from "../components/BranchTreePanel.vue";
import {
  EDITOR_FONT_PRESETS,
  EDITOR_FONT_SIZES,
  DEFAULT_EDITOR_FONT_SIZE,
  presetIdFromSettings,
  applyEditorTypography,
} from "../utils/editorTypography.js";
import {
  activePathKey,
  applyScrollerProgress,
  getChapterProgress,
  saveChapterProgress,
} from "../services/editorReadingProgress.js";
import { isMobileUx, watchMobileViewport } from "../utils/platform.js";
import {
  aiPanelLayoutButtonLabel,
  cycleAiPanelLayout,
  readAiPanelLayout,
  readEditorTocVisible,
  saveAiPanelLayout,
  saveEditorTocVisible,
} from "../utils/layoutPrefs.js";
import {
  chapterQueueStatus,
  chapterQueueStatusClass,
  chapterQueueStatusLabel,
  isChapterBodyEmpty,
} from "../utils/chapterStatus.js";
import {
  runContinueOutline,
  runFullOutlinePipeline,
  runSingleChapterOutline,
} from "../services/bookOutlineQueue.js";
import { outlineQueueState } from "../services/outlineQueue.js";
import { invoke } from "../services/tauri.js";
import { appConfirm } from "../services/confirmDialog.js";
import { updateChapterMeta } from "../services/projectClient.js";
import { aiPanelForm } from "../stores/aiPanelState.js";

const mobileUx = ref(isMobileUx());
const tocDrawerOpen = ref(false);
const tocVisible = ref(readEditorTocVisible());
const aiPanelLayout = ref(readAiPanelLayout());

const isAiHidden = computed(() => aiPanelLayout.value === "hidden");
const isAiFloat = computed(
  () => !isAiHidden.value && (mobileUx.value || aiPanelLayout.value === "float")
);
const aiLayoutButtonLabel = computed(() => aiPanelLayoutButtonLabel(aiPanelLayout.value));

function toggleTocVisible() {
  if (mobileUx.value) {
    tocDrawerOpen.value = !tocDrawerOpen.value;
    return;
  }
  tocVisible.value = !tocVisible.value;
  saveEditorTocVisible(tocVisible.value);
}

function setAiPanelLayout(layout) {
  aiPanelLayout.value = layout;
  saveAiPanelLayout(layout);
}

/** 工具栏：侧栏 → 浮条 → 隐藏 → 侧栏 */
function toggleAiPanelLayout() {
  if (mobileUx.value) {
    setAiPanelLayout("float");
    return;
  }
  setAiPanelLayout(cycleAiPanelLayout(aiPanelLayout.value));
}

/** AI 面板内：侧栏 ↔ 浮条（不进入 hidden） */
function onAiPanelToggleLayout() {
  if (mobileUx.value) {
    setAiPanelLayout("float");
    return;
  }
  if (isAiHidden.value || aiPanelLayout.value === "float") {
    setAiPanelLayout("dock");
  } else {
    setAiPanelLayout("float");
  }
}

const newTitle = ref("");
const error = ref("");
const blockEditor = ref(null);

/** v-for 内挂载编辑器：避免 ref 变成数组 */
function setBlockEditorRef(el) {
  blockEditor.value = el || null;
}
const showInline = ref(false);
const inlinePrompt = ref("");
const caret = ref({ index: 0, start: 0, end: 0 });
const ghostText = ref("");
const ghostActive = ref(false);
const ghostBlockIndex = ref(0);
const ghostOffset = ref(0);
const inlineBusy = ref(false);
const fontPresets = EDITOR_FONT_PRESETS;
const fontSizes = EDITOR_FONT_SIZES;
const showBranchGraph = ref(true);

/** chapterId -> 是否展开 */
const expanded = reactive({});
/** chapterId -> toc items (flat or branch) */
const tocByChapter = reactive({});
/** chapterId -> branch toc (仅当前章详细) */
const branchTocByChapter = reactive({});
const tocLoading = reactive({});
/** chapterId -> 正文是否为空（待写判定） */
const bodyEmptyByChapter = reactive({});
const editingSummaryId = ref("");
const editingSummaryDraft = ref("");
const tocQueueBusy = ref(false);
const activeBlockKey = ref("");
/** 滚动阅读焦点章（可与正在编辑章不同） */
const tocFocusChapterId = ref(appState.chapterId || "");
/** chapterId -> 邻章只读块缓存（连续阅读） */
const chapterBodyCache = reactive({});
/** 程序滚动 TOC / 冻结写回时跳过 spy，避免抢激活 */
let suppressTocSpyUntil = 0;
let tocSpyRaf = 0;
/** 阅读进度防抖写入 */
let progressSaveTimer = 0;
/** 避免同一次切章重复恢复 */
let restoringProgress = false;
/** 邻章正文预载去重 */
let preloadBodiesToken = 0;

function escapeAttrSelector(value) {
  const s = String(value || "");
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(s);
  }
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

/**
 * @param {Element | null | undefined} scroller
 * @param {string} chapterId
 * @returns {HTMLElement | null}
 */
function findChapterSection(scroller, chapterId) {
  if (!scroller || !chapterId) return null;
  return scroller.querySelector(
    `.continuous-chapter[data-chapter-id="${escapeAttrSelector(chapterId)}"]`
  );
}

/**
 * 相对章顶的阅读进度（连续叠章后绝对 scrollTop 会串章）
 * @param {Element | null | undefined} scroller
 * @param {string} chapterId
 * @param {string} [blockKey]
 */
function captureRelativeChapterProgress(scroller, chapterId, blockKey = "") {
  const sec = findChapterSection(scroller, chapterId);
  const base = sec && typeof sec.offsetTop === "number" ? sec.offsetTop : 0;
  return {
    scrollTop: Math.max(0, (scroller && scroller.scrollTop) || 0) - base,
    blockKey: String(blockKey || ""),
  };
}

/**
 * @param {Element | null | undefined} scroller
 * @param {string} chapterId
 * @param {number} scrollTop
 */
function applyRelativeChapterProgress(scroller, chapterId, scrollTop) {
  const sec = findChapterSection(scroller, chapterId);
  const base = sec && typeof sec.offsetTop === "number" ? sec.offsetTop : 0;
  applyScrollerProgress(scroller, base + Math.max(0, Number(scrollTop) || 0));
}

function snapshotBlocks(blocks) {
  if (!Array.isArray(blocks)) return [];
  return blocks.map((b) => (b && typeof b === "object" ? { ...b } : b));
}

function cacheCurrentChapterBody() {
  const id = appState.chapterId;
  if (!id) return;
  chapterBodyCache[id] = snapshotBlocks(appState.chapterBlocks);
}

/**
 * 预载全书邻章正文，供连续滚动阅读
 */
async function preloadChapterBodies() {
  if (!appState.projectRoot) return;
  const token = ++preloadBodiesToken;
  const list = chapters.value;
  for (const ch of list) {
    if (token !== preloadBodiesToken) return;
    if (!ch?.id) continue;
    if (ch.id === appState.chapterId) {
      chapterBodyCache[ch.id] = snapshotBlocks(appState.chapterBlocks);
      continue;
    }
    try {
      const blocks = await project.peekChapterBlocks(ch.id);
      if (token !== preloadBodiesToken) return;
      chapterBodyCache[ch.id] = snapshotBlocks(blocks);
    } catch {
      if (token !== preloadBodiesToken) return;
      chapterBodyCache[ch.id] = [];
    }
  }
}

function scrollChapterSectionToTop(chapterId) {
  const scroller = getEditorScroller();
  const sec = findChapterSection(scroller, chapterId);
  if (!scroller || !sec) return;
  scroller.scrollTop = sec.offsetTop;
}

const fontFamily = computed({
  get() {
    return presetIdFromSettings(appState.settings || {});
  },
  set(v) {
    void patchTypography({ editor_font_family: v });
  },
});

const fontSize = computed({
  get() {
    const n = Number(appState.settings && appState.settings.editor_font_size);
    return Number.isFinite(n) && n >= 10 ? n : DEFAULT_EDITOR_FONT_SIZE;
  },
  set(v) {
    void patchTypography({ editor_font_size: Number(v) || DEFAULT_EDITOR_FONT_SIZE });
  },
});

async function patchTypography(partial) {
  error.value = "";
  try {
    const base = { ...(appState.settings || {}) };
    const next = { ...base, ...partial };
    await saveSettings(next);
    applyEditorTypography(next);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

const chapters = computed(() => (appState.project && appState.project.chapters) || []);
const wordCount = computed(() => (appState.chapterContent || "").replace(/\s/g, "").length);
const showEditorDraft = computed(() => trailingDraftJobs().length > 0 || isTrailingEditorDraft());
const trailingJobs = computed(() => trailingDraftJobs());
/** Esc 取消：任一路编辑区草稿 */
const anyEditorDraft = computed(
  () =>
    visibleGenJobs.value.length > 0 ||
    isTrailingEditorDraft() ||
    !!(appState.draftAnchorBlockKey && appState.draftPlacement === "editor")
);

function ensureBlocks() {
  const raw = appState.chapterContent || "";
  if (/<!--\s*kk-gen\b/i.test(raw)) {
    const migrated = blocksFromContent(raw);
    applyBranchDoc(migrateBlocksToBranchDoc(migrated));
    appState.dirty = true;
    return;
  }
  if (!appState.chapterBranchDoc) {
    if (!Array.isArray(appState.chapterBlocks) || !appState.chapterBlocks.length) {
      applyBranchDoc(migrateBlocksToBranchDoc([createPlainBlock(raw)]));
    } else {
      applyBranchDoc(migrateBlocksToBranchDoc(appState.chapterBlocks));
    }
    return;
  }
  if (!Array.isArray(appState.chapterBlocks) || !appState.chapterBlocks.length) {
    applyBranchDoc(appState.chapterBranchDoc);
  }
}

function onCaret(info) {
  caret.value = info;
  const list = appState.chapterBlocks || [];
  const b = list[info?.index];
  if (b && b.type === "gen" && b.key) {
    setActiveBlockKey(b.key, { scrollToc: true });
  }
}

function getEditorScroller() {
  return document.querySelector(".editor-scroll");
}

/** 保存当前焦点章（及激活分支路径）阅读进度 */
function saveReadingProgress(chapterId = tocFocusChapterId.value || appState.chapterId) {
  if (!chapterId || !appState.projectRoot) return;
  if (appState.editorScrollFreezeTop != null) return;
  if (restoringProgress) return;
  const scroller = getEditorScroller();
  if (!scroller) return;
  saveChapterProgress(
    appState.projectRoot,
    chapterId,
    captureRelativeChapterProgress(scroller, chapterId, activeBlockKey.value || ""),
    chapterId === appState.chapterId ? activePathKey(appState.chapterBranchDoc) : ""
  );
}

function scheduleSaveReadingProgress() {
  if (Date.now() < suppressTocSpyUntil) return;
  if (appState.editorScrollFreezeTop != null) return;
  if (restoringProgress) return;
  if (!appState.chapterId || !appState.projectRoot) return;
  if (progressSaveTimer) clearTimeout(progressSaveTimer);
  progressSaveTimer = window.setTimeout(() => {
    progressSaveTimer = 0;
    saveReadingProgress();
  }, 180);
}

/**
 * 恢复某章阅读进度：优先精确 scrollTop，块 key 仍在则同步目录高亮
 * @param {string} chapterId
 * @param {{ pathKey?: string }} [opts]
 */
async function restoreReadingProgress(chapterId, opts = {}) {
  if (!chapterId || !appState.projectRoot) return;
  const pathKey =
    opts.pathKey != null ? opts.pathKey : activePathKey(appState.chapterBranchDoc);
  const prog = getChapterProgress(appState.projectRoot, chapterId, pathKey);
  if (!prog) {
    tocFocusChapterId.value = chapterId;
    scrollChapterSectionToTop(chapterId);
    return;
  }

  restoringProgress = true;
  suppressTocSpyUntil = Date.now() + 560;
  tocFocusChapterId.value = chapterId;
  try {
    await nextTick();
    await nextTick();
    const scroller = getEditorScroller();
    const blocks =
      chapterId === appState.chapterId
        ? appState.chapterBlocks || []
        : chapterBodyCache[chapterId] || [];
    const key = prog.blockKey;
    const hasKey = !!(key && blocks.some((b) => b && b.key === key));
    if (hasKey) {
      setActiveBlockKey(key, { scrollToc: true, chapterId });
    }

    const apply = () => {
      if (hasKey && scroller) {
        const el = scroller.querySelector(
          `.continuous-chapter[data-chapter-id="${escapeAttrSelector(chapterId)}"] .chapter-block[data-block-key="${escapeAttrSelector(key)}"]`
        );
        if (el) {
          const rootRect = scroller.getBoundingClientRect();
          const r = el.getBoundingClientRect();
          scroller.scrollTop += r.top - rootRect.top - 8;
        } else {
          applyRelativeChapterProgress(scroller, chapterId, prog.scrollTop);
        }
      } else {
        applyRelativeChapterProgress(scroller, chapterId, prog.scrollTop);
      }
      if (hasKey) setActiveBlockKey(key, { scrollToc: true, chapterId });
    };
    apply();
    await nextTick();
    requestAnimationFrame(() => {
      apply();
      window.setTimeout(apply, 40);
    });
  } finally {
    window.setTimeout(() => {
      restoringProgress = false;
    }, 80);
  }
}

/**
 * 根据编辑区滚动位置，激活「阅读线」上对应的章 + 生成块目录项
 */
function syncActiveBlockFromScroll() {
  if (Date.now() < suppressTocSpyUntil) return;
  if (appState.editorScrollFreezeTop != null) return;
  if (restoringProgress) return;
  if (appState.activeNav !== "editor") return;
  const scroller = getEditorScroller();
  if (!scroller) return;
  const sections = scroller.querySelectorAll(".continuous-chapter[data-chapter-id]");
  const rootRect = scroller.getBoundingClientRect();
  // 略低于顶栏/吸顶条，当作当前「正在看」的位置
  const probeY = rootRect.top + Math.min(120, Math.max(48, rootRect.height * 0.18));

  let focusCh = "";
  for (const sec of sections) {
    const id = sec.getAttribute("data-chapter-id") || "";
    if (!id) continue;
    const r = sec.getBoundingClientRect();
    if (r.top <= probeY + 10) focusCh = id;
  }
  if (!focusCh && sections.length) {
    focusCh = sections[0].getAttribute("data-chapter-id") || "";
  }
  if (focusCh && focusCh !== tocFocusChapterId.value) {
    tocFocusChapterId.value = focusCh;
    expanded[focusCh] = true;
    void loadTocForChapter(focusCh);
    nextTick(() => {
      const row = document.querySelector(".toc-chapter-row.active");
      if (row && typeof row.scrollIntoView === "function") {
        row.scrollIntoView({ block: "nearest", inline: "nearest" });
      }
    });
  }

  const scope = focusCh ? findChapterSection(scroller, focusCh) : null;
  const nodes = (scope || scroller).querySelectorAll(".chapter-block.is-gen[data-block-key]");
  if (!nodes.length) {
    if (activeBlockKey.value) activeBlockKey.value = "";
    return;
  }
  let key = "";
  for (const el of nodes) {
    const k = el.getAttribute("data-block-key") || "";
    if (!k) continue;
    const r = el.getBoundingClientRect();
    if (r.top <= probeY + 10) key = k;
  }
  if (!key) {
    key = nodes[0].getAttribute("data-block-key") || "";
  }
  if (key) setActiveBlockKey(key, { scrollToc: true, chapterId: focusCh });
}

function setActiveBlockKey(key, opts = {}) {
  if (!key || key === activeBlockKey.value) return;
  activeBlockKey.value = key;
  const chapterId = opts.chapterId || tocFocusChapterId.value || appState.chapterId;
  if (chapterId) expanded[chapterId] = true;
  if (opts.scrollToc) {
    nextTick(() => {
      const row = document.querySelector(".toc-block-row.active");
      if (row && typeof row.scrollIntoView === "function") {
        row.scrollIntoView({ block: "nearest", inline: "nearest" });
      }
    });
  }
}

function isExpanded(chapterId) {
  if (expanded[chapterId] === undefined) {
    return (
      chapterId === tocFocusChapterId.value || chapterId === appState.chapterId
    );
  }
  return !!expanded[chapterId];
}

function toggleExpand(chapterId, ev) {
  if (ev) {
    ev.preventDefault();
    ev.stopPropagation();
  }
  const next = !isExpanded(chapterId);
  expanded[chapterId] = next;
  if (next) void loadTocForChapter(chapterId);
}

function syncCurrentToc() {
  const id = appState.chapterId;
  if (!id) return;
  if (appState.chapterBranchDoc) {
    branchTocByChapter[id] = branchTocTree(appState.chapterBranchDoc);
    tocByChapter[id] = genBlocksToc(appState.chapterBlocks || []);
  } else {
    tocByChapter[id] = genBlocksToc(appState.chapterBlocks || []);
    branchTocByChapter[id] = [];
  }
  if (expanded[id] === undefined) expanded[id] = true;
  syncCurrentBodyEmpty();
}

function syncCurrentBodyEmpty() {
  const id = appState.chapterId;
  if (!id) return;
  const ch = chapters.value.find((c) => c.id === id);
  bodyEmptyByChapter[id] = isChapterBodyEmpty(
    appState.chapterContent || "",
    (ch && ch.title) || ""
  );
}

/**
 * @param {object} ch
 */
function tocStatusOf(ch) {
  if (!ch) return "writing";
  const known = Object.prototype.hasOwnProperty.call(bodyEmptyByChapter, ch.id);
  return chapterQueueStatus(ch, {
    bodyEmpty: known ? bodyEmptyByChapter[ch.id] : undefined,
  });
}

function tocStatusLabel(ch) {
  return chapterQueueStatusLabel(tocStatusOf(ch));
}

function tocStatusClass(ch) {
  return chapterQueueStatusClass(tocStatusOf(ch));
}

async function refreshBodyEmpty(chapterId) {
  if (!chapterId || !appState.projectRoot) return;
  if (chapterId === appState.chapterId) {
    syncCurrentBodyEmpty();
    return;
  }
  const ch = chapters.value.find((c) => c.id === chapterId);
  try {
    const r = await invoke("chapter_read", {
      root: appState.projectRoot,
      chapterId,
    });
    bodyEmptyByChapter[chapterId] = isChapterBodyEmpty(
      r.content || "",
      (ch && ch.title) || ""
    );
  } catch {
    /* 保持未知 */
  }
}

function startEditSummary(ch, ev) {
  if (ev) {
    ev.preventDefault();
    ev.stopPropagation();
  }
  if (!ch) return;
  editingSummaryId.value = ch.id;
  editingSummaryDraft.value = String(ch.summary || "");
}

async function saveEditSummary(ch) {
  if (!ch || editingSummaryId.value !== ch.id) return;
  const summary = String(editingSummaryDraft.value || "").trim();
  error.value = "";
  try {
    const patch = { summary };
    if (summary && (!ch.status || ch.status === "draft")) {
      patch.status = "pending";
    }
    await updateChapterMeta(ch.id, patch);
    editingSummaryId.value = "";
    appState.statusMessage = "章纲已保存";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function cancelEditSummary() {
  editingSummaryId.value = "";
  editingSummaryDraft.value = "";
}

async function onTocWriteChapter(ch, ev) {
  if (ev) {
    ev.preventDefault();
    ev.stopPropagation();
  }
  if (!ch) return;
  error.value = "";
  tocQueueBusy.value = true;
  try {
    if (ch.id !== appState.chapterId) await selectChapter(ch.id);
    await runSingleChapterOutline(ch.id, {
      instruction: aiPanelForm.instruction,
    });
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    tocQueueBusy.value = false;
  }
}

async function onTocContinueSplit() {
  error.value = "";
  tocQueueBusy.value = true;
  try {
    const r = await runContinueOutline({
      instruction: aiPanelForm.instruction,
    });
    appState.statusMessage = `已续拆追加 ${
      (r.createdIds || []).length
    } 章到目录`;
    await refreshAllTocs();
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    tocQueueBusy.value = false;
  }
}

async function onTocWriteAll() {
  error.value = "";
  const ok = await appConfirm("将按目录中待写章纲依次生成正文，确认开始？", {
    title: "全部按纲写",
    confirmText: "开始",
    cancelText: "取消",
  });
  if (!ok) return;
  tocQueueBusy.value = true;
  try {
    await runFullOutlinePipeline({
      instruction: aiPanelForm.instruction,
    });
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    tocQueueBusy.value = false;
  }
}

function currentBranchToc(chapterId) {
  if (chapterId === appState.chapterId && appState.chapterBranchDoc) {
    return branchTocTree(appState.chapterBranchDoc);
  }
  return branchTocByChapter[chapterId] || [];
}

/** 某生成块是否正被改写 / 锚定生成 */
function isBlockGenerating(blockKey) {
  if (!blockKey) return false;
  if (anchoredJobsFor(blockKey).length) return true;
  return visibleGenJobs.value.some(
    (j) =>
      (j.status === "pending" || j.status === "streaming" || (j.status === "done" && !j.accepted)) &&
      (j.draftRewriteBlockKey === blockKey || j.draftAnchorBlockKey === blockKey)
  );
}

/**
 * 尚未落盘的「正在生成」小节占位（按纲节拍 / 尾部草稿）
 * @param {string} chapterId
 */
function tocGeneratingPhantoms(chapterId) {
  if (!chapterId) return [];
  /** @type {Array<{kind:string,key:string,label:string,generating:boolean,genIndex:number,depth:number,active?:boolean}>} */
  const out = [];
  const oq = outlineQueueState;
  if (
    oq.running &&
    oq.chapterId === chapterId &&
    (oq.phase === "writing" || oq.phase === "splitting")
  ) {
    const label =
      oq.phase === "splitting"
        ? "正在拆分节拍…"
        : oq.beatTitle
          ? oq.beatTitle
          : oq.beatIndex
            ? `节拍 ${oq.beatIndex}/${oq.beatTotal || "?"}`
            : "正在生成…";
    out.push({
      kind: "generating",
      key: `__generating_outline_${oq.phase}_${oq.beatIndex || 0}`,
      label,
      generating: true,
      genIndex: -1,
      depth: 0,
      active: true,
    });
    return out;
  }

  if (chapterId !== appState.chapterId) return out;
  for (const job of visibleGenJobs.value) {
    if (job.draftAnchorBlockKey || job.draftRewriteBlockKey) continue;
    if (!(job.status === "pending" || job.status === "streaming" || (job.status === "done" && !job.accepted))) {
      continue;
    }
    out.push({
      kind: "generating",
      key: `__generating_job_${job.id}`,
      label: String(job.label || "生成中").trim() || "生成中",
      generating: true,
      genIndex: -1,
      depth: 0,
      active: true,
    });
  }
  return out;
}

/**
 * 目录小节行：已有块 + 生成中占位，并给正在改写的块打标
 * @param {{ id: string }} ch
 */
function tocRowsForChapter(ch) {
  if (!ch?.id) return [];
  const chapterId = ch.id;
  let rows = [];
  if (chapterId === appState.chapterId && currentBranchToc(chapterId).length) {
    rows = currentBranchToc(chapterId).map((item) => ({
      ...item,
      generating: !!(item.key && isBlockGenerating(item.key)),
    }));
  } else {
    rows = tocItems(chapterId).map((item) => ({
      kind: "section",
      key: item.key,
      label: item.label,
      genIndex: item.genIndex,
      depth: 0,
      active: true,
      generating: !!(item.key && isBlockGenerating(item.key)),
    }));
  }
  const phantoms = tocGeneratingPhantoms(chapterId);
  return [...rows, ...phantoms];
}

function scrollTocGeneratingIntoView() {
  nextTick(() => {
    const row = document.querySelector(".toc-block-row.is-generating");
    if (row && typeof row.scrollIntoView === "function") {
      row.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
  });
}

/** 点击生成中占位：切到该章并滚到草稿区 */
async function onTocGeneratingClick(chapterId) {
  error.value = "";
  try {
    if (chapterId && chapterId !== appState.chapterId) {
      await selectChapter(chapterId, { skipRestore: true, preserveViewport: true });
    }
    tocFocusChapterId.value = chapterId;
    expanded[chapterId] = true;
    await nextTick();
    const scroller = getEditorScroller();
    if (!scroller) return;
    const draft = scroller.querySelector(".editor-draft");
    if (draft) {
      const rootRect = scroller.getBoundingClientRect();
      const r = draft.getBoundingClientRect();
      scroller.scrollTop += r.top - rootRect.top - 24;
    } else {
      scroller.scrollTop = scroller.scrollHeight;
    }
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function selectTocItem(chapterId, item) {
  error.value = "";
  if (!item) return;
  try {
    if (chapterId !== appState.chapterId) {
      await selectChapter(chapterId, { skipRestore: true, preserveViewport: true });
      await nextTick();
    } else {
      saveReadingProgress(chapterId);
    }
    if (item.kind === "variant" && item.nodeId && item.variantId) {
      switchBlockVariant(item.nodeId, item.variantId);
    } else if (item.kind === "branchHint" && item.parentNodeId && item.parentVariantId) {
      switchBlockVariant(item.parentNodeId, item.parentVariantId);
      await nextTick();
      if (item.nodeId) {
        applyBranchDoc(activatePathToNode(appState.chapterBranchDoc, item.nodeId));
      }
    } else if (item.nodeId && appState.chapterBranchDoc) {
      applyBranchDoc(activatePathToNode(appState.chapterBranchDoc, item.nodeId));
    }
    const key =
      item.key ||
      (appState.chapterBlocks || []).find((b) => b._nodeId === item.nodeId)?.key ||
      "";
    if (key) await selectBlock(chapterId, key);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function onBranchGraphSelect(blockKey) {
  if (!blockKey || !appState.chapterId) return;
  void selectBlock(appState.chapterId, blockKey);
  syncCurrentToc();
}

async function loadTocForChapter(chapterId) {
  if (!chapterId || !appState.projectRoot) return;
  if (chapterId === appState.chapterId) {
    syncCurrentToc();
    return;
  }
  if (tocLoading[chapterId]) return;
  tocLoading[chapterId] = true;
  try {
    const blocks = await project.peekChapterBlocks(chapterId);
    tocByChapter[chapterId] = genBlocksToc(blocks);
    await refreshBodyEmpty(chapterId);
  } catch (e) {
    tocByChapter[chapterId] = [];
    error.value = String(e.message || e);
  } finally {
    tocLoading[chapterId] = false;
  }
}

async function refreshAllTocs() {
  const list = chapters.value;
  await Promise.all(
    list.map(async (ch) => {
      if (ch.id === appState.chapterId) {
        syncCurrentToc();
        return;
      }
      await loadTocForChapter(ch.id);
      await refreshBodyEmpty(ch.id);
    })
  );
}

async function selectChapter(id, opts = {}) {
  error.value = "";
  try {
    clearGhost();
    const prevId = appState.chapterId;
    if (prevId && prevId !== id) {
      saveReadingProgress(prevId);
      chapterBodyCache[prevId] = snapshotBlocks(appState.chapterBlocks);
    }

    const scroller = getEditorScroller();
    let anchorDelta = null;
    if (scroller && opts.preserveViewport) {
      const sec = findChapterSection(scroller, id);
      if (sec) {
        const sr = scroller.getBoundingClientRect();
        anchorDelta = sec.getBoundingClientRect().top - sr.top;
      }
    }

    if (appState.dirty) await project.saveChapter();
    await project.loadChapter(id);
    tocFocusChapterId.value = id;
    expanded[id] = true;
    chapterBodyCache[id] = snapshotBlocks(appState.chapterBlocks);
    syncCurrentToc();

    await nextTick();
    await nextTick();

    if (opts.preserveViewport && scroller && anchorDelta != null) {
      const sec = findChapterSection(scroller, id);
      if (sec) {
        const sr = scroller.getBoundingClientRect();
        const now = sec.getBoundingClientRect().top - sr.top;
        scroller.scrollTop += now - anchorDelta;
      }
    }

    if (opts.skipRestore) {
      activeBlockKey.value = "";
    } else if (opts.jumpTop) {
      scrollChapterSectionToTop(id);
      activeBlockKey.value = "";
    } else {
      await restoreReadingProgress(id);
    }
    if (mobileUx.value) tocDrawerOpen.value = false;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

/** 点击邻章只读区：切入编辑且尽量不跳滚动 */
async function onActivateNeighborChapter(id) {
  if (!id || id === appState.chapterId) return;
  await selectChapter(id, { skipRestore: true, preserveViewport: true });
}

async function selectBlock(chapterId, blockKey) {
  error.value = "";
  suppressTocSpyUntil = Date.now() + 450;
  tocFocusChapterId.value = chapterId;
  setActiveBlockKey(blockKey || "", { scrollToc: false, chapterId });
  try {
    if (chapterId !== appState.chapterId) {
      await selectChapter(chapterId, { skipRestore: true, preserveViewport: true });
      await nextTick();
      await nextTick();
    }
    const ed = blockEditor.value;
    if (ed && typeof ed.scrollBlockIntoView === "function") {
      ed.scrollBlockIntoView(blockKey, { force: true });
    } else {
      const scroller = getEditorScroller();
      const el = scroller?.querySelector(
        `.chapter-block[data-block-key="${escapeAttrSelector(blockKey)}"]`
      );
      if (el && scroller) {
        const rootRect = scroller.getBoundingClientRect();
        const r = el.getBoundingClientRect();
        scroller.scrollTop += r.top - rootRect.top - 8;
      } else {
        appState.pendingScrollBlockKey = blockKey;
      }
    }
    window.setTimeout(() => saveReadingProgress(chapterId), 120);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function deleteTocBlock(chapterId, blockKey, ev) {
  if (ev) {
    ev.preventDefault();
    ev.stopPropagation();
  }
  error.value = "";
  if (!blockKey || anchoredJobsFor(blockKey).length) {
    if (anchoredJobsFor(blockKey).length || appState.generating) {
      error.value = "正在生成中，请稍候再删";
    }
    return;
  }
  if (
    !(await appConfirmDelete("删除这一段生成内容？", {
      title: "删除生成块",
    }))
  ) {
    return;
  }
  try {
    if (chapterId !== appState.chapterId) {
      await selectChapter(chapterId);
    }
    await deleteGenBlock(blockKey);
    if (activeBlockKey.value === blockKey) activeBlockKey.value = "";
    syncCurrentToc();
    appState.statusMessage = "已从目录删除生成块（未保存）";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

/**
 * 从目录删除整章（含正文）
 * @param {{ id: string, title?: string }} ch
 * @param {Event} [ev]
 */
async function deleteTocChapter(ch, ev) {
  if (ev) {
    ev.preventDefault();
    ev.stopPropagation();
  }
  error.value = "";
  if (!ch?.id) return;
  if (appState.generating || outlineQueueState.running || tocQueueBusy.value) {
    error.value = "正在生成中，请稍候再删";
    return;
  }
  const title = String(ch.title || "本章").trim() || "本章";
  if (
    !(await appConfirmDelete(`删除章节「${title}」？正文将一并删除，不可恢复。`, {
      title: "删除章节",
    }))
  ) {
    return;
  }
  try {
    const deletedId = ch.id;
    const wasCurrent = deletedId === appState.chapterId;
    if (wasCurrent) {
      clearGhost();
      appState.dirty = false;
    }
    await project.deleteChapter(deletedId);

    delete chapterBodyCache[deletedId];
    delete tocByChapter[deletedId];
    delete branchTocByChapter[deletedId];
    delete bodyEmptyByChapter[deletedId];
    delete expanded[deletedId];
    delete tocLoading[deletedId];

    const nextId = appState.chapterId;
    const stillThere = chapters.value.some((c) => c.id === nextId);
    if (nextId && stillThere) {
      if (wasCurrent) {
        await project.loadChapter(nextId);
        cacheCurrentChapterBody();
        syncCurrentToc();
        tocFocusChapterId.value = nextId;
        expanded[nextId] = true;
        activeBlockKey.value = "";
        await nextTick();
        scrollChapterSectionToTop(nextId);
      } else if (tocFocusChapterId.value === deletedId) {
        tocFocusChapterId.value = nextId;
        expanded[nextId] = true;
        activeBlockKey.value = "";
      }
    } else {
      tocFocusChapterId.value = "";
      activeBlockKey.value = "";
      appState.chapterId = "";
      appState.chapterContent = "";
      appState.chapterBlocks = [];
      appState.chapterBranchDoc = null;
    }
    if (editingSummaryId.value === deletedId) {
      editingSummaryId.value = "";
      editingSummaryDraft.value = "";
    }
    appState.statusMessage = `已删除章节「${title}」`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onSave() {
  try {
    await project.saveChapter();
    syncCurrentToc();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onAdd() {
  const t = newTitle.value.trim() || `第${chapters.value.length + 1}章`;
  try {
    await project.createChapter(t);
    newTitle.value = "";
    await refreshAllTocs();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function clearGhost() {
  ghostText.value = "";
  ghostActive.value = false;
  inlineBusy.value = false;
  showInline.value = false;
  inlinePrompt.value = "";
}

function openInline() {
  ensureBlocks();
  ghostBlockIndex.value = caret.value.index || 0;
  ghostOffset.value = caret.value.end ?? caret.value.start ?? 0;
  showInline.value = true;
  inlinePrompt.value = "";
  nextTick(() => {
    const el = document.getElementById("inline-cmd");
    if (el) el.focus();
  });
}

async function runInline() {
  if (!appState.projectRoot || !appState.chapterId) {
    error.value = "请先打开作品并选择章节";
    return;
  }
  ensureBlocks();
  const bi = ghostBlockIndex.value;
  const blocks = appState.chapterBlocks;
  const block = blocks[bi] || blocks[0];
  const text = block?.text || "";
  const selStart = caret.value.index === bi ? caret.value.start : ghostOffset.value;
  const selEnd = caret.value.index === bi ? caret.value.end : ghostOffset.value;
  const selected = selStart !== selEnd ? text.slice(selStart, selEnd) : "";
  ghostOffset.value = selected ? selEnd : selStart;
  ghostBlockIndex.value = bi;
  const task = selected ? "polish" : "continue";
  showInline.value = false;
  ghostActive.value = true;
  ghostText.value = "";
  inlineBusy.value = true;
  error.value = "";
  appState.draftPlacement = "";
  appState.draftTask = "";
  appState.draftSelection = "";
  try {
    if (appState.dirty) await project.saveChapter();
    const stop = watch(
      () => appState.previewText,
      (v) => {
        if (ghostActive.value) ghostText.value = v || "";
      },
    );
    await runWriting(
      task === "continue"
        ? withBranchContext(
            {
              project_root: appState.projectRoot,
              chapter_id: appState.chapterId,
              task,
              instruction: inlinePrompt.value || "在光标处续写一小段",
              selection: selected,
            },
            "continue",
            ""
          )
        : {
            project_root: appState.projectRoot,
            chapter_id: appState.chapterId,
            task,
            instruction: inlinePrompt.value || "润色选区",
            selection: selected,
          }
    );
    stop();
    ghostText.value = appState.previewText || ghostText.value;
  } catch (e) {
    error.value = String(e.message || e);
    clearGhost();
  } finally {
    inlineBusy.value = false;
  }
}

async function acceptGhost() {
  if (!ghostActive.value || !ghostText.value) return;
  await pushAiUndo("行内生成");
  ensureBlocks();
  const bi = ghostBlockIndex.value;
  const list = appState.chapterBlocks.map((b) => ({ ...b }));
  const block = list[bi] || list[0];
  if (!block) return;
  const i = ghostOffset.value;
  const t = block.text || "";
  block.text = t.slice(0, i) + ghostText.value + t.slice(i);
  if (block.type === "gen") block.chars = [...block.text].length;
  list[bi] = block;
  appState.chapterBlocks = list;
  appState.chapterContent = contentFromBlocks(list);
  syncBranchDocFromEditor();
  appState.dirty = true;
  clearGhost();
}

function rejectGhost() {
  if (inlineBusy.value) cancelGeneration();
  clearGhost();
}

function onKeydown(e) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    void onSave();
    return;
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
    e.preventDefault();
    openInline();
    return;
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z" && !e.shiftKey) {
    if (appState.aiUndoStack && appState.aiUndoStack.length) {
      e.preventDefault();
      undoLastAi();
    }
    return;
  }
  if (ghostActive.value && e.key === "Tab") {
    e.preventDefault();
    acceptGhost();
    return;
  }
  if (ghostActive.value && e.key === "Escape") {
    e.preventDefault();
    rejectGhost();
    return;
  }
  if (anyEditorDraft.value && !ghostActive.value && e.key === "Escape") {
    e.preventDefault();
    void rejectDraft();
  }
}

function tocItems(chapterId) {
  return tocByChapter[chapterId] || [];
}

function onEditorScroll() {
  noteEditorUserScroll();
  scheduleSaveReadingProgress();
  if (tocSpyRaf) return;
  tocSpyRaf = requestAnimationFrame(() => {
    tocSpyRaf = 0;
    syncActiveBlockFromScroll();
  });
}

function onEditorScrollIntent() {
  noteEditorScrollIntent();
}

/** 浮动按钮：跳到当前阅读块顶部 */
function jumpToCurrentBlockTop() {
  noteEditorScrollIntent();
  const key = activeBlockKey.value;
  const scroller = getEditorScroller();
  if (key && scroller) {
    const focusId = tocFocusChapterId.value || appState.chapterId;
    const scoped = focusId
      ? scroller.querySelector(
          `.continuous-chapter[data-chapter-id="${escapeAttrSelector(focusId)}"] .chapter-block[data-block-key="${escapeAttrSelector(key)}"]`
        )
      : null;
    const el =
      scoped ||
      scroller.querySelector(`.chapter-block[data-block-key="${escapeAttrSelector(key)}"]`);
    if (el) {
      const rootRect = scroller.getBoundingClientRect();
      const r = el.getBoundingClientRect();
      scroller.scrollTop += r.top - rootRect.top - 8;
      return;
    }
  }
  const ed = blockEditor.value;
  if (ed && typeof ed.scrollBlockIntoView === "function" && key) {
    ed.scrollBlockIntoView(key, { force: true });
    return;
  }
  jumpToChapterTop();
}

/** 浮动按钮：跳到当前阅读章最顶 */
function jumpToChapterTop() {
  noteEditorScrollIntent();
  const id = tocFocusChapterId.value || appState.chapterId;
  if (id) scrollChapterSectionToTop(id);
  else {
    const scroller = getEditorScroller();
    if (scroller) scroller.scrollTop = 0;
  }
}

let unwatchMobile = () => {};

onMounted(() => {
  mobileUx.value = isMobileUx();
  if (mobileUx.value) aiPanelLayout.value = "float";
  unwatchMobile = watchMobileViewport((m) => {
    mobileUx.value = m;
    if (m) {
      aiPanelLayout.value = "float";
      tocDrawerOpen.value = false;
    }
  });
  window.addEventListener("keydown", onKeydown);
  ensureBlocks();
  syncCurrentToc();
  cacheCurrentChapterBody();
  void preloadChapterBodies();
  void refreshCharacterNameIndex().catch(() => {});
  nextTick(() => {
    const el = getEditorScroller();
    if (el) {
      el.addEventListener("scroll", onEditorScroll, { passive: true });
      el.addEventListener("wheel", onEditorScrollIntent, { passive: true });
      el.addEventListener("touchmove", onEditorScrollIntent, { passive: true });
    }
    if (appState.chapterId) {
      tocFocusChapterId.value = appState.chapterId;
      void restoreReadingProgress(appState.chapterId);
    } else {
      syncActiveBlockFromScroll();
    }
  });
});
onUnmounted(() => {
  unwatchMobile();
  saveReadingProgress();
  window.removeEventListener("keydown", onKeydown);
  const el = getEditorScroller();
  if (el) {
    el.removeEventListener("scroll", onEditorScroll);
    el.removeEventListener("wheel", onEditorScrollIntent);
    el.removeEventListener("touchmove", onEditorScrollIntent);
  }
  if (tocSpyRaf) {
    cancelAnimationFrame(tocSpyRaf);
    tocSpyRaf = 0;
  }
  if (progressSaveTimer) {
    clearTimeout(progressSaveTimer);
    progressSaveTimer = 0;
  }
});

watch(
  () => appState.projectRoot,
  () => {
    void refreshCharacterNameIndex().catch(() => {});
  }
);

watch(
  () => appState.activeNav,
  (v) => {
    if (v === "editor") {
      void refreshCharacterNameIndex().catch(() => {});
      syncCurrentToc();
      nextTick(() => syncActiveBlockFromScroll());
    }
  }
);

watch(
  () => appState.chapterId,
  (id) => {
    if (!id) return;
    tocFocusChapterId.value = id;
    expanded[id] = true;
    cacheCurrentChapterBody();
    syncCurrentToc();
    nextTick(() => syncActiveBlockFromScroll());
  }
);

watch(
  () => appState.chapterBlocks,
  () => {
    cacheCurrentChapterBody();
    if (appState.editorScrollFreezeTop != null) return;
    if (restoringProgress) return;
    syncCurrentToc();
    nextTick(() => syncActiveBlockFromScroll());
  },
  { deep: true }
);

watch(
  () => chapters.value.map((c) => c.id).join("|"),
  () => {
    const id = appState.chapterId;
    if (id) {
      expanded[id] = true;
      tocFocusChapterId.value = id;
      syncCurrentToc();
    }
    void refreshAllTocs();
    void preloadChapterBodies();
  }
);

watch(
  () => appState.chapterContent,
  () => {
    syncCurrentBodyEmpty();
  }
);

watch(
  () => appState.editorScrollFreezeTop,
  (top) => {
    if (top == null) {
      // 冻结结束后补一次 TOC，写入过程中故意跳过以免侧栏重绘抢布局
      syncCurrentToc();
      nextTick(() => syncActiveBlockFromScroll());
    }
  }
);

watch(
  () => appState.draftPlacement,
  (v) => {
    if (v === "editor") clearGhost();
  }
);

watch(
  () => [
    outlineQueueState.running,
    outlineQueueState.chapterId,
    outlineQueueState.beatIndex,
    outlineQueueState.phase,
    visibleGenJobs.value.map((j) => `${j.id}:${j.status}:${j.label}`).join("|"),
  ],
  () => {
    const cid = outlineQueueState.chapterId || appState.chapterId;
    if (
      cid &&
      (outlineQueueState.running ||
        visibleGenJobs.value.some(
          (j) => j.status === "pending" || j.status === "streaming"
        ))
    ) {
      expanded[cid] = true;
      if (outlineQueueState.running && outlineQueueState.chapterId) {
        tocFocusChapterId.value = outlineQueueState.chapterId;
      }
      scrollTocGeneratingIntoView();
    }
  }
);
</script>

<template>
  <div v-if="!appState.projectRoot" class="panel">
    <h1 class="panel-heading">写作</h1>
    <p class="muted">请先在「作品」页打开或新建作品。</p>
  </div>
  <div
    v-else
    class="editor-layout"
    :class="{
      'is-float-ai': isAiFloat,
      'is-mobile': mobileUx,
      'toc-hidden': !mobileUx && !tocVisible,
    }"
  >
    <div
      v-if="mobileUx && tocDrawerOpen"
      class="toc-backdrop"
      @click="tocDrawerOpen = false"
    />
    <aside
      v-show="mobileUx || tocVisible"
      class="chapter-tree"
      :class="{ 'toc-drawer-open': !mobileUx || tocDrawerOpen }"
    >
      <div class="tree-head-row">
        <span class="tree-head">目录</span>
        <div class="tree-head-actions">
          <button
            type="button"
            class="tree-head-toggle"
            title="根据已有章节续拆后续待写章"
            :disabled="tocQueueBusy || outlineQueueState.running || !appState.chapterId"
            @click="onTocContinueSplit"
          >
            续拆后续
          </button>
          <button
            type="button"
            class="tree-head-toggle"
            title="按目录待写章纲整队生成"
            :disabled="tocQueueBusy || outlineQueueState.running || !appState.chapterId"
            @click="onTocWriteAll"
          >
            全部按纲写
          </button>
          <button
            v-if="!mobileUx"
            type="button"
            class="tree-head-toggle"
            title="隐藏目录栏"
            @click="toggleTocVisible"
          >
            隐藏
          </button>
        </div>
      </div>
      <div class="toc-list">
        <div
          v-for="ch in chapters"
          :key="ch.id"
          class="toc-node"
          :class="tocStatusClass(ch)"
        >
          <div
            class="toc-chapter-row"
            :class="{
              active: ch.id === tocFocusChapterId,
              editing: ch.id === appState.chapterId && ch.id !== tocFocusChapterId,
              [tocStatusClass(ch)]: true,
            }"
          >
            <button
              type="button"
              class="toc-caret"
              :class="{ open: isExpanded(ch.id) }"
              :title="isExpanded(ch.id) ? '收起章纲' : '展开章纲'"
              @click="toggleExpand(ch.id, $event)"
            >
              ›
            </button>
            <button
              type="button"
              class="toc-chapter"
              :class="{
                active: ch.id === tocFocusChapterId,
                editing: ch.id === appState.chapterId,
              }"
              @click="selectChapter(ch.id)"
            >
              <span class="toc-chapter-title">{{ ch.title }}</span>
              <span class="toc-status-badge" :class="tocStatusClass(ch)">{{
                tocStatusLabel(ch)
              }}</span>
            </button>
            <div class="toc-chapter-ops">
              <button
                v-if="tocStatusOf(ch) !== 'done'"
                type="button"
                class="toc-op-btn"
                title="按本章纲生成正文"
                :disabled="tocQueueBusy || outlineQueueState.running"
                @click="onTocWriteChapter(ch, $event)"
              >
                写
              </button>
              <button
                type="button"
                class="toc-op-btn"
                title="编辑本章纲"
                @click="startEditSummary(ch, $event)"
              >
                纲
              </button>
              <button
                type="button"
                class="toc-op-btn toc-op-del"
                title="删除本章"
                :disabled="tocQueueBusy || outlineQueueState.running || appState.generating"
                @click="deleteTocChapter(ch, $event)"
              >
                删
              </button>
            </div>
          </div>
          <div
            v-if="editingSummaryId === ch.id"
            class="toc-summary-edit"
            @click.stop
          >
            <textarea
              v-model="editingSummaryDraft"
              rows="3"
              placeholder="本章冲突 / 推进 / 钩子"
            />
            <div class="toc-summary-actions">
              <button type="button" class="app-btn" @click="saveEditSummary(ch)">
                保存
              </button>
              <button type="button" class="app-btn" @click="cancelEditSummary">
                取消
              </button>
            </div>
          </div>
          <div v-if="isExpanded(ch.id)" class="toc-children">
            <p v-if="ch.summary && editingSummaryId !== ch.id" class="toc-summary-preview muted">
              {{ ch.summary }}
            </p>
            <p
              v-else-if="!ch.summary && editingSummaryId !== ch.id"
              class="toc-empty muted"
            >
              暂无章纲
            </p>
          </div>
        </div>
      </div>
      <div class="field toc-add">
        <input v-model="newTitle" type="text" placeholder="新章节标题" />
        <button type="button" class="app-btn" style="margin-top: 6px; width: 100%" @click="onAdd">添加章节</button>
      </div>
      <div class="branch-graph-slot">
        <button
          type="button"
          class="branch-graph-toggle"
          @click="showBranchGraph = !showBranchGraph"
        >
          {{ showBranchGraph ? "收起分支图" : "展开分支图" }}
        </button>
        <BranchTreePanel
          v-if="showBranchGraph && appState.chapterId"
          :height="220"
          @select-block="onBranchGraphSelect"
        />
      </div>
    </aside>

    <section class="editor-main">
      <div class="editor-toolbar">
        <button
          v-if="mobileUx"
          type="button"
          class="app-btn"
          @click="tocDrawerOpen = !tocDrawerOpen"
        >
          目录
        </button>
        <button
          v-else-if="!tocVisible"
          type="button"
          class="app-btn"
          title="显示章节目录与分支图"
          @click="toggleTocVisible"
        >
          显示目录
        </button>
        <strong>{{
          chapters.find((c) => c.id === tocFocusChapterId)?.title ||
          chapters.find((c) => c.id === appState.chapterId)?.title ||
          "未选章节"
        }}</strong>
        <span
          v-if="
            tocFocusChapterId &&
            appState.chapterId &&
            tocFocusChapterId !== appState.chapterId
          "
          class="muted tip"
        >阅读中 · 编辑章另见目录</span>
        <span class="muted">{{ wordCount }} 字{{ appState.dirty ? " · 未保存" : "" }}</span>
        <label class="typo-ctrl muted">
          字体
          <select v-model="fontFamily" class="typo-select">
            <option v-for="p in fontPresets" :key="p.id" :value="p.id">{{ p.label }}</option>
          </select>
        </label>
        <label class="typo-ctrl muted">
          字号
          <select v-model="fontSize" class="typo-select">
            <option v-for="n in fontSizes" :key="n" :value="n">{{ n }}</option>
          </select>
        </label>
        <span v-if="!mobileUx" class="muted tip">Ctrl+S 保存 · Ctrl+K 行内 · Esc 取消生成</span>
        <button
          v-if="!mobileUx"
          type="button"
          class="app-btn"
          :title="
            aiPanelLayout === 'dock'
              ? '切到正文底部浮条'
              : aiPanelLayout === 'float'
                ? '完全隐藏 AI 面板'
                : '显示 AI 侧栏'
          "
          @click="toggleAiPanelLayout"
        >
          {{ aiLayoutButtonLabel }}
        </button>
        <button type="button" class="app-btn app-btn-primary" @click="onSave">保存</button>
      </div>
      <div class="editor-wrap">
        <div class="editor-scroll">
          <section
            v-for="ch in chapters"
            :key="ch.id"
            class="continuous-chapter"
            :class="{
              'is-editing': ch.id === appState.chapterId,
              'is-focus': ch.id === tocFocusChapterId,
            }"
            :data-chapter-id="ch.id"
          >
            <h2 class="continuous-chapter-title">{{ ch.title }}</h2>
            <ChapterBlockEditor
              v-if="ch.id === appState.chapterId"
              :ref="setBlockEditorRef"
              :readonly="ghostActive"
              :ghost-text="ghostActive ? ghostText : ''"
              :ghost-block-index="ghostBlockIndex"
              :ghost-offset="ghostOffset"
              @caret="onCaret"
            />
            <ContinuousChapterRead
              v-else
              :chapter-id="ch.id"
              :title="ch.title"
              :blocks="chapterBodyCache[ch.id] || []"
              @activate="onActivateNeighborChapter"
            />
            <template v-if="ch.id === appState.chapterId && trailingJobs.length">
              <EditorDraftPreview
                v-for="job in trailingJobs"
                :key="job.id"
                :job="job"
              />
            </template>
            <EditorDraftPreview
              v-else-if="ch.id === appState.chapterId && showEditorDraft"
            />
          </section>
          <div class="editor-jump-fabs" aria-label="跳转">
            <div class="editor-jump-inner">
              <button
                type="button"
                class="editor-jump-fab"
                title="跳到当前块顶部"
                aria-label="跳到当前块顶部"
                @click="jumpToCurrentBlockTop"
              >
                <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
                  <path
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M5 9h14M12 19V11M8 14l4-4 4 4"
                  />
                </svg>
              </button>
              <button
                type="button"
                class="editor-jump-fab"
                title="跳到本章顶部"
                aria-label="跳到本章顶部"
                @click="jumpToChapterTop"
              >
                <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
                  <path
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M5 5h14M12 20V9M8 13l4-4 4 4M8 17l4-4 4 4"
                  />
                </svg>
              </button>
            </div>
          </div>
        </div>
        <div v-if="showInline" class="inline-box">
          <input
            id="inline-cmd"
            v-model="inlinePrompt"
            type="text"
            placeholder="指令，回车生成"
            @keydown.enter.prevent="runInline"
            @keydown.escape.prevent="showInline = false"
          />
          <button type="button" class="app-btn app-btn-primary" @click="runInline">生成</button>
        </div>
        <p v-if="ghostActive" class="ghost-hint muted">
          {{ inlineBusy ? "生成中…" : "幽灵文本已就绪" }} — Tab 接受 / Esc 丢弃
        </p>
        <AiPanel
          v-if="isAiFloat"
          layout="float"
          @toggle-layout="onAiPanelToggleLayout"
        />
      </div>
      <pre v-if="error" class="out error">{{ error }}</pre>
    </section>

    <AiPanel
      v-if="!isAiFloat && !isAiHidden"
      class="ai-panel-slot"
      layout="dock"
      @toggle-layout="onAiPanelToggleLayout"
    />
  </div>
</template>

<style scoped>
.editor-layout {
  display: flex;
  height: 100%;
  min-height: 480px;
  gap: 10px;
  border: none;
  overflow: hidden;
  background: transparent;
  position: relative;
}
.editor-layout.is-mobile {
  min-height: 0;
}
.editor-layout.is-mobile .chapter-tree {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  z-index: 20;
  width: min(86vw, 300px);
  transform: translateX(-105%);
  transition: transform 0.22s ease;
  box-shadow: var(--shadow);
}
.editor-layout.is-mobile .chapter-tree.toc-drawer-open {
  transform: translateX(0);
}
.toc-backdrop {
  position: absolute;
  inset: 0;
  z-index: 18;
  background: rgba(20, 16, 24, 0.35);
}
@media (max-width: 720px) {
  .editor-jump-fabs {
    bottom: calc(88px + env(safe-area-inset-bottom, 0px));
  }
  .editor-toolbar {
    flex-wrap: wrap;
  }
}
.chapter-tree {
  width: 300px;
  flex-shrink: 0;
  padding: 14px 10px;
  overflow: hidden;
  background: var(--panel);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow);
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.tree-head-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 0 4px;
}
.tree-head {
  font-weight: 700;
  font-size: 13px;
  color: var(--text);
  flex-shrink: 0;
}
.tree-head-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 2px;
}
.tree-head-toggle {
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 4px;
}
.tree-head-toggle:hover:not(:disabled) {
  color: var(--text);
  background: var(--hover, rgba(0, 0, 0, 0.04));
}
.tree-head-toggle:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.toc-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.toc-node {
  margin-bottom: 4px;
  /* 章与章同级，禁止被上一章 toc-children 的左边线「看起来像嵌套」 */
  position: relative;
  z-index: 1;
  clear: both;
}
.toc-node.toc-status-pending {
  opacity: 0.92;
}
.toc-children {
  margin: 2px 0 8px 22px;
  padding-left: 8px;
  border-left: 1px solid color-mix(in srgb, var(--muted) 35%, transparent);
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.toc-chapter-row {
  display: flex;
  align-items: center;
  gap: 2px;
  border-radius: var(--radius-pill);
  border-left: 3px solid transparent;
}
.toc-chapter-row.toc-status-pending {
  border-left-color: var(--accent, #7c6cf0);
  opacity: 0.88;
}
.toc-chapter-row.toc-status-done {
  border-left-color: transparent;
}
.toc-chapter-row.toc-status-writing {
  border-left-color: var(--muted);
}
.toc-chapter-row.active {
  background: var(--accent-soft);
}
.toc-chapter-row.editing:not(.active) {
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 35%, transparent);
}
.toc-caret {
  width: 22px;
  height: 28px;
  flex-shrink: 0;
  border: none;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  font-size: 14px;
  line-height: 1;
  border-radius: 6px;
  transform: rotate(0deg);
  transition: transform 0.12s ease;
}
.toc-caret.open {
  transform: rotate(90deg);
  color: var(--text);
}
.toc-caret:hover {
  background: var(--accent-soft);
  color: var(--text);
}
.toc-chapter {
  flex: 1;
  min-width: 0;
  text-align: left;
  border: none;
  background: transparent;
  color: var(--muted);
  padding: 7px 6px 7px 4px;
  border-radius: var(--radius-pill);
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 6px;
}
.toc-chapter-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.toc-status-badge {
  flex-shrink: 0;
  font-size: 10px;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 4px;
  line-height: 1.4;
}
.toc-status-badge.toc-status-pending {
  color: var(--accent, #5b4fcf);
  background: var(--accent-soft, rgba(124, 108, 240, 0.15));
}
.toc-status-badge.toc-status-writing {
  color: var(--muted);
  background: rgba(0, 0, 0, 0.05);
}
.toc-status-badge.toc-status-done {
  color: #2f6b3a;
  background: rgba(47, 107, 58, 0.12);
}
.toc-chapter-ops {
  display: flex;
  flex-shrink: 0;
  gap: 2px;
  padding-right: 2px;
}
.toc-op-btn {
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 11px;
  font-weight: 700;
  cursor: pointer;
  padding: 4px 5px;
  border-radius: 4px;
}
.toc-op-btn:hover:not(:disabled) {
  color: var(--accent);
  background: var(--accent-soft);
}
.toc-op-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.toc-op-btn.toc-op-del:hover:not(:disabled) {
  color: #b33a3a;
  background: rgba(179, 58, 58, 0.12);
}
.toc-summary-edit {
  padding: 4px 6px 8px 26px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.toc-summary-edit textarea {
  width: 100%;
  font-size: 12px;
  resize: vertical;
  min-height: 54px;
}
.toc-summary-actions {
  display: flex;
  gap: 6px;
}
.toc-summary-preview {
  margin: 0 0 6px;
  padding: 0 4px;
  font-size: 11px;
  line-height: 1.45;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.toc-chapter:hover {
  color: var(--text);
}
.toc-chapter.active {
  color: var(--accent);
}
.toc-block-row {
  display: flex;
  align-items: stretch;
  gap: 2px;
  width: 100%;
  border-radius: 8px;
}
.toc-block-row:hover {
  background: var(--accent-soft);
}
.toc-block-row.active {
  background: var(--accent);
  box-shadow: var(--shadow-nav);
}
.toc-block-row.is-generating {
  background: color-mix(in srgb, var(--accent-soft, #fde8ee) 80%, transparent);
}
.toc-block-row.is-generating.active {
  background: color-mix(in srgb, var(--accent) 88%, #fff);
}
.toc-block {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  flex: 1 1 auto;
  min-width: 0;
  text-align: left;
  border: none;
  background: transparent;
  color: var(--muted);
  padding: 5px 4px 5px 8px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 11px;
  line-height: 1.35;
}
.toc-block-row.is-generating .toc-block {
  color: var(--accent-hover, var(--accent));
  font-weight: 650;
}
.toc-gen-spin {
  flex: 0 0 auto;
  width: 12px;
  height: 12px;
  margin-top: 2px;
  border: 2px solid color-mix(in srgb, var(--accent) 28%, transparent);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: toc-gen-spin 0.7s linear infinite;
}
.toc-gen-badge {
  flex: 0 0 auto;
  margin-left: auto;
  padding: 0 5px;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 700;
  line-height: 1.5;
  color: var(--accent-hover, var(--accent));
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  white-space: nowrap;
}
@keyframes toc-gen-spin {
  to {
    transform: rotate(360deg);
  }
}
.toc-block-row:hover .toc-block,
.toc-block:hover {
  color: var(--text);
}
.toc-block-row.active .toc-block {
  color: #fff;
}
.toc-block-del {
  flex: 0 0 auto;
  align-self: center;
  border: none;
  background: transparent;
  color: inherit;
  opacity: 0.45;
  cursor: pointer;
  font-size: 10px;
  line-height: 1;
  padding: 4px 6px;
  margin-right: 2px;
  border-radius: 6px;
}
.toc-block-row:hover .toc-block-del {
  opacity: 0.85;
}
.toc-block-row.active .toc-block-del {
  color: #fff;
  opacity: 0.9;
}
.toc-block-del:hover:not(:disabled) {
  opacity: 1;
  background: color-mix(in srgb, #c0392b 22%, transparent);
  color: #c0392b;
}
.toc-block-row.active .toc-block-del:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.22);
  color: #fff;
}
.toc-block-del:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}
.toc-block-idx {
  flex-shrink: 0;
  opacity: 0.7;
  font-variant-numeric: tabular-nums;
  min-width: 1.1em;
}
.toc-block-label {
  min-width: 0;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}
.toc-empty {
  margin: 4px 0 4px 8px;
  font-size: 11px;
}
.toc-add {
  margin-top: 0;
  padding-top: 6px;
  flex-shrink: 0;
}
.toc-block-row.toc-variant .toc-block-label {
  font-weight: 500;
  opacity: 0.92;
}
.toc-block-row.toc-hint .toc-block-label {
  font-style: italic;
  opacity: 0.75;
}
.toc-block-idx.toc-var {
  font-size: 10px;
}
.branch-graph-slot {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 48%;
  min-height: 0;
  overflow: hidden;
}
.branch-graph-toggle {
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  text-align: left;
  padding: 2px 4px;
}
.branch-graph-toggle:hover {
  color: var(--accent);
}
.editor-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: 12px 14px;
  background: var(--panel);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow);
}
.editor-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
  flex-wrap: wrap;
}
.tip {
  font-size: 11px;
}
.editor-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  position: relative;
}
.editor-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  /* 不用 flex：否则子项撑满视口时 sticky 总结条容易失效 */
  display: block;
  overflow-anchor: none;
}
.continuous-chapter {
  padding: 8px 4px 28px;
  border-bottom: 1px solid color-mix(in srgb, var(--muted) 22%, transparent);
  scroll-margin-top: 8px;
}
.continuous-chapter:last-of-type {
  border-bottom: none;
  padding-bottom: 48px;
}
.continuous-chapter.is-focus .continuous-chapter-title {
  color: var(--accent-hover, var(--accent));
}
.continuous-chapter-title {
  margin: 0 0 12px;
  padding: 4px 2px 8px;
  font-size: 1.15rem;
  font-weight: 700;
  line-height: 1.35;
  color: var(--text);
  border-bottom: 1px dashed color-mix(in srgb, var(--muted) 28%, transparent);
}
.editor-jump-fabs {
  position: sticky;
  bottom: 12px;
  height: 0;
  z-index: 9;
  pointer-events: none;
}
.editor-jump-inner {
  position: absolute;
  right: 14px;
  bottom: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.editor-jump-fab {
  pointer-events: auto;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 1px solid color-mix(in srgb, var(--muted) 28%, transparent);
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--panel, #fff) 92%, transparent);
  color: var(--muted);
  box-shadow: var(--shadow-sm, 0 2px 8px rgba(0, 0, 0, 0.08));
  cursor: pointer;
  backdrop-filter: blur(6px);
}
.editor-jump-fab:hover {
  color: var(--accent-hover, var(--accent));
  border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  background: var(--accent-soft, #fde8ee);
}
.editor-jump-fab:active {
  transform: translateY(1px);
}
.typo-ctrl {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  margin-left: 8px;
}
.typo-select {
  width: auto;
  min-width: 88px;
  padding: 4px 8px;
  font-size: 12px;
}
.inline-box {
  position: absolute;
  top: 12px;
  left: 14px;
  right: 14px;
  display: flex;
  gap: 8px;
  padding: 8px;
  background: var(--panel);
  border-radius: var(--radius-pill);
  box-shadow: var(--shadow);
  z-index: 2;
}
.inline-box input {
  flex: 1;
  border-radius: var(--radius-pill);
}
.ghost-hint {
  margin-top: 6px;
  font-size: 12px;
}
.error { color: var(--error); }
</style>
