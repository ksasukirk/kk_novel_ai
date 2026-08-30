<!--
  正文区生成草稿：流式预览（完成后自动写入）
  可嵌在目标块内（生成变体/重写）或挂在章末（续写）；支持传入 job 多路并存
  代码路径: kk_novel_ai/src/components/EditorDraftPreview.vue
-->
<script setup>
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import {
  isEditorDraftVisible,
  recentlyEditorScrollIntent,
  rejectDraft,
  rejectJob,
} from "../services/draftAccept.js";
import { highlightNamesHtml } from "../utils/characterNameIndex.js";
import CharacterHoverCard from "./CharacterHoverCard.vue";

const props = defineProps({
  /** 嵌在章节块内前台显示 */
  embedded: { type: Boolean, default: false },
  /** 多路生成时绑定某一 job；不传则走全局预览（兼容） */
  job: { type: Object, default: null },
});

const rootEl = ref(null);
const hideTimer = ref(null);
/** 用户滚离流式末尾后，停止追尾，直到其回到末尾附近 */
const followDetached = ref(false);
let scrollerEl = null;

const hover = reactive({
  visible: false,
  entry: null,
  term: "",
  x: 0,
  y: 0,
  pinned: false,
});

const streaming = computed(() => {
  if (props.job) {
    return props.job.status === "pending" || props.job.status === "streaming";
  }
  return !!appState.generating;
});

const visible = computed(() => {
  if (props.job) {
    if (props.job.accepted || props.job.draftPlacement !== "editor") return false;
    if (props.job.status === "pending" || props.job.status === "streaming") return true;
    if (props.job.status === "done" || props.job.status === "error") {
      return !!(props.job.previewRawText || props.job.previewText);
    }
    return false;
  }
  return isEditorDraftVisible();
});

const text = computed(() => {
  if (props.job) return props.job.previewRawText || props.job.previewText || "";
  return appState.previewRawText || appState.previewText || "";
});

const bodyHtml = computed(() =>
  highlightNamesHtml(text.value || "…", appState.characterNameTerms || [])
);

const draftTask = computed(() => (props.job ? props.job.draftTask : appState.draftTask) || "");
const draftBranchMode = computed(
  () => (props.job ? props.job.draftBranchMode : appState.draftBranchMode) || ""
);
const draftRewriteKey = computed(
  () => (props.job ? props.job.draftRewriteBlockKey : appState.draftRewriteBlockKey) || ""
);
const modelUsed = computed(
  () => (props.job ? props.job.lastModelUsed : appState.lastModelUsed) || ""
);

const draftTitle = computed(() => {
  const done = !streaming.value && !!(text.value);
  if (done) {
    if (draftBranchMode.value === "variant" || draftTask.value === "same_slot_variant") {
      return props.job?.label ? `${props.job.label}草稿` : "变体草稿";
    }
    if (draftBranchMode.value === "fork") return "岔开草稿";
    if (draftRewriteKey.value && draftTask.value === "polish") return "润色草稿";
    if (draftRewriteKey.value) return "重写草稿";
    return "生成草稿";
  }
  if (draftBranchMode.value === "variant" || draftTask.value === "same_slot_variant") {
    return props.job?.label ? `正在生成${props.job.label}` : "正在生成变体";
  }
  if (draftBranchMode.value === "fork") return "正在岔开续写";
  if (draftRewriteKey.value && draftTask.value === "polish") return "正在润色";
  if (draftRewriteKey.value) return "正在重写本段";
  return "正在生成";
});

const meta = computed(() => {
  const waitingWrite =
    !streaming.value &&
    !!text.value &&
    String(appState.statusMessage || "").includes("停滚");
  const parts = [
    waitingWrite ? "已完成 · 停滚后写入" : streaming.value ? "生成中…" : "已完成",
  ];
  if (draftBranchMode.value === "variant" || draftTask.value === "same_slot_variant") {
    parts.push("同位置按指令重写");
  } else if (draftBranchMode.value === "fork") parts.push("岔开");
  else if (draftRewriteKey.value) parts.push(draftTask.value === "polish" ? "润色" : "重写");
  else if (draftTask.value) parts.push(draftTask.value);
  if (modelUsed.value) parts.push(modelUsed.value);
  const n = [...text.value].length;
  if (n) parts.push(`${n} 字`);
  if (props.job?.progressPct && streaming.value) {
    parts.push(`${props.job.progressPct}%`);
  }
  return parts.join(" · ");
});

async function onCancel() {
  if (props.job) await rejectJob(props.job);
  else await rejectDraft();
}

function resolveScroller() {
  const draft = rootEl.value;
  if (!draft) return document.querySelector(".editor-scroll");
  return (
    (draft.closest && draft.closest(".editor-scroll")) ||
    document.querySelector(".editor-scroll")
  );
}

function distancePastFold(scroller, draft, slack = 48) {
  const sRect = scroller.getBoundingClientRect();
  const dRect = draft.getBoundingClientRect();
  return dRect.bottom - (sRect.bottom - slack);
}

function markFollowDetached() {
  followDetached.value = true;
}

function clearFollowDetachIfNearTail() {
  const draft = rootEl.value;
  const scroller = resolveScroller();
  if (!draft || !scroller) return;
  const past = distancePastFold(scroller, draft, 48);
  // 回到末尾附近则恢复跟随
  if (past < 120) followDetached.value = false;
}

/**
 * 流式追尾：仅当用户本来就贴着草稿末尾时微调；
 * 正在往上读 / 刚滚过鼠标时绝不抢滚动
 */
function followEmbeddedTail() {
  if (!props.embedded || !streaming.value) return;
  const draft = rootEl.value;
  if (!draft) return;
  const scroller = resolveScroller();
  if (!scroller) return;
  if (appState.editorScrollFreezeTop != null) return;
  if (recentlyEditorScrollIntent(2200)) {
    followDetached.value = true;
    return;
  }
  if (followDetached.value) {
    clearFollowDetachIfNearTail();
    if (followDetached.value) return;
  }

  const past = distancePastFold(scroller, draft, 48);
  // 尾巴刚好探出视口一点点才跟；远离说明用户在看上方
  if (past <= 0 || past > 96) {
    if (past > 96) followDetached.value = true;
    return;
  }
  scroller.scrollTop += past;
}

function onScrollerWheel() {
  markFollowDetached();
}

function onScrollerTouch() {
  markFollowDetached();
}

function bindScrollerIntent() {
  unbindScrollerIntent();
  scrollerEl = resolveScroller();
  if (!scrollerEl) return;
  scrollerEl.addEventListener("wheel", onScrollerWheel, { passive: true });
  scrollerEl.addEventListener("touchmove", onScrollerTouch, { passive: true });
}

function unbindScrollerIntent() {
  if (!scrollerEl) return;
  scrollerEl.removeEventListener("wheel", onScrollerWheel);
  scrollerEl.removeEventListener("touchmove", onScrollerTouch);
  scrollerEl = null;
}

watch(
  () => text.value.length,
  () => {
    if (!props.embedded) return;
    nextTick(() => {
      requestAnimationFrame(followEmbeddedTail);
    });
  }
);

watch(
  () => [visible.value, streaming.value, props.embedded],
  ([vis, stream, emb]) => {
    if (vis && stream && emb) {
      followDetached.value = false;
      nextTick(bindScrollerIntent);
    } else {
      unbindScrollerIntent();
      followDetached.value = false;
    }
  },
  { immediate: true }
);

onMounted(() => {
  if (props.embedded && visible.value && streaming.value) bindScrollerIntent();
});

onUnmounted(() => {
  unbindScrollerIntent();
});

function scheduleHide() {
  if (hover.pinned) return;
  if (hideTimer.value) clearTimeout(hideTimer.value);
  hideTimer.value = setTimeout(() => {
    hover.visible = false;
    hover.entry = null;
    hideTimer.value = null;
  }, 180);
}

function onHitOver(e) {
  const el = e.target && e.target.closest && e.target.closest(".char-hit");
  if (!el || !e.currentTarget.contains(el)) return;
  if (hideTimer.value) {
    clearTimeout(hideTimer.value);
    hideTimer.value = null;
  }
  const id = el.getAttribute("data-char-id") || "";
  const entry = appState.characterById && appState.characterById[id];
  if (!entry) return;
  hover.entry = entry;
  hover.term = el.getAttribute("data-char-term") || "";
  hover.x = e.clientX;
  hover.y = e.clientY;
  hover.visible = true;
}

function onHitMove(e) {
  if (!hover.visible) return;
  if (!(e.target && e.target.closest && e.target.closest(".char-hit"))) return;
  hover.x = e.clientX;
  hover.y = e.clientY;
}

function onHitOut(e) {
  const el = e.target && e.target.closest && e.target.closest(".char-hit");
  if (!el) return;
  const to = e.relatedTarget;
  if (to && (to.closest?.(".char-hit") || to.closest?.(".char-hover-card"))) return;
  scheduleHide();
}
</script>

<template>
  <div
    v-if="visible"
    ref="rootEl"
    class="editor-draft"
    :class="{ embedded }"
  >
    <div class="draft-head">
      <span class="draft-title">{{ draftTitle }}</span>
      <span class="draft-meta muted">{{ meta }}</span>
      <span class="draft-auto muted">{{
        !streaming && String(appState.statusMessage || "").includes("停滚")
          ? "停滚约 1.4 秒后自动写入并保存"
          : "完成后自动写入并保存"
      }}</span>
    </div>
    <div
      class="draft-body"
      @mouseover="onHitOver"
      @mousemove="onHitMove"
      @mouseout="onHitOut"
      v-html="bodyHtml"
    />
    <div class="draft-actions">
      <button type="button" class="app-btn" @click="onCancel">取消</button>
    </div>
    <CharacterHoverCard
      :visible="hover.visible"
      :entry="hover.entry"
      :term="hover.term"
      :x="hover.x"
      :y="hover.y"
      @enter="hover.pinned = true"
      @leave="hover.pinned = false; scheduleHide()"
    />
  </div>
</template>

<style scoped>
.editor-draft {
  margin-top: 12px;
  padding: 12px 14px;
  background: var(--surface-solid);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-sm);
  border-left: 3px solid var(--accent);
  flex-shrink: 0;
  overflow-anchor: none;
}
.editor-draft.embedded {
  margin-top: 8px;
  margin-bottom: 4px;
}
.draft-head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px 12px;
  margin-bottom: 8px;
  overflow-anchor: none;
}
.draft-title {
  font-size: 12px;
  font-weight: 700;
  color: var(--accent-hover, var(--accent));
}
.draft-meta,
.draft-auto {
  font-size: 11px;
}
.draft-body {
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--editor-font-family);
  font-size: var(--editor-font-size);
  line-height: 1.7;
  color: var(--text);
  min-height: 3em;
  max-height: none;
  overflow: visible;
  opacity: 0.92;
}
.draft-body :deep(.char-hit) {
  background: rgba(220, 90, 120, 0.2);
  border-radius: 3px;
  cursor: help;
  box-decoration-break: clone;
  -webkit-box-decoration-break: clone;
}
.draft-body :deep(.char-hit:hover) {
  background: rgba(220, 90, 120, 0.35);
}
.draft-actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}
</style>
