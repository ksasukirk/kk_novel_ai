<!--
  关系网力导向图谱（对照 Obsidian：d3-force + 拖拽/缩放）
  代码路径: kk_novel_ai/src/components/RelationForceGraph.vue
-->
<script setup>
import { computed, markRaw, onMounted, onUnmounted, ref, watch } from "vue";
import {
  DEFAULT_FORCES,
  GRAPH_WORLD,
  applyForces,
  buildRelationGraph,
  clearPins,
  createRelationSimulation,
  endpoint,
  graphSignature,
  mergeNodePositions,
} from "../utils/relationForceLayout.js";

const props = defineProps({
  edges: { type: Array, default: () => [] },
  loreItems: { type: Array, default: () => [] },
  height: { type: [Number, String], default: 420 },
  fill: { type: Boolean, default: false },
});

const emit = defineEmits(["select"]);

const worldW = GRAPH_WORLD.width;
const worldH = GRAPH_WORLD.height;

const nodes = ref([]);
const links = ref([]);
const frame = ref(0);
const scale = ref(1);
const panX = ref(0);
const panY = ref(0);
const panning = ref(false);
const last = ref({ x: 0, y: 0 });
const selectedId = ref("");
const hoverId = ref("");
const hoverEdgeId = ref("");
const viewportRef = ref(null);

const center = ref(DEFAULT_FORCES.center);
const chargeAbs = ref(Math.abs(DEFAULT_FORCES.charge));
const linkDistance = ref(DEFAULT_FORCES.linkDistance);
const collidePad = ref(DEFAULT_FORCES.collidePad);
const showForces = ref(false);

let sim = null;
let draggingNode = null;
let dragMoved = false;
let panMoved = false;

const forces = computed(() => ({
  center: Number(center.value),
  charge: -Math.abs(Number(chargeAbs.value)),
  linkDistance: Number(linkDistance.value),
  collidePad: Number(collidePad.value),
}));

const neighborIds = computed(() => {
  const id = selectedId.value;
  const set = new Set();
  if (!id) return set;
  set.add(id);
  for (const e of links.value) {
    const a = endpoint(e.source);
    const b = endpoint(e.target);
    const sid = a ? a.id : e.source;
    const tid = b ? b.id : e.target;
    if (sid === id) set.add(tid);
    if (tid === id) set.add(sid);
  }
  return set;
});

const hasGraph = computed(() => nodes.value.length > 0);
const fillHeight = computed(() => {
  if (props.fill) return true;
  return String(props.height) === "100%";
});
const viewportStyle = computed(() => {
  if (fillHeight.value) return {};
  const h = Number(props.height);
  return { height: (Number.isFinite(h) ? h : 420) + "px" };
});

function bump() {
  frame.value += 1;
}

function stopSim() {
  if (sim) {
    sim.stop();
    sim = null;
  }
}

function restartSim(keepPositions) {
  stopSim();
  const built = buildRelationGraph(props.edges, props.loreItems);
  if (keepPositions) mergeNodePositions(nodes.value, built.nodes);
  nodes.value = built.nodes.map((n) => markRaw(n));
  links.value = built.links.map((l) => markRaw(l));
  if (!built.nodes.length) {
    bump();
    return;
  }
  sim = createRelationSimulation(built.nodes, built.links, {
    width: worldW,
    height: worldH,
    forces: forces.value,
    onTick: bump,
  });
  sim.tick();
  bump();
}

function clientToWorld(clientX, clientY) {
  const el = viewportRef.value;
  if (!el) return { x: 0, y: 0 };
  const rect = el.getBoundingClientRect();
  return {
    x: (clientX - rect.left - panX.value) / scale.value,
    y: (clientY - rect.top - panY.value) / scale.value,
  };
}

function onWheel(e) {
  e.preventDefault();
  const delta = e.deltaY > 0 ? -0.08 : 0.08;
  scale.value = Math.min(2.4, Math.max(0.35, scale.value + delta));
}

function onPointerDown(e) {
  if (e.button !== 0 || draggingNode) return;
  panning.value = true;
  panMoved = false;
  last.value = { x: e.clientX, y: e.clientY };
  e.currentTarget.setPointerCapture?.(e.pointerId);
}

function onPointerMove(e) {
  if (draggingNode) {
    const w = clientToWorld(e.clientX, e.clientY);
    if (Math.hypot(w.x - draggingNode.x, w.y - draggingNode.y) > 2) dragMoved = true;
    draggingNode.fx = w.x;
    draggingNode.fy = w.y;
    bump();
    return;
  }
  if (!panning.value) return;
  const dx = e.clientX - last.value.x;
  const dy = e.clientY - last.value.y;
  if (Math.abs(dx) + Math.abs(dy) > 2) panMoved = true;
  panX.value += dx;
  panY.value += dy;
  last.value = { x: e.clientX, y: e.clientY };
}

function onPointerUp() {
  panning.value = false;
  if (!draggingNode) return;
  const node = draggingNode;
  draggingNode = null;
  if (sim) sim.alphaTarget(0);
  if (!dragMoved) {
    node.fx = null;
    node.fy = null;
    selectNode(node);
  }
}

function onNodeDown(e, node) {
  e.stopPropagation();
  e.preventDefault();
  panning.value = false;
  draggingNode = node;
  dragMoved = false;
  node.fx = node.x;
  node.fy = node.y;
  if (sim) sim.alphaTarget(0.22).restart();
}

function selectNode(node) {
  selectedId.value = node.id;
  emit("select", { id: node.id, label: node.label });
}

function onCanvasClick(e) {
  if (dragMoved || panMoved) return;
  if (e.target && e.target.closest && e.target.closest(".rg-node")) return;
  selectedId.value = "";
  emit("select", null);
}

function zoomIn() {
  scale.value = Math.min(2.4, scale.value + 0.15);
}
function zoomOut() {
  scale.value = Math.max(0.35, scale.value - 0.15);
}
function resetView() {
  scale.value = 1;
  panX.value = 0;
  panY.value = 0;
  clearPins(nodes.value);
  if (sim) sim.alpha(0.7).restart();
}

function edgeVisible(e) {
  void frame.value;
  const a = endpoint(e.source);
  const b = endpoint(e.target);
  if (!a || !b) return false;
  if (!selectedId.value && !hoverId.value) return false;
  const focus = selectedId.value || hoverId.value;
  return a.id === focus || b.id === focus;
}

function edgeClass(e) {
  void frame.value;
  const a = endpoint(e.source);
  const b = endpoint(e.target);
  if (!a || !b) return "";
  const dim = selectedId.value && !neighborIds.value.has(a.id) && !neighborIds.value.has(b.id);
  const hot = hoverEdgeId.value === e.id || (selectedId.value && (a.id === selectedId.value || b.id === selectedId.value));
  return { dim, hot };
}

function nodeClass(n) {
  void frame.value;
  return {
    selected: n.id === selectedId.value,
    dim: selectedId.value && !neighborIds.value.has(n.id),
    hover: n.id === hoverId.value,
  };
}

watch(
  () => graphSignature(props.edges),
  () => restartSim(true)
);

watch(
  () => (props.loreItems || []).map((x) => `${x.id}:${x.title}`).join("|"),
  () => {
    const byId = new Map((props.loreItems || []).map((x) => [x.id, x]));
    for (const n of nodes.value) {
      const lore = byId.get(n.id);
      if (lore && lore.title) n.label = lore.title;
    }
    bump();
  }
);

watch(
  forces,
  (f) => {
    if (sim) applyForces(sim, f, worldW, worldH);
  },
  { deep: true }
);

onMounted(() => restartSim(false));
onUnmounted(() => stopSim());
</script>

<template>
  <div class="rg-wrap" :class="{ fill: fillHeight }">
    <div
      class="rg-toolbar"
      title="拖动画布平移 · 拖节点钉住 · 滚轮缩放 · 点击高亮邻居"
    >
      <slot name="toolbar" />
      <button type="button" class="app-btn" @click="zoomOut">缩小</button>
      <button type="button" class="app-btn" @click="zoomIn">放大</button>
      <button type="button" class="app-btn" @click="resetView">复位</button>
      <button
        type="button"
        class="app-btn"
        :class="{ 'chip-on': showForces }"
        @click="showForces = !showForces"
      >
        力
      </button>
    </div>
    <div v-if="showForces" class="rg-forces">
      <label class="rg-force">
        中心
        <input v-model.number="center" type="range" min="0.02" max="0.45" step="0.01" />
      </label>
      <label class="rg-force">
        斥力
        <input v-model.number="chargeAbs" type="range" min="40" max="360" step="5" />
      </label>
      <label class="rg-force">
        连线距离
        <input v-model.number="linkDistance" type="range" min="40" max="180" step="2" />
      </label>
      <label class="rg-force">
        碰撞
        <input v-model.number="collidePad" type="range" min="2" max="36" step="1" />
      </label>
    </div>
    <div
      ref="viewportRef"
      class="rg-viewport"
      :style="viewportStyle"
      @wheel="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointerleave="onPointerUp"
      @click="onCanvasClick"
    >
      <svg
        class="rg-svg"
        :viewBox="`0 0 ${worldW} ${worldH}`"
        :width="worldW * scale"
        :height="worldH * scale"
        :style="{ transform: `translate(${panX}px, ${panY}px)` }"
      >
        <g :data-frame="frame">
          <g v-for="e in links" :key="e.id">
            <line
              v-if="endpoint(e.source) && endpoint(e.target)"
              :x1="endpoint(e.source).x"
              :y1="endpoint(e.source).y"
              :x2="endpoint(e.target).x"
              :y2="endpoint(e.target).y"
              class="rg-edge"
              :class="edgeClass(e)"
              @pointerenter="hoverEdgeId = e.id"
              @pointerleave="hoverEdgeId = ''"
            />
            <text
              v-if="edgeVisible(e) && endpoint(e.source) && endpoint(e.target)"
              :x="(endpoint(e.source).x + endpoint(e.target).x) / 2"
              :y="(endpoint(e.source).y + endpoint(e.target).y) / 2 - 6"
              class="rg-elabel"
              text-anchor="middle"
            >
              {{ e.label }}
            </text>
          </g>
          <g
            v-for="n in nodes"
            :key="n.id"
            class="rg-node"
            :class="nodeClass(n)"
            @pointerdown="onNodeDown($event, n)"
            @pointerenter="hoverId = n.id"
            @pointerleave="hoverId = ''"
          >
            <circle :cx="n.x" :cy="n.y" :r="n.r" />
            <text :x="n.x" :y="n.y + n.r + 12" text-anchor="middle">{{ n.label }}</text>
            <title>{{ n.label }}</title>
          </g>
        </g>
      </svg>
      <p v-if="!hasGraph" class="empty muted">暂无关系边。点「加边」后会出现节点。</p>
    </div>
  </div>
</template>

<style scoped>
.rg-wrap {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 0;
  min-height: 0;
}
.rg-wrap.fill {
  flex: 1;
  min-height: 0;
}
.rg-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  flex-shrink: 0;
}
.rg-toolbar .app-btn {
  padding: 4px 10px;
  font-size: 12px;
}
.rg-toolbar .chip-on {
  background: var(--accent-soft);
  color: var(--accent-hover);
}
.rg-forces {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px 12px;
}
.rg-force {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 11px;
  color: var(--muted);
  font-weight: 600;
}
.rg-force input {
  width: 100%;
  accent-color: var(--accent-hover);
}
.rg-viewport {
  position: relative;
  overflow: hidden;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
  cursor: grab;
  touch-action: none;
}
.rg-wrap.fill .rg-viewport {
  flex: 1;
  min-height: 0;
  height: auto;
}
.rg-viewport:active {
  cursor: grabbing;
}
.rg-svg {
  display: block;
  transform-origin: 0 0;
}
.rg-edge {
  stroke: var(--muted);
  stroke-width: 1.4;
  opacity: 0.55;
  pointer-events: stroke;
}
.rg-edge.hot {
  stroke: var(--accent-hover);
  stroke-width: 2;
  opacity: 0.9;
}
.rg-edge.dim {
  opacity: 0.12;
}
.rg-elabel {
  font-size: 10px;
  fill: var(--muted);
  pointer-events: none;
}
.rg-node {
  cursor: pointer;
}
.rg-node circle {
  fill: var(--accent-soft);
  stroke: var(--accent-hover);
  stroke-width: 1.4;
}
.rg-node text {
  font-size: 11px;
  font-weight: 650;
  fill: var(--text);
  pointer-events: none;
}
.rg-node.hover circle,
.rg-node.selected circle {
  stroke-width: 2.4;
  filter: drop-shadow(0 0 6px color-mix(in srgb, var(--accent-hover) 55%, transparent));
}
.rg-node.dim {
  opacity: 0.28;
}
.rg-node.dim text {
  opacity: 0.7;
}
.empty {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0;
}
@media (max-width: 720px) {
  .rg-forces {
    grid-template-columns: 1fr 1fr;
  }
}
</style>
