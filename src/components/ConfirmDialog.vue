<!--
  全局自定义确认/提示弹窗
  代码路径: kk_novel_ai/src/components/ConfirmDialog.vue
-->
<script setup>
import { computed, onMounted, onUnmounted, watch, nextTick, ref } from "vue";
import {
  confirmState,
  resolveAppConfirm,
  cancelAppConfirm,
} from "../services/confirmDialog.js";
import { createBackdropDismiss } from "../utils/backdropDismiss.js";

const primaryBtn = ref(null);
const confirmBackdrop = createBackdropDismiss(() => cancelAppConfirm());

const visible = computed(() => confirmState.open);
const isAlert = computed(() => confirmState.mode === "alert");

function onKey(e) {
  if (!confirmState.open) return;
  if (e.key === "Escape") {
    e.preventDefault();
    cancelAppConfirm();
  } else if (e.key === "Enter") {
    e.preventDefault();
    resolveAppConfirm(true);
  }
}

watch(visible, async (v) => {
  if (!v) return;
  await nextTick();
  primaryBtn.value?.focus?.();
});

onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="confirm-mask"
      role="dialog"
      aria-modal="true"
      :aria-label="confirmState.title"
      @mousedown="confirmBackdrop.onMouseDown"
      @click="confirmBackdrop.onClick"
    >
      <div class="confirm-card">
        <h2 class="confirm-title">{{ confirmState.title }}</h2>
        <p class="confirm-message">{{ confirmState.message }}</p>
        <div class="confirm-actions">
          <button
            v-if="!isAlert"
            type="button"
            class="app-btn"
            @click="cancelAppConfirm"
          >
            {{ confirmState.cancelText }}
          </button>
          <button
            v-if="!isAlert && confirmState.extraText"
            type="button"
            class="app-btn"
            @click="resolveAppConfirm('extra')"
          >
            {{ confirmState.extraText }}
          </button>
          <button
            ref="primaryBtn"
            type="button"
            class="app-btn"
            :class="confirmState.danger ? 'app-btn-danger' : 'app-btn-primary'"
            @click="resolveAppConfirm(true)"
          >
            {{ confirmState.confirmText }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.confirm-mask {
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
.confirm-card {
  width: min(420px, 100%);
  padding: 22px 24px;
  border-radius: var(--radius-lg, 14px);
  background: var(--panel, #fff);
  box-shadow: var(--shadow, 0 12px 40px rgba(0, 0, 0, 0.18));
  box-sizing: border-box;
}
.confirm-title {
  margin: 0 0 10px;
  font-size: 16px;
  font-weight: 650;
  color: var(--text);
}
.confirm-message {
  margin: 0 0 18px;
  color: var(--muted);
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}
.confirm-actions {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 8px;
}
</style>
