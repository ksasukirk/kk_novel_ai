<!--
  用量趋势折线（纯 SVG）
  代码路径: kk_novel_ai/src/components/analytics/UsageTrendChart.vue
-->
<script setup>
import { computed } from "vue";

const props = defineProps({
  /** [{ date, cost, tokens }] */
  series: { type: Array, default: () => [] },
  /** cost | tokens */
  metric: { type: String, default: "cost" },
  title: { type: String, default: "" },
  subtitle: { type: String, default: "" },
  estimate: { type: Boolean, default: false },
});

const W = 560;
const H = 160;
const PAD = { t: 16, r: 12, b: 28, l: 44 };

const values = computed(() =>
  (props.series || []).map((d) =>
    props.metric === "tokens" ? Number(d.tokens) || 0 : Number(d.cost) || 0
  )
);

const maxY = computed(() => {
  const m = Math.max(...values.value, 0);
  return m > 0 ? m * 1.1 : 1;
});

const points = computed(() => {
  const n = values.value.length;
  if (n === 0) return "";
  const iw = W - PAD.l - PAD.r;
  const ih = H - PAD.t - PAD.b;
  return values.value
    .map((v, i) => {
      const x = PAD.l + (n === 1 ? iw / 2 : (i / (n - 1)) * iw);
      const y = PAD.t + ih - (v / maxY.value) * ih;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
});

const xLabels = computed(() => {
  const arr = props.series || [];
  if (arr.length <= 1) return arr.map((d, i) => ({ i, label: (d.date || "").slice(5) }));
  const idxs = [0, Math.floor((arr.length - 1) / 2), arr.length - 1];
  return [...new Set(idxs)].map((i) => ({
    i,
    label: String(arr[i].date || "").slice(5),
  }));
});

function labelX(i) {
  const n = values.value.length;
  const iw = W - PAD.l - PAD.r;
  return PAD.l + (n === 1 ? iw / 2 : (i / (n - 1)) * iw);
}

const yTop = computed(() => {
  if (props.metric === "tokens") return Math.round(maxY.value).toLocaleString();
  return maxY.value < 0.01 ? maxY.value.toFixed(4) : maxY.value.toFixed(2);
});
</script>

<template>
  <div class="chart-card" :class="{ estimate }">
    <div class="chart-head">
      <h3 class="chart-title">{{ title }}</h3>
      <p v-if="subtitle" class="muted chart-sub">{{ subtitle }}</p>
    </div>
    <svg
      class="trend-svg"
      :viewBox="`0 0 ${W} ${H}`"
      preserveAspectRatio="xMidYMid meet"
      role="img"
    >
      <line
        :x1="PAD.l"
        :y1="PAD.t"
        :x2="PAD.l"
        :y2="H - PAD.b"
        class="axis"
      />
      <line
        :x1="PAD.l"
        :y1="H - PAD.b"
        :x2="W - PAD.r"
        :y2="H - PAD.b"
        class="axis"
      />
      <text :x="PAD.l - 6" :y="PAD.t + 4" class="tick" text-anchor="end">{{ yTop }}</text>
      <text :x="PAD.l - 6" :y="H - PAD.b" class="tick" text-anchor="end">0</text>
      <polyline
        v-if="points"
        :points="points"
        fill="none"
        class="line"
        :class="{ 'line-est': estimate }"
      />
      <template v-for="lab in xLabels" :key="lab.i">
        <text :x="labelX(lab.i)" :y="H - 8" class="tick" text-anchor="middle">
          {{ lab.label }}
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
  background: var(--panel-bg, transparent);
}
.chart-card.estimate {
  border-style: dashed;
  opacity: 0.95;
}
.chart-head {
  margin-bottom: 0.35rem;
}
.chart-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 600;
}
.chart-sub {
  margin: 0.2rem 0 0;
  font-size: 0.8rem;
}
.trend-svg {
  width: 100%;
  height: auto;
  display: block;
}
.axis {
  stroke: var(--muted, #888);
  stroke-width: 1;
  opacity: 0.5;
}
.line {
  stroke: var(--accent, #6cb6ff);
  stroke-width: 2;
}
.line-est {
  stroke-dasharray: 6 4;
  stroke: var(--warn, #c9a227);
}
.tick {
  fill: var(--muted, #888);
  font-size: 10px;
}
</style>
