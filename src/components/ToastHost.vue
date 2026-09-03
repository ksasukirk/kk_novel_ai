<!--
  全局浮窗提示队列（窗口顶部居中）
  代码路径: kk_novel_ai/src/components/ToastHost.vue
-->
<script setup>
import { toastState, dismissToast } from "../services/toast.js";

function label(type) {
  if (type === "error") return "错误";
  if (type === "warning") return "提示";
  if (type === "success") return "完成";
  return "消息";
}
</script>

<template>
  <div class="toast-host" aria-live="polite" aria-relevant="additions">
    <TransitionGroup name="toast" tag="div" class="toast-stack">
      <div
        v-for="item in toastState.items"
        :key="item.id"
        class="toast-item"
        :class="'toast-' + item.type"
        role="alert"
      >
        <span class="toast-tag">{{ label(item.type) }}</span>
        <p class="toast-msg">{{ item.message }}</p>
        <button
          type="button"
          class="toast-close"
          aria-label="关闭"
          @click="dismissToast(item.id)"
        >
          ×
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-host {
  position: fixed;
  top: calc(46px + env(safe-area-inset-top, 0px));
  left: 50%;
  transform: translateX(-50%);
  z-index: 960;
  width: min(520px, calc(100vw - 24px));
  pointer-events: none;
}

.toast-stack {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.toast-item {
  pointer-events: auto;
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: start;
  gap: 8px 10px;
  padding: 10px 12px;
  border-radius: var(--radius-md);
  background: var(--panel);
  box-shadow: var(--shadow);
  border: 1px solid var(--divider);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
}

.toast-error {
  border-color: color-mix(in srgb, var(--error) 35%, var(--divider));
  background: color-mix(in srgb, var(--error) 8%, var(--panel));
}

.toast-warning {
  border-color: color-mix(in srgb, var(--warn-text, #d97706) 35%, var(--divider));
}

.toast-success {
  border-color: color-mix(in srgb, var(--accent) 35%, var(--divider));
}

.toast-tag {
  flex-shrink: 0;
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  padding: 2px 7px;
  border-radius: 999px;
  line-height: 1.3;
  margin-top: 1px;
}

.toast-error .toast-tag {
  color: var(--error);
  background: color-mix(in srgb, var(--error) 14%, transparent);
}

.toast-warning .toast-tag {
  color: var(--warn-text, #d97706);
  background: color-mix(in srgb, var(--warn-text, #d97706) 12%, transparent);
}

.toast-success .toast-tag {
  color: var(--accent-hover);
  background: var(--accent-soft);
}

.toast-info .toast-tag {
  color: var(--muted);
  background: var(--chip-bg);
}

.toast-msg {
  margin: 0;
  font-size: 13px;
  line-height: 1.45;
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-word;
}

.toast-error .toast-msg {
  color: var(--error);
}

.toast-close {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: var(--muted);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  padding: 0;
}

.toast-close:hover {
  background: rgba(0, 0, 0, 0.06);
  color: var(--text);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

.toast-move {
  transition: transform 0.2s ease;
}
</style>
