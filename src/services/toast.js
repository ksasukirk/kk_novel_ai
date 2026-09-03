/**
 * 全局浮窗提示队列（窗口顶部居中）
 * 代码路径: kk_novel_ai/src/services/toast.js
 */
import { reactive, ref, watch } from "vue";

const MAX_TOASTS = 8;
const DEFAULT_DURATION = {
  error: 6500,
  warning: 5500,
  info: 4500,
  success: 3500,
};

let idSeq = 0;
const timers = new Map();

export const toastState = reactive({
  items: [],
});

function scheduleDismiss(id, duration) {
  const prev = timers.get(id);
  if (prev) clearTimeout(prev);
  const t = setTimeout(() => dismissToast(id), duration);
  timers.set(id, t);
}

/**
 * @param {string} message
 * @param {{ type?: 'error'|'warning'|'info'|'success', duration?: number }} [opts]
 */
export function pushToast(message, opts = {}) {
  const text = String(message || "").trim();
  if (!text) return null;
  const type = opts.type || "info";
  const id = ++idSeq;
  const item = {
    id,
    message: text,
    type,
    createdAt: Date.now(),
  };
  toastState.items.unshift(item);
  while (toastState.items.length > MAX_TOASTS) {
    const dropped = toastState.items.pop();
    if (dropped) {
      const tm = timers.get(dropped.id);
      if (tm) clearTimeout(tm);
      timers.delete(dropped.id);
    }
  }
  scheduleDismiss(id, opts.duration ?? DEFAULT_DURATION[type] ?? 4500);
  return id;
}

export function dismissToast(id) {
  const tm = timers.get(id);
  if (tm) clearTimeout(tm);
  timers.delete(id);
  const i = toastState.items.findIndex((x) => x.id === id);
  if (i >= 0) toastState.items.splice(i, 1);
}

export function toastError(message) {
  return pushToast(message, { type: "error" });
}

export function toastWarning(message) {
  return pushToast(message, { type: "warning" });
}

export function toastInfo(message) {
  return pushToast(message, { type: "info" });
}

export function toastSuccess(message) {
  return pushToast(message, { type: "success" });
}

/** 多行文本拆成队列逐条显示 */
export function toastErrorLines(raw) {
  const text = String(raw || "").trim();
  if (!text) return;
  const lines = text.split("\n").map((l) => l.trim()).filter(Boolean);
  if (!lines.length) return;
  for (const line of lines) toastError(line);
}

/**
 * 页面级 error ref：赋值后自动推入浮窗并清空，替代内联 `<pre class="error">`
 * @returns {import('vue').Ref<string>}
 */
export function useToastError() {
  const error = ref("");
  watch(error, (v) => {
    if (!v) return;
    toastErrorLines(v);
    error.value = "";
  });
  return error;
}
