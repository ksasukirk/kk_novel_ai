<!--
  发现新版本：确认 → 下载进度 → 启动新进程
  代码路径: kk_novel_ai/src/components/UpdateDialog.vue
-->
<script setup>
import { computed } from "vue";
import { createBackdropDismiss } from "../utils/backdropDismiss.js";
import {
  confirmUpdateDownloadAndLaunch,
  dismissUpdateFlow,
  formatUpdateMb,
  formatUpdateSpeedMbs,
  updateFlow,
  updateFlowPct,
} from "../services/updateFlow.js";

const backdrop = createBackdropDismiss(() => dismissUpdateFlow());
const busy = computed(
  () => updateFlow.phase === "downloading" || updateFlow.phase === "launching",
);
const pct = computed(() => updateFlowPct());
const receivedMb = computed(() => formatUpdateMb(updateFlow.received));
const totalMb = computed(() => {
  const t = Number(updateFlow.total) || 0;
  return t > 0 ? formatUpdateMb(t) : "?";
});
const speedMbs = computed(() =>
  formatUpdateSpeedMbs(updateFlow.received, updateFlow.startedAt),
);
const notes = computed(() => {
  const n = String((updateFlow.info && updateFlow.info.notes) || "").trim();
  if (!n) return "";
  return n.length > 220 ? `${n.slice(0, 220)}…` : n;
});

function onConfirm() {
  if (busy.value) return;
  if (updateFlow.phase === "error") {
    dismissUpdateFlow();
    return;
  }
  void confirmUpdateDownloadAndLaunch();
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="updateFlow.open"
      class="update-mask"
      role="dialog"
      aria-modal="true"
      aria-label="发现新版本"
      @mousedown="!busy && backdrop.onMouseDown($event)"
      @click="!busy && backdrop.onClick($event)"
    >
      <div class="update-card">
        <h2 class="update-title">发现新版本</h2>
        <template v-if="updateFlow.phase === 'prompt'">
          <p class="update-message">
            当前 {{ updateFlow.info && updateFlow.info.current }}，可更新到
            {{ updateFlow.info && updateFlow.info.latest }}。确认后会下载到临时目录并启动新程序，本窗口将关闭。
          </p>
          <p v-if="notes" class="update-notes">{{ notes }}</p>
        </template>
        <template v-else-if="updateFlow.phase === 'downloading'">
          <p class="update-message">正在下载 {{ updateFlow.info && updateFlow.info.latest }}…</p>
          <div class="update-bar" aria-hidden="true">
            <span class="update-bar-fill" :style="{ width: `${pct}%` }" />
          </div>
          <p class="update-progress">
            已下载 {{ receivedMb }} / {{ totalMb }} MB（{{ pct }}%），平均 {{ speedMbs }} MB/s
          </p>
        </template>
        <template v-else-if="updateFlow.phase === 'launching'">
          <p class="update-message">下载完成，正在启动新版本…</p>
        </template>
        <template v-else>
          <p class="update-message">{{ updateFlow.error || "更新失败" }}</p>
        </template>
        <div class="update-actions">
          <button
            v-if="updateFlow.phase === 'prompt'"
            type="button"
            class="app-btn"
            @click="dismissUpdateFlow"
          >
            稍后
          </button>
          <button
            v-if="updateFlow.phase === 'prompt' || updateFlow.phase === 'error'"
            type="button"
            class="app-btn app-btn-primary"
            @click="onConfirm"
          >
            {{ updateFlow.phase === "error" ? "知道了" : "下载并启动" }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.update-mask {
  position: fixed;
  inset: 0;
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background: rgba(20, 16, 24, 0.45);
  backdrop-filter: blur(4px);
}
.update-card {
  width: min(420px, 100%);
  padding: 22px 24px;
  border-radius: var(--radius-lg, 14px);
  background: var(--panel, #fff);
  box-shadow: var(--shadow, 0 12px 40px rgba(0, 0, 0, 0.18));
  box-sizing: border-box;
}
.update-title {
  margin: 0 0 10px;
  font-size: 16px;
  font-weight: 650;
  color: var(--text);
}
.update-message {
  margin: 0 0 14px;
  color: var(--muted);
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}
.update-notes {
  margin: 0 0 16px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--muted);
  max-height: 96px;
  overflow: auto;
  white-space: pre-wrap;
}
.update-bar {
  height: 8px;
  margin: 0 0 10px;
  border-radius: 99px;
  background: var(--panel-2, #eee);
  overflow: hidden;
}
.update-bar-fill {
  display: block;
  height: 100%;
  border-radius: 99px;
  background: var(--accent, #e8a0b4);
  transition: width 0.15s linear;
}
.update-progress {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--muted);
}
.update-actions {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 8px;
}
</style>
