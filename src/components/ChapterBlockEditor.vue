<!--
  章节分块编辑器：整块总结横条吸顶 + 生成块来源 + 角色名 hover
  代码路径: kk_novel_ai/src/components/ChapterBlockEditor.vue
-->
<script setup>
import { computed, nextTick, reactive, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import {
  contentFromBlocks,
  createPlainBlock,
  formatBlockMeta,
  formatBlockSources,
  paragraphSummaryLabel,
  paragraphSummaryTags,
} from "../utils/genBlock.js";
import { highlightNamesHtml } from "../utils/characterNameIndex.js";
import {
  deleteGenBlock,
  rewriteGenBlock,
  polishGenBlock,
  generateVariantBlock,
  forkFromBlock,
  switchBlockVariant,
  isDraftAnchoredTo,
  anchoredJobsFor,
} from "../services/draftAccept.js";
import { findNodeByBlockKey } from "../utils/branchModel.js";
import { syncBranchDocFromEditor } from "../services/projectClient.js";
import { runBlockDigest, saveBlockDigestManual } from "../services/blockDigest.js";
import { appConfirmDelete } from "../services/confirmDialog.js";
import { createBackdropDismiss } from "../utils/backdropDismiss.js";
import {
  activePathKey,
  applyScrollerProgress,
  captureScrollerProgress,
  getChapterProgress,
  saveChapterProgress,
} from "../services/editorReadingProgress.js";
import { canStartMoreJobs, MAX_PARALLEL_GEN } from "../stores/genJobs.js";
import CharacterHoverCard from "./CharacterHoverCard.vue";
import EditorDraftPreview from "./EditorDraftPreview.vue";

const props = defineProps({
  readonly: { type: Boolean, default: false },
  ghostText: { type: String, default: "" },
  ghostBlockIndex: { type: Number, default: -1 },
  ghostOffset: { type: Number, default: 0 },
});

const emit = defineEmits(["caret"]);

const rootEl = ref(null);
const areaRefs = ref([]);
const digestRefs = ref([]);
const blockEls = ref([]);
const hideTimer = ref(null);
const blockError = ref("");
const rewritingKey = ref("");
const polishingKey = ref("");
const digestingKey = ref("");
const copiedInstrKey = ref("");
let copiedInstrTimer = null;

/** 标题条动作浮动指令框 */
const actionPopup = reactive({
  open: false,
  mode: "", // polish | rewrite
  blockKey: "",
  instruction: "",
  x: 0,
  y: 0,
});

const hover = reactive({
  visible: false,
  entry: null,
  term: "",
  x: 0,
  y: 0,
  pinned: false,
});

function setAreaRef(el, i) {
  if (el) areaRefs.value[i] = el;
  else areaRefs.value[i] = null;
}

function setDigestRef(el, i) {
  if (el) digestRefs.value[i] = el;
  else digestRefs.value[i] = null;
}

function setBlockRef(el, i) {
  if (el) blockEls.value[i] = el;
  else blockEls.value[i] = null;
}

const blocks = computed(() => {
  if (Array.isArray(appState.chapterBlocks) && appState.chapterBlocks.length) {
    return appState.chapterBlocks;
  }
  return [createPlainBlock(appState.chapterContent || "")];
});

const nameTerms = computed(() => appState.characterNameTerms || []);

function displayText(block, index) {
  const base = block.text || "";
  if (
    props.ghostText &&
    props.ghostBlockIndex === index &&
    props.ghostOffset >= 0
  ) {
    const i = Math.min(props.ghostOffset, base.length);
    return base.slice(0, i) + props.ghostText + base.slice(i);
  }
  return base;
}

function mirrorHtml(block, index) {
  return highlightNamesHtml(displayText(block, index), nameTerms.value);
}

/** 整块总结标题：生成块优先指令，否则取正文开头 */
function blockSummaryLabel(block, index) {
  if (block.type === "gen") {
    const instr = String(block.instruction || "").trim();
    if (instr) return paragraphSummaryLabel(instr, 40);
    const sourcesLine = formatBlockSources(block);
    if (sourcesLine) return paragraphSummaryLabel(sourcesLine, 40);
  }
  return paragraphSummaryLabel(displayText(block, index), 40);
}

function blockSummaryTags(block, index) {
  if (block.type !== "gen") return [];
  return paragraphSummaryTags(displayText(block, index), block);
}

function blockSourceLine(block) {
  if (block.type !== "gen") return "";
  return formatBlockSources(block);
}

function syncFromBlocks(list) {
  appState.chapterBlocks = list;
  appState.chapterContent = contentFromBlocks(list);
  syncBranchDocFromEditor();
  appState.dirty = true;
}

function onInput(index, e) {
  if (props.readonly) return;
  const list = blocks.value.map((b, i) =>
    i === index ? { ...b, text: e.target.value } : b
  );
  if (list[index]?.type === "gen") {
    const text = list[index].text || "";
    list[index] = {
      ...list[index],
      chars: [...text].length,
      // 正文被清空时一并丢掉本段记忆，保存时会同步 memory.json
      digest: text.trim() ? list[index].digest || "" : "",
    };
    if (!text.trim() && list[index].key) {
      void import("../services/blockDigest.js").then((m) =>
        m.removeBlockNote(list[index].key)
      );
    }
  }
  syncFromBlocks(list);
  nextTick(() => autoSize(areaRefs.value[index]));
}

function onCaret(index, e) {
  const el = e.target;
  emit("caret", {
    index,
    start: el.selectionStart ?? 0,
    end: el.selectionEnd ?? 0,
  });
}

function autoSize(el) {
  if (!el) return;
  el.style.height = "";
  el.style.minHeight = "";
}

/** 本段记忆：按内容撑开，不出现内部滚动条 */
function autoSizeDigest(el) {
  if (!el) return;
  el.style.height = "0px";
  el.style.height = `${Math.max(el.scrollHeight, 40)}px`;
}

function autoSizeAll() {
  areaRefs.value.forEach((el) => autoSize(el));
  digestRefs.value.forEach((el) => autoSizeDigest(el));
}

function scrollBlockIntoView(keyOrIndex, opts = {}) {
  const force = !!opts.force;
  const list = blocks.value;
  let index = -1;
  if (typeof keyOrIndex === "number") {
    index = keyOrIndex;
  } else if (keyOrIndex) {
    index = list.findIndex((b) => b.key === keyOrIndex);
  }
  if (index < 0) index = list.length - 1;
  if (index < 0) return;

  const blockEl = blockEls.value[index];
  const area = areaRefs.value[index];
  const root = rootEl.value;
  if (!blockEl || !root) return;

  const scroller =
    (root && root.closest && root.closest(".editor-scroll")) || root;
  if (!scroller) return;
  const rootRect = scroller.getBoundingClientRect();
  const blockRect = blockEl.getBoundingClientRect();
  // 已在视口内则不滚，避免生成写入后硬对齐块顶造成「跳回顶部」
  // TOC 点击传 force：始终滚到该块顶部附近
  if (!force) {
    const fullyAbove = blockRect.bottom < rootRect.top + 8;
    const fullyBelow = blockRect.top > rootRect.bottom - 8;
    if (!fullyAbove && !fullyBelow) {
      if (area && !props.readonly) {
        area.focus({ preventScroll: true });
      }
      return;
    }
  }
  const nextTop = scroller.scrollTop + (blockRect.top - rootRect.top) - 8;
  scroller.scrollTop = Math.max(0, nextTop);

  if (area && !props.readonly) {
    area.focus({ preventScroll: true });
    try {
      area.setSelectionRange(0, 0);
    } catch {
      /* ignore */
    }
  }
}

function showHoverFromEl(el, clientX, clientY) {
  if (!el) return;
  const id = el.getAttribute("data-char-id") || "";
  const term = el.getAttribute("data-char-term") || "";
  const entry = appState.characterById && appState.characterById[id];
  if (!entry) return;
  if (hideTimer.value) {
    clearTimeout(hideTimer.value);
    hideTimer.value = null;
  }
  hover.entry = entry;
  hover.term = term;
  hover.x = clientX;
  hover.y = clientY;
  hover.visible = true;
}

function onHitOver(e) {
  const el = e.target && e.target.closest && e.target.closest(".char-hit");
  if (!el || !e.currentTarget.contains(el)) return;
  showHoverFromEl(el, e.clientX, e.clientY);
}

function onHitMove(e) {
  const el = e.target && e.target.closest && e.target.closest(".char-hit");
  if (!el || !hover.visible) return;
  hover.x = e.clientX;
  hover.y = e.clientY;
}

function scheduleHide() {
  if (hover.pinned) return;
  if (hideTimer.value) clearTimeout(hideTimer.value);
  hideTimer.value = setTimeout(() => {
    hover.visible = false;
    hover.entry = null;
    hideTimer.value = null;
  }, 180);
}

function onHitOut(e) {
  const el = e.target && e.target.closest && e.target.closest(".char-hit");
  if (!el) return;
  const to = e.relatedTarget;
  if (to && (to.closest?.(".char-hit") || to.closest?.(".char-hover-card"))) return;
  scheduleHide();
}

function onCardEnter() {
  hover.pinned = true;
  if (hideTimer.value) {
    clearTimeout(hideTimer.value);
    hideTimer.value = null;
  }
}

function onCardLeave() {
  hover.pinned = false;
  scheduleHide();
}

function blockBusy(block) {
  return anchoredJobsFor(block?.key).length > 0;
}

function genSlotsFree(n = 1) {
  return canStartMoreJobs(n);
}

async function onDeleteBlock(block) {
  blockError.value = "";
  if (!block?.key || blockBusy(block)) return;
  if (
    !(await appConfirmDelete("删除这一段生成内容？", {
      title: "删除生成块",
    }))
  ) {
    return;
  }
  try {
    await deleteGenBlock(block.key);
  } catch (e) {
    blockError.value = String(e.message || e);
  }
}

function closeActionPopup() {
  actionPopup.open = false;
  actionPopup.mode = "";
  actionPopup.blockKey = "";
  actionPopup.instruction = "";
}

function openActionPopup(mode, block, ev) {
  if (!block?.key || props.readonly || !genSlotsFree(1)) return;
  const btn = ev && ev.currentTarget;
  const rect = btn && btn.getBoundingClientRect ? btn.getBoundingClientRect() : null;
  const vw = window.innerWidth || 800;
  const vh = window.innerHeight || 600;
  let x = rect ? rect.left : 80;
  let y = rect ? rect.bottom + 6 : 120;
  const boxW = 320;
  const boxH = 168;
  if (x + boxW > vw - 12) x = Math.max(12, vw - boxW - 12);
  if (y + boxH > vh - 12) y = Math.max(12, (rect ? rect.top : 120) - boxH - 6);
  actionPopup.open = true;
  actionPopup.mode = mode;
  actionPopup.blockKey = block.key;
  actionPopup.instruction =
    mode === "polish"
      ? "删尽「不是…是…」否定对照，保持情节，提升画面感"
      : block.instruction || "保持人物与文风，推进同一情节，不要复述前文";
  actionPopup.x = x;
  actionPopup.y = y;
  nextTick(() => {
    const el = document.getElementById("block-action-instr");
    if (el) {
      el.focus();
      el.select();
    }
  });
}

function onRewriteClick(block, ev) {
  openActionPopup("rewrite", block, ev);
}

function onPolishClick(block, ev) {
  openActionPopup("polish", block, ev);
}

function nodeVariantsFor(block) {
  if (!block?.key || !appState.chapterBranchDoc) return [];
  const hit = findNodeByBlockKey(appState.chapterBranchDoc, block.key);
  return hit?.node?.variants || [];
}

/** 本块正在前台流式生成（变体/重写/润色/岔开） */
function isAnchoredDraft(block) {
  return !!(block?.key && isDraftAnchoredTo(block.key));
}

function draftJobsForBlock(block) {
  return anchoredJobsFor(block?.key);
}

/** 变体/重写/润色：隐藏原正文，只看新稿；岔开：保留原正文，草稿接在后面 */
function hideBodyForAnchoredDraft(block) {
  const jobs = draftJobsForBlock(block);
  if (!jobs.length) {
    if (!isAnchoredDraft(block)) return false;
    if (appState.draftBranchMode === "fork") return false;
    return true;
  }
  // 只要有一路非岔开草稿，就藏原正文，避免和多路预览抢视线
  return jobs.some((j) => j.draftBranchMode !== "fork");
}

function onSwitchVariant(block, variantId) {
  const hit = findNodeByBlockKey(appState.chapterBranchDoc, block?.key);
  if (!hit || !variantId) return;
  if (hit.variant.id === variantId) return;

  const root = appState.projectRoot;
  const chapterId = appState.chapterId;
  const scroller =
    (rootEl.value && rootEl.value.closest && rootEl.value.closest(".editor-scroll")) ||
    document.querySelector(".editor-scroll");
  const oldPath = activePathKey(appState.chapterBranchDoc);
  if (root && chapterId && scroller) {
    saveChapterProgress(
      root,
      chapterId,
      captureScrollerProgress(scroller, block?.key || ""),
      oldPath
    );
  }

  switchBlockVariant(hit.node.id, variantId);

  const newPath = activePathKey(appState.chapterBranchDoc);
  const prog =
    root && chapterId ? getChapterProgress(root, chapterId, newPath) : null;
  const sameNodeKey =
    (appState.chapterBlocks || []).find((b) => b._nodeId === hit.node.id)?.key ||
    "";
  nextTick(() => {
    requestAnimationFrame(() => {
      if (prog) {
        applyScrollerProgress(scroller, prog.scrollTop);
        if (prog.blockKey) {
          scrollBlockIntoView(prog.blockKey, { force: false });
          applyScrollerProgress(scroller, prog.scrollTop);
        }
      } else if (sameNodeKey) {
        // 该路径尚无记录：对齐到同节点新变体，避免跳回顶
        scrollBlockIntoView(sameNodeKey, { force: false });
      }
    });
  });
}

async function onGenerateVariant(block, count = 1) {
  blockError.value = "";
  if (!block?.key) return;
  try {
    await generateVariantBlock(block.key, { count });
  } catch (e) {
    blockError.value = String(e.message || e);
  }
}

async function onForkBranch(block) {
  blockError.value = "";
  if (!block?.key) return;
  try {
    await forkFromBlock(block.key);
  } catch (e) {
    blockError.value = String(e.message || e);
  }
}

async function confirmActionPopup() {
  const mode = actionPopup.mode;
  const key = actionPopup.blockKey;
  const instr = String(actionPopup.instruction || "").trim();
  if (!key || !mode) return;
  closeActionPopup();
  blockError.value = "";
  if (mode === "rewrite") {
    rewritingKey.value = key;
    try {
      await rewriteGenBlock(key, { instruction: instr });
    } catch (e) {
      blockError.value = String(e.message || e);
    } finally {
      rewritingKey.value = "";
    }
  } else if (mode === "polish") {
    polishingKey.value = key;
    try {
      await polishGenBlock(key, { instruction: instr });
    } catch (e) {
      blockError.value = String(e.message || e);
    } finally {
      polishingKey.value = "";
    }
  }
}

function cancelActionPopup() {
  closeActionPopup();
  appState.statusMessage = "已取消";
}

const actionPopupBackdrop = createBackdropDismiss(cancelActionPopup);

/** 块生成指令全文（含 sources 里的 instruction） */
function blockInstructionText(block) {
  if (!block || block.type !== "gen") return "";
  const direct = String(block.instruction || "").trim();
  if (direct) return direct;
  const items = Array.isArray(block.sources) ? block.sources : [];
  const hit = items.find((s) => s && s.kind === "instruction");
  if (!hit) return "";
  return String(hit.detail || hit.title || "").trim();
}

/**
 * 可复制内容：优先生成指令；无指令时回退标题条上的章纲/设定等来源摘要
 * （旧块常只落了 sources、没落 instruction，不能因此把按钮灰掉）
 */
function blockCopyableText(block) {
  const instr = blockInstructionText(block);
  if (instr) return { kind: "instruction", text: instr };
  const sources = formatBlockSources(block);
  if (sources) return { kind: "sources", text: sources };
  return { kind: "", text: "" };
}

async function copyTextToClipboard(text) {
  const value = String(text || "");
  if (!value) throw new Error("没有可复制的内容");
  if (navigator.clipboard && navigator.clipboard.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }
  const ta = document.createElement("textarea");
  ta.value = value;
  ta.setAttribute("readonly", "");
  ta.style.position = "fixed";
  ta.style.left = "-9999px";
  document.body.appendChild(ta);
  ta.select();
  const ok = document.execCommand("copy");
  document.body.removeChild(ta);
  if (!ok) throw new Error("复制失败");
}

async function onCopyInstruction(block) {
  blockError.value = "";
  if (!block?.key) return;
  try {
    const payload = blockCopyableText(block);
    await copyTextToClipboard(payload.text);
    copiedInstrKey.value = block.key;
    if (copiedInstrTimer) clearTimeout(copiedInstrTimer);
    copiedInstrTimer = setTimeout(() => {
      if (copiedInstrKey.value === block.key) copiedInstrKey.value = "";
    }, 1600);
    appState.statusMessage =
      payload.kind === "instruction" ? "生成指令已复制" : "块来源条目已复制";
  } catch (e) {
    blockError.value = String(e.message || e);
  }
}

async function onRedigestBlock(block) {
  blockError.value = "";
  if (!block?.key || !block.text || appState.generating) return;
  digestingKey.value = block.key;
  try {
    await runBlockDigest({
      blockKey: block.key,
      text: block.text,
      instruction: block.instruction || "",
    });
  } catch (e) {
    blockError.value = String(e.message || e);
  } finally {
    digestingKey.value = "";
  }
}

function onDigestInput(index, e) {
  if (props.readonly) return;
  const list = blocks.value.map((b, i) =>
    i === index ? { ...b, digest: e.target.value } : b
  );
  syncFromBlocks(list);
  nextTick(() => autoSizeDigest(e.target));
}

async function onDigestBlur(block) {
  if (props.readonly || !block?.key) return;
  if (digestingKey.value === block.key) return;
  try {
    await saveBlockDigestManual({
      blockKey: block.key,
      digest: block.digest || "",
    });
  } catch (e) {
    blockError.value = String(e.message || e);
  }
}

watch(
  () => blocks.value.map((b) => `${b.key}:${(b.text || "").length}`).join("|"),
  async () => {
    await nextTick();
    autoSizeAll();
    const freeze = appState.editorScrollFreezeTop;
    if (freeze != null) {
      const scroller =
        (rootEl.value && rootEl.value.closest && rootEl.value.closest(".editor-scroll")) ||
        document.querySelector(".editor-scroll");
      if (scroller) scroller.scrollTop = freeze;
    }
  },
  { immediate: true }
);

watch(
  () =>
    blocks.value
      .map((b) => `${b.key}:${String(b.digest || "").length}:${digestingKey.value === b.key ? 1 : 0}`)
      .join("|"),
  async () => {
    await nextTick();
    digestRefs.value.forEach((el) => autoSizeDigest(el));
  }
);

watch(
  () => nameTerms.value.length,
  async () => {
    await nextTick();
    autoSizeAll();
  }
);

watch(
  () => appState.pendingScrollBlockKey,
  async (key) => {
    if (!key) return;
    await nextTick();
    await nextTick();
    scrollBlockIntoView(key, { force: true });
    appState.pendingScrollBlockKey = "";
  }
);

defineExpose({
  focusBlock(index, offset) {
    nextTick(() => {
      const el = areaRefs.value[index];
      if (!el) return;
      el.focus({ preventScroll: true });
      const o = Math.min(Math.max(0, offset ?? el.value.length), el.value.length);
      el.setSelectionRange(o, o);
    });
  },
  scrollBlockIntoView,
  getAreas() {
    return areaRefs.value;
  },
});
</script>

<template>
  <div ref="rootEl" class="block-editor">
    <div
      v-for="(block, index) in blocks"
      :key="block.key || index"
      :ref="(el) => setBlockRef(el, index)"
      class="chapter-block"
      :class="[
        block.type === 'gen' ? 'is-gen' : 'is-plain',
        { 'is-drafting': isAnchoredDraft(block) },
      ]"
      :data-block-key="block.key || ''"
    >
      <!-- 整块一条总结横条：滚到本块范围内时吸顶；生成块带操作按钮 -->
      <div
        class="block-sticky-bar"
        :title="blockSourceLine(block) || blockSummaryLabel(block, index)"
      >
        <div class="block-sticky-main">
          <span class="block-sum-label">{{ blockSummaryLabel(block, index) }}</span>
          <span
            v-for="(tag, ti) in blockSummaryTags(block, index)"
            :key="`${tag.kind}-${ti}-${tag.label}`"
            class="block-sum-tag"
            :data-kind="tag.kind"
          >{{ tag.label }}</span>
        </div>
        <div v-if="block.type === 'gen'" class="block-sticky-actions">
          <div
            v-if="nodeVariantsFor(block).length"
            class="block-variants"
            title="切换本小节变体"
          >
            <button
              v-for="(v, vi) in nodeVariantsFor(block)"
              :key="v.id"
              type="button"
              class="block-act variant-chip"
              :class="{ active: v.id === (block._variantId || '') }"
              :disabled="readonly || blockBusy(block)"
              @click.stop="onSwitchVariant(block, v.id)"
            >
              {{ v.label || `变体${vi + 1}` }}
            </button>
          </div>
          <button
            type="button"
            class="block-act block-act-emphasis"
            :disabled="
              readonly ||
              !genSlotsFree(1) ||
              polishingKey === block.key ||
              rewritingKey === block.key
            "
            title="完全按本块创作指令从零重写一版，不覆盖当前变体；不参照旧正文"
            @click.stop="onGenerateVariant(block, 1)"
          >
            {{
              draftJobsForBlock(block).some((j) => j.draftBranchMode === "variant")
                ? `生成中(${draftJobsForBlock(block).length})`
                : "生成变体"
            }}
          </button>
          <button
            type="button"
            class="block-act"
            :disabled="
              readonly ||
              !genSlotsFree(2) ||
              polishingKey === block.key ||
              rewritingKey === block.key
            "
            :title="`同时按本块指令从零重写 2 版（上限 ${MAX_PARALLEL_GEN} 路）`"
            @click.stop="onGenerateVariant(block, 2)"
          >
            并发生成×2
          </button>
          <button
            type="button"
            class="block-act"
            :disabled="
              readonly ||
              !genSlotsFree(1) ||
              polishingKey === block.key ||
              rewritingKey === block.key
            "
            title="从当前变体岔开写下一节"
            @click.stop="onForkBranch(block)"
          >
            从此岔开
          </button>
          <button
            type="button"
            class="block-act"
            :disabled="!blockCopyableText(block).text"
            :title="
              blockCopyableText(block).text ||
              '无生成指令，也无章纲/设定等来源'
            "
            @click.stop="onCopyInstruction(block)"
          >
            {{
              copiedInstrKey === block.key
                ? "已复制"
                : blockInstructionText(block)
                  ? "复制指令"
                  : "复制条目"
            }}
          </button>
          <button
            type="button"
            class="block-act"
            :disabled="
              readonly ||
              !genSlotsFree(1) ||
              polishingKey === block.key ||
              rewritingKey === block.key
            "
            @click.stop="onPolishClick(block, $event)"
          >
            {{ polishingKey === block.key ? "润色中…" : "润色" }}
          </button>
          <button
            type="button"
            class="block-act"
            :disabled="
              readonly ||
              !genSlotsFree(1) ||
              rewritingKey === block.key ||
              polishingKey === block.key
            "
            @click.stop="onRewriteClick(block, $event)"
          >
            {{ rewritingKey === block.key ? "生成中…" : "重新生成" }}
          </button>
          <button
            type="button"
            class="block-act danger"
            :disabled="readonly || blockBusy(block)"
            @click.stop="onDeleteBlock(block)"
          >
            删除
          </button>
        </div>
      </div>

      <EditorDraftPreview
        v-for="job in hideBodyForAnchoredDraft(block) ? draftJobsForBlock(block) : []"
        :key="job.id"
        :job="job"
        embedded
      />

      <div
        v-show="!hideBodyForAnchoredDraft(block)"
        class="block-stack"
        @mouseover="onHitOver"
        @mousemove="onHitMove"
        @mouseout="onHitOut"
      >
        <div
          class="block-mirror"
          :class="{ ghosting: readonly && ghostBlockIndex === index && ghostText }"
          aria-hidden="true"
          v-html="mirrorHtml(block, index)"
        />
        <textarea
          :ref="(el) => setAreaRef(el, index)"
          class="block-area"
          :class="{ ghosting: readonly && ghostBlockIndex === index && ghostText }"
          :value="displayText(block, index)"
          :readonly="readonly"
          :placeholder="index === 0 ? '在此写作…（Ctrl+K 行内生成）' : ''"
          rows="2"
          spellcheck="false"
          @input="onInput(index, $event)"
          @click="onCaret(index, $event)"
          @keyup="onCaret(index, $event)"
          @select="onCaret(index, $event)"
        />
      </div>

      <EditorDraftPreview
        v-for="job in !hideBodyForAnchoredDraft(block) ? draftJobsForBlock(block) : []"
        :key="job.id"
        :job="job"
        embedded
      />

      <div
        v-if="block.type === 'gen' && blockSourceLine(block) && !hideBodyForAnchoredDraft(block)"
        class="block-sources"
        :title="blockSourceLine(block)"
      >
        来源：{{ blockSourceLine(block) }}
      </div>

      <div
        v-if="block.type === 'gen' && !hideBodyForAnchoredDraft(block)"
        class="block-digest"
      >
        <div class="block-digest-title">本段记忆</div>
        <textarea
          :ref="(el) => setDigestRef(el, index)"
          class="block-digest-body"
          rows="1"
          :value="
            digestingKey === block.key && !block.digest
              ? ''
              : block.digest || ''
          "
          :placeholder="
            digestingKey === block.key ? '提炼中…' : '本段记忆（可编辑，失焦保存）'
          "
          :readonly="
            readonly ||
            digestingKey === block.key ||
            blockBusy(block)
          "
          @input="onDigestInput(index, $event)"
          @blur="onDigestBlur(block)"
        />
      </div>

      <div v-if="block.type === 'gen' && !hideBodyForAnchoredDraft(block)" class="block-meta-row">
        <div class="block-meta">{{ formatBlockMeta(block) }}</div>
        <div class="block-actions">
          <button
            type="button"
            class="block-act"
            :disabled="blockBusy(block) || digestingKey === block.key"
            @click="onRedigestBlock(block)"
          >
            {{ digestingKey === block.key ? "提炼中…" : "重提炼" }}
          </button>
          <button
            type="button"
            class="block-act danger"
            :disabled="blockBusy(block)"
            @click="onDeleteBlock(block)"
          >
            删除
          </button>
        </div>
      </div>
    </div>

    <p v-if="blockError" class="block-error">{{ blockError }}</p>

    <Teleport to="body">
      <div
        v-if="actionPopup.open"
        class="block-action-mask"
        @mousedown="actionPopupBackdrop.onMouseDown"
        @click="actionPopupBackdrop.onClick"
        @keydown.escape.prevent="cancelActionPopup"
      >
        <div
          class="block-action-pop"
          :style="{ left: actionPopup.x + 'px', top: actionPopup.y + 'px' }"
          role="dialog"
          aria-modal="true"
        >
          <div class="block-action-pop-title">
            {{ actionPopup.mode === "polish" ? "润色指令" : "重新生成指令" }}
          </div>
          <textarea
            id="block-action-instr"
            v-model="actionPopup.instruction"
            class="block-action-pop-input"
            rows="4"
            placeholder="输入本轮要求…"
            @keydown.enter.ctrl.prevent="confirmActionPopup"
          />
          <div class="block-action-pop-actions">
            <button type="button" class="app-btn" @click="cancelActionPopup">
              取消
            </button>
            <button
              type="button"
              class="app-btn app-btn-primary"
              @click="confirmActionPopup"
            >
              {{ actionPopup.mode === "polish" ? "开始润色" : "开始重写" }}
            </button>
          </div>
        </div>
      </div>
    </Teleport>

    <CharacterHoverCard
      :visible="hover.visible"
      :entry="hover.entry"
      :term="hover.term"
      :x="hover.x"
      :y="hover.y"
      @enter="onCardEnter"
      @leave="onCardLeave"
    />
  </div>
</template>

<style scoped>
.block-editor {
  flex: none;
  min-height: 0;
  overflow: visible;
  display: block;
  padding: 4px 2px 12px;
}
.chapter-block {
  display: block;
  width: 100%;
  margin: 0 0 14px;
  flex: none;
  overflow: visible;
  position: relative;
  overflow-anchor: none;
}
.chapter-block.is-gen {
  padding: 10px 12px 12px;
  background: var(--surface-solid);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-sm);
  border-left: 3px solid var(--accent);
  box-sizing: border-box;
}
.chapter-block.is-plain {
  padding-bottom: 4px;
  box-sizing: border-box;
}
.chapter-block.is-drafting {
  outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
  outline-offset: 2px;
}
.block-sticky-bar {
  position: sticky;
  top: 0;
  z-index: 8;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 6px 10px;
  padding: 7px 10px;
  margin: 0 0 8px;
  background: var(--surface-solid, #fff);
  border: 1px solid color-mix(in srgb, var(--accent) 22%, transparent);
  border-radius: var(--radius-md, 8px);
  box-shadow: 0 1px 0 color-mix(in srgb, var(--accent) 12%, transparent),
    0 6px 16px rgba(0, 0, 0, 0.06);
  box-sizing: border-box;
  /* 避免内容增高时浏览器滚动锚定把视口拽回块顶 */
  overflow-anchor: none;
}
.block-sticky-main {
  display: flex;
  flex: 1 1 180px;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px 8px;
  min-width: 0;
}
.block-sticky-actions {
  display: flex;
  flex: 1 1 220px;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}
.block-act.block-act-emphasis {
  background: var(--accent-soft);
  color: var(--accent-hover, var(--accent));
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent);
}
.block-act.block-act-emphasis:hover:not(:disabled) {
  background: color-mix(in srgb, var(--accent) 22%, transparent);
}
.chapter-block.is-plain .block-sticky-bar {
  margin-left: 0;
  margin-right: 0;
}
.block-sum-label {
  flex: 1 1 140px;
  min-width: 0;
  font-size: 12px;
  font-weight: 650;
  line-height: 1.35;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.block-sum-tag {
  flex: 0 0 auto;
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 2px 8px;
  border-radius: var(--radius-pill, 999px);
  font-size: 11px;
  font-weight: 600;
  line-height: 1.4;
  background: var(--chip-bg, #f0f0f0);
  color: var(--muted);
}
.block-sum-tag[data-kind="instruction"] {
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  color: var(--accent-hover, var(--accent));
}
.block-sum-tag[data-kind="lore"],
.block-sum-tag[data-kind="pov"] {
  background: rgba(220, 90, 120, 0.14);
  color: var(--accent-hover, #b84a62);
}
.block-sum-tag[data-kind="arc"],
.block-sum-tag[data-kind="must_do"] {
  background: color-mix(in srgb, var(--accent) 10%, #f5f5f5);
  color: var(--muted);
}
.block-stack {
  position: relative;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  width: 100%;
  min-height: 3em;
}
.chapter-block.is-plain .block-stack {
  background: var(--surface-solid);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-sm);
}
.block-mirror {
  grid-area: 1 / 1;
  position: relative;
  z-index: 2;
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  overflow: hidden;
  pointer-events: none;
  font-family: var(--editor-font-family);
  font-size: var(--editor-font-size);
  line-height: 1.7;
  color: var(--text);
  box-sizing: border-box;
  height: auto;
  min-height: 3em;
}
.chapter-block.is-plain .block-mirror {
  padding: 12px 14px;
}
.chapter-block.is-gen .block-mirror {
  padding: 0;
}
.block-mirror.ghosting {
  color: var(--muted);
}
.block-mirror :deep(.char-hit) {
  pointer-events: auto;
  background: rgba(220, 90, 120, 0.2);
  border-radius: 3px;
  box-decoration-break: clone;
  -webkit-box-decoration-break: clone;
  cursor: help;
}
.block-mirror :deep(.char-hit:hover) {
  background: rgba(220, 90, 120, 0.35);
}
.block-area {
  grid-area: 1 / 1;
  position: relative;
  z-index: 1;
  display: block;
  width: 100%;
  height: 100%;
  min-height: 3em;
  resize: none;
  overflow: hidden;
  font-family: var(--editor-font-family);
  font-size: var(--editor-font-size);
  line-height: 1.7;
  color: transparent;
  caret-color: var(--text);
  background: transparent !important;
  box-shadow: none !important;
  border: none;
  outline: none;
  box-sizing: border-box;
  field-sizing: fixed;
}
.chapter-block.is-plain .block-area {
  padding: 12px 14px;
}
.chapter-block.is-gen .block-area {
  padding: 0;
}
.block-area.ghosting {
  caret-color: var(--muted);
}
.block-area::placeholder {
  color: var(--muted);
  opacity: 0.7;
  -webkit-text-fill-color: var(--muted);
}
.block-sources {
  margin-top: 6px;
  padding: 0 2px 2px;
  font-size: 11px;
  line-height: 1.45;
  color: var(--accent-hover, var(--accent));
  opacity: 0.92;
  user-select: none;
  word-break: break-word;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.block-digest {
  margin-top: 8px;
  padding: 6px 8px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--accent) 6%, transparent);
  font-size: 11px;
  color: var(--muted);
}
.block-digest-title {
  font-weight: 600;
  color: var(--text);
  user-select: none;
  margin-bottom: 4px;
}
.block-digest-body {
  display: block;
  width: 100%;
  margin: 0;
  padding: 4px 6px;
  box-sizing: border-box;
  border: 1px solid color-mix(in srgb, var(--border, #888) 50%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--bg, #fff) 80%, transparent);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
  color: var(--text);
  opacity: 0.95;
  font: inherit;
  font-size: 11px;
  resize: none;
  overflow: hidden;
  min-height: 2.4em;
  field-sizing: content;
}
.block-digest-body:focus {
  outline: 1px solid color-mix(in srgb, var(--accent) 55%, transparent);
  opacity: 1;
}
.block-digest-body[readonly] {
  opacity: 0.7;
}
.block-meta-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 6px;
}
.block-meta {
  font-size: 11px;
  line-height: 1.4;
  color: var(--muted);
  padding: 0 2px 2px;
  user-select: none;
  word-break: break-all;
  flex: 1 1 auto;
}
.block-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  flex-shrink: 0;
  align-items: center;
  justify-content: flex-end;
}
.block-variants {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-right: 2px;
}
.block-act.variant-chip.active {
  background: var(--accent-soft);
  color: var(--accent-hover);
  box-shadow: inset 0 0 0 1px var(--accent, #6b8cae);
}
.block-act {
  border: none;
  background: var(--chip-bg, #f0f0f0);
  color: var(--muted);
  border-radius: var(--radius-pill);
  padding: 4px 10px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}
.block-act:hover:not(:disabled) {
  background: var(--accent-soft);
  color: var(--accent-hover);
}
.block-act.danger:hover:not(:disabled) {
  background: rgba(220, 90, 120, 0.18);
  color: var(--error, #c45c6a);
}
.block-act:disabled {
  opacity: 0.45;
  cursor: default;
}
.block-error {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--error);
}
</style>

<style>
/* Teleport 到 body，不可用 scoped */
.block-action-mask {
  position: fixed;
  inset: 0;
  z-index: 2100;
  background: transparent;
}
.block-action-pop {
  position: fixed;
  width: min(320px, calc(100vw - 24px));
  padding: 12px 12px 10px;
  border-radius: var(--radius-md, 10px);
  background: var(--panel, #fff);
  box-shadow: 0 10px 36px rgba(0, 0, 0, 0.18);
  border: 1px solid color-mix(in srgb, var(--accent, #c45) 22%, transparent);
  box-sizing: border-box;
}
.block-action-pop-title {
  font-size: 12px;
  font-weight: 650;
  margin-bottom: 8px;
  color: var(--text, #222);
}
.block-action-pop-input {
  display: block;
  width: 100%;
  box-sizing: border-box;
  margin: 0 0 10px;
  padding: 8px;
  border-radius: 8px;
  border: 1px solid color-mix(in srgb, var(--border, #888) 55%, transparent);
  background: var(--bg, #fff);
  color: var(--text, #222);
  font: inherit;
  font-size: 12px;
  line-height: 1.45;
  resize: vertical;
  min-height: 4.5em;
}
.block-action-pop-input:focus {
  outline: 1px solid color-mix(in srgb, var(--accent, #c45) 55%, transparent);
}
.block-action-pop-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
