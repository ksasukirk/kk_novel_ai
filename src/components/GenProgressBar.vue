<!--
  生成进度条（流式字数估算 + 无字时不确定动画）
  代码路径: kk_novel_ai/src/components/GenProgressBar.vue
-->
<script setup>
import { computed } from "vue";
import { appState } from "../stores/appState.js";
import { tropeScanState } from "../services/tropeScan.js";
import { bulkRenameState } from "../services/bulkRenameProgress.js";
import { t } from "../i18n/index.js";

defineProps({
  /** compact：顶栏细条；panel：AI 面板较粗 */
  variant: { type: String, default: "compact" },
});

const scanRunning = computed(
  () =>
    (!!tropeScanState.running || !!tropeScanState.batchRunning) && !appState.generating
);
const renameRunning = computed(
  () => !!bulkRenameState.running && !appState.generating && !scanRunning.value
);
const visible = computed(
  () =>
    !!appState.generating ||
    appState.genProgressPct >= 100 ||
    scanRunning.value ||
    renameRunning.value
);
const indeterminate = computed(() => {
  if (renameRunning.value) {
    return (bulkRenameState.total || 0) <= 0;
  }
  if (scanRunning.value) {
    return (tropeScanState.steps || 0) <= 0 && (tropeScanState.total || 0) <= 0;
  }
  return !!appState.generating && (appState.genStreamChars || 0) <= 0;
});
const pct = computed(() => {
  if (renameRunning.value) {
    return Math.max(0, Math.min(100, Number(bulkRenameState.pct) || 0));
  }
  if (scanRunning.value) {
    return Math.max(0, Math.min(100, Number(tropeScanState.pct) || 0));
  }
  return Math.max(0, Math.min(100, Number(appState.genProgressPct) || 0));
});
function shortTitle(raw, max = 10) {
  const s = String(raw || "").trim();
  if (!s) return "";
  const chars = [...s];
  if (chars.length <= max) return s;
  return `${chars.slice(0, max).join("")}…`;
}

/** 标题空时去掉多余「 · 」 */
function tidyMeter(s) {
  return String(s || "")
    .replace(/\s*·\s*·/g, " · ")
    .replace(/\s*·\s*$/g, "")
    .trim();
}

const label = computed(() => {
  if (renameRunning.value) {
    const name = shortTitle(bulkRenameState.title, 10);
    if ((bulkRenameState.total || 0) <= 0) {
      return t("progress.renamePrep");
    }
    return tidyMeter(
      t("progress.renameMeter", {
        pct: pct.value,
        current: bulkRenameState.current,
        total: bulkRenameState.total,
        title: name,
      })
    );
  }
  if (scanRunning.value) {
    const chTitle = shortTitle(tropeScanState.title);
    if (tropeScanState.phase === "structure") {
      if ((tropeScanState.total || 0) <= 0) {
        return t("trope.structureMeterPrep");
      }
      return tidyMeter(
        t("trope.structureMeter", {
          pct: pct.value,
          current: tropeScanState.current,
          total: tropeScanState.total,
          rebuilt: tropeScanState.rebuilt,
          title: chTitle,
        })
      );
    }
    // 尚未收到章数时不要写「0/0 章」，改显示准备中
    if ((tropeScanState.total || 0) <= 0) {
      return t("trope.scanMeterPrep");
    }
    if (tropeScanState.batchTotal > 1) {
      return tidyMeter(
        t("trope.scanMeterBatch", {
          book: tropeScanState.batchIndex || 1,
          books: tropeScanState.batchTotal,
          title: shortTitle(tropeScanState.batchTitle || "", 8),
          pct: pct.value,
          current: tropeScanState.current,
          total: tropeScanState.total,
          chapter: chTitle,
        })
      );
    }
    if (tropeScanState.chunk > 0 && tropeScanState.chunks > 1) {
      return tidyMeter(
        t("trope.scanMeterChunk", {
          pct: pct.value,
          current: tropeScanState.current,
          total: tropeScanState.total,
          chunk: tropeScanState.chunk,
          chunks: tropeScanState.chunks,
          title: chTitle,
        })
      );
    }
    return tidyMeter(
      t("trope.scanMeter", {
        pct: pct.value,
        current: tropeScanState.current,
        total: tropeScanState.total,
        title: chTitle,
      })
    );
  }
  if (!appState.generating && pct.value >= 100) return t("progress.done");
  if (indeterminate.value) return t("progress.connecting");
  const chars = appState.genStreamChars || 0;
  return t("progress.chars", { n: chars, pct: pct.value });
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
    :aria-busy="!!appState.generating || scanRunning || renameRunning"
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
  width: 260px;
  max-width: min(42vw, 320px);
  flex: 0 1 260px;
}
.gen-progress.compact .label {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
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
