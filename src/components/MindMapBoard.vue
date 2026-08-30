<!--
  思维导图画布（SVG，可平移缩放）
  代码路径: kk_novel_ai/src/components/MindMapBoard.vue
-->
<script setup>
import { computed, ref, watch } from "vue";
import { layoutMindMap } from "../utils/mindmapLayout.js";

const props = defineProps({
  tree: { type: Object, required: true },
  height: { type: Number, default: 420 },
});

const emit = defineEmits(["select"]);

const scale = ref(1);
const panX = ref(0);
const panY = ref(0);
const dragging = ref(false);
const last = ref({ x: 0, y: 0 });

const layout = computed(() => {
  if (!props.tree || !props.tree.id) {
    return { nodes: [], edges: [], width: 480, height: 280 };
  }
  return layoutMindMap(props.tree, {
    nodeWidth: 148,
    nodeHeight: 38,
    gapX: 52,
    gapY: 12,
  });
});

const viewW = computed(() => layout.value.width);
const viewH = computed(() => Math.max(layout.value.height, props.height));

watch(
  () => props.tree,
  () => {
    panX.value = 0;
    panY.value = 0;
    scale.value = 1;
  }
);

function onWheel(e) {
  e.preventDefault();
  const delta = e.deltaY > 0 ? -0.08 : 0.08;
  scale.value = Math.min(2.2, Math.max(0.45, scale.value + delta));
}

function onPointerDown(e) {
  if (e.button !== 0) return;
  dragging.value = true;
  last.value = { x: e.clientX, y: e.clientY };
  e.currentTarget.setPointerCapture?.(e.pointerId);
}

function onPointerMove(e) {
  if (!dragging.value) return;
  panX.value += e.clientX - last.value.x;
  panY.value += e.clientY - last.value.y;
  last.value = { x: e.clientX, y: e.clientY };
}

function onPointerUp() {
  dragging.value = false;
}

function zoomIn() {
  scale.value = Math.min(2.2, scale.value + 0.15);
}
function zoomOut() {
  scale.value = Math.max(0.45, scale.value - 0.15);
}
function resetView() {
  scale.value = 1;
  panX.value = 0;
  panY.value = 0;
}

function onNodeClick(n) {
  emit("select", n);
}

function pathD(e) {
  const mx = (e.x1 + e.x2) / 2;
  return `M ${e.x1} ${e.y1} C ${mx} ${e.y1}, ${mx} ${e.y2}, ${e.x2} ${e.y2}`;
}
</script>

<template>
  <div class="mm-wrap">
    <div class="mm-toolbar">
      <button type="button" class="app-btn" @click="zoomOut">缩小</button>
      <button type="button" class="app-btn" @click="zoomIn">放大</button>
      <button type="button" class="app-btn" @click="resetView">复位</button>
      <span class="muted tip">拖动画布平移 · 滚轮缩放 · 点击节点</span>
    </div>
    <div
      class="mm-viewport"
      :style="{ height: height + 'px' }"
      @wheel="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointerleave="onPointerUp"
    >
      <svg
        class="mm-svg"
        :viewBox="`0 0 ${viewW} ${viewH}`"
        :width="viewW * scale"
        :height="viewH * scale"
        :style="{ transform: `translate(${panX}px, ${panY}px)` }"
      >
        <path
          v-for="e in layout.edges"
          :key="e.id"
          :d="pathD(e)"
          class="mm-link"
          fill="none"
        />
        <g
          v-for="n in layout.nodes"
          :key="n.id"
          class="mm-node"
          :class="'k-' + n.kind"
          @click.stop="onNodeClick(n)"
        >
          <rect
            :x="n.x"
            :y="n.y"
            :width="n.w"
            :height="n.h"
            rx="12"
            ry="12"
          />
          <text :x="n.x + n.w / 2" :y="n.y + n.h / 2 + 1" text-anchor="middle" dominant-baseline="middle">
            {{ n.label.length > 14 ? n.label.slice(0, 13) + "…" : n.label }}
          </text>
          <title>{{ n.label }}{{ n.meta ? " — " + n.meta : "" }}</title>
        </g>
      </svg>
      <p v-if="!layout.nodes.length" class="empty muted">暂无导图数据，请先填写大纲或总谱。</p>
    </div>
  </div>
</template>

<style scoped>
.mm-wrap {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.mm-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}
.tip {
  font-size: 12px;
}
.mm-viewport {
  position: relative;
  overflow: hidden;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
  cursor: grab;
  touch-action: none;
}
.mm-viewport:active {
  cursor: grabbing;
}
.mm-svg {
  display: block;
  transform-origin: 0 0;
}
.mm-link {
  stroke: var(--muted);
  stroke-width: 1.6;
  opacity: 0.55;
}
.mm-node {
  cursor: pointer;
}
.mm-node rect {
  fill: var(--panel);
  stroke: var(--divider);
  stroke-width: 1;
  filter: drop-shadow(0 2px 6px rgba(0, 0, 0, 0.06));
}
.mm-node text {
  font-size: 11px;
  font-weight: 650;
  fill: var(--text);
  pointer-events: none;
}
.mm-node:hover rect {
  stroke: var(--accent-hover);
}
.k-root rect {
  fill: var(--accent-soft);
  stroke: var(--accent-hover);
}
.k-branch rect {
  fill: rgba(232, 93, 117, 0.12);
  stroke: var(--accent);
}
.k-volume rect,
.k-chapter rect {
  fill: var(--panel);
}
.k-beat rect {
  fill: rgba(100, 140, 200, 0.12);
}
.k-arc-main rect {
  fill: rgba(232, 93, 117, 0.2);
  stroke: var(--accent-hover);
}
.k-arc rect {
  fill: rgba(232, 93, 117, 0.1);
}
.k-promise-open rect {
  fill: rgba(230, 160, 40, 0.18);
}
.k-event rect {
  fill: rgba(80, 160, 120, 0.14);
}
.k-canon-locked rect {
  fill: rgba(200, 60, 60, 0.16);
  stroke: #c44;
}
.k-relation rect {
  fill: rgba(120, 100, 200, 0.12);
}
.k-character rect {
  fill: rgba(232, 93, 150, 0.16);
  stroke: var(--accent);
}
.k-section rect {
  fill: rgba(100, 140, 200, 0.12);
}
.k-variant rect {
  fill: var(--panel);
}
.k-activeVariant rect {
  fill: var(--accent-soft);
  stroke: var(--accent-hover);
  stroke-width: 2;
}
.empty {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0;
}
</style>
