<!--
  生成进度条（流式字数估算 + 无字时不确定动画）
  代码路径: kk_novel_ai/src/components/GenProgressBar.vue
-->
<script setup>
import { computed } from "vue";
import { appState } from "../stores/appState.js";

const props = defineProps({
  /** compact：顶栏细条；panel：AI 面板较粗 */
  variant: { type: String, default: "compact" },
});

const visible = computed(() => !!appState.generating || appState.genProgressPct >= 100);
const indeterminate = computed(
  () => !!appState.generating && (appState.genStreamChars || 0) <= 0
);
const pct = computed(() => Math.max(0, Math.min(100, Number(appState.genProgressPct) || 0)));
const label = computed(() => {
  if (!appState.generating && pct.value >= 100) return "完成";
  if (indeterminate.value) return "连接模型…";
  const chars = appState.genStreamChars || 0;
  return `${chars} 字 · ${pct.value}%`;
});
</script>

<template>
  <div
    v-if="visible"
    class="gen-progress"
    :class="[variant, { indeterminate }]"
    role="progressbar"
    :aria-valuenow="indeterminate ? undefined : pct"
    aria-valuemin="0"
    aria-valuemax="100"
    :aria-busy="!!appState.generating"
  >
    <div class="track">
      <div
        class="fill"
        :style="indeterminate ? undefined : { width: pct + '%' }"
      />
    </div>
    <span class="label">{{ label }}</span>
  </div>
</template>

<style scoped>
.gen-progress {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 8px;
  min-width: 0;
  /* 禁止在纵轴 flex 布局里被撑成整列空白 */
  flex: 0 0 auto;
  height: 22px;
  max-height: 28px;
  box-sizing: border-box;
}
.gen-progress.compact {
  width: 168px;
  max-width: 200px;
  flex: 0 0 168px;
}
.gen-progress.panel {
  width: 100%;
  margin: 0;
  height: 24px;
  max-height: 28px;
}
.track {
  flex: 1 1 auto;
  height: 6px;
  border-radius: 999px;
  background: var(--chip-bg, rgba(0, 0, 0, 0.08));
  overflow: hidden;
  min-width: 48px;
  max-height: 8px;
  align-self: center;
}
.panel .track {
  height: 8px;
}
.fill {
  height: 100%;
  width: 0;
  border-radius: inherit;
  background: linear-gradient(90deg, var(--accent, #f472b6), var(--accent-hover, #ec4899));
  transition: width 0.2s ease-out;
}
.indeterminate .fill {
  width: 36%;
  animation: gen-indet 1.1s ease-in-out infinite;
}
.label {
  flex-shrink: 0;
  font-size: 11px;
  font-weight: 600;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  line-height: 1;
}
.panel .label {
  font-size: 12px;
}
@keyframes gen-indet {
  0% {
    transform: translateX(-120%);
  }
  100% {
    transform: translateX(320%);
  }
}
</style>
