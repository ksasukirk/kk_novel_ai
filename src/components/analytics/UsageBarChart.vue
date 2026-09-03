<!--
  用量柱状图（纯 SVG）
  代码路径: kk_novel_ai/src/components/analytics/UsageBarChart.vue
-->
<script setup>
import { computed } from "vue";

const props = defineProps({
  /** [{ name, cost, tokens, calls }] */
  rows: { type: Array, default: () => [] },
  /** cost | tokens | calls */
  metric: { type: String, default: "cost" },
  title: { type: String, default: "" },
  maxBars: { type: Number, default: 8 },
});

const W = 560;
const ROW_H = 22;
const PAD = { t: 8, r: 12, b: 8, l: 100 };

const sliced = computed(() => (props.rows || []).slice(0, props.maxBars));

const H = computed(() => PAD.t + PAD.b + Math.max(sliced.value.length, 1) * ROW_H);

const maxV = computed(() => {
  const vals = sliced.value.map((r) => Number(r[props.metric]) || 0);
  const m = Math.max(...vals, 0);
  return m > 0 ? m : 1;
});

function barWidth(row) {
  const iw = W - PAD.l - PAD.r;
  return ((Number(row[props.metric]) || 0) / maxV.value) * iw;
}

function barY(i) {
  return PAD.t + i * ROW_H + 4;
}

function fmt(row) {
  const v = Number(row[props.metric]) || 0;
  if (props.metric === "cost") return `¥${v.toFixed(4)}`;
  if (props.metric === "tokens") return `${Math.round(v)}`;
  return `${Math.round(v)}`;
}

function shortName(n) {
  const s = String(n || "");
  return s.length > 14 ? s.slice(0, 13) + "…" : s;
}
</script>

<template>
  <div class="chart-card">
    <h3 class="chart-title">{{ title }}</h3>
    <p v-if="!sliced.length" class="muted empty">暂无数据</p>
    <svg
      v-else
      class="bar-svg"
      :viewBox="`0 0 ${W} ${H}`"
      preserveAspectRatio="xMidYMid meet"
      role="img"
    >
      <template v-for="(row, i) in sliced" :key="row.name + i">
        <text :x="PAD.l - 6" :y="barY(i) + 11" class="label" text-anchor="end">
          {{ shortName(row.name) }}
        </text>
        <rect
          :x="PAD.l"
          :y="barY(i)"
          :width="Math.max(barWidth(row), 1)"
          height="14"
          class="bar"
          rx="2"
        />
        <text :x="PAD.l + barWidth(row) + 4" :y="barY(i) + 11" class="val">
          {{ fmt(row) }}
        </text>
      </template>
    </svg>
  </div>
</template>

<style scoped>
.chart-card {
  margin: 0.75rem 0 1rem;
  padding: 0.65rem 0.75rem;
  border: 1px solid var(--border, #333);
  border-radius: 8px;
}
.chart-title {
  margin: 0 0 0.35rem;
  font-size: 0.95rem;
  font-weight: 600;
}
.empty {
  margin: 0.25rem 0;
  font-size: 0.85rem;
}
.bar-svg {
  width: 100%;
  height: auto;
  display: block;
}
.label,
.val {
  fill: var(--muted, #888);
  font-size: 10px;
}
.bar {
  fill: var(--accent, #6cb6ff);
  opacity: 0.85;
}
</style>
