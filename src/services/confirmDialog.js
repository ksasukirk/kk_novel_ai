/**
 * 全局自定义确认/提示弹窗（替代 window.confirm / alert）
 * 代码路径: kk_novel_ai/src/services/confirmDialog.js
 */
import { reactive } from "vue";
import { appState } from "../stores/appState.js";

export const confirmState = reactive({
  open: false,
  mode: "confirm", // confirm | alert
  title: "确认",
  message: "",
  confirmText: "确定",
  cancelText: "取消",
  danger: false,
});

let pendingResolve = null;

function closeWith(result) {
  const resolve = pendingResolve;
  pendingResolve = null;
  confirmState.open = false;
  if (resolve) resolve(result);
}

/** 设置「删除不需确认」是否开启（默认 true） */
export function isSkipDeleteConfirm() {
  const s = appState.settings;
  if (!s) return true;
  return s.skip_delete_confirm !== false;
}

/**
 * @param {string} message
 * @param {{ title?: string, confirmText?: string, cancelText?: string, danger?: boolean }} [opts]
 * @returns {Promise<boolean>}
 */
export function appConfirm(message, opts = {}) {
  if (pendingResolve) {
    pendingResolve(false);
    pendingResolve = null;
  }
  return new Promise((resolve) => {
    pendingResolve = resolve;
    confirmState.open = true;
    confirmState.mode = "confirm";
    confirmState.title = opts.title || "确认";
    confirmState.message = String(message || "");
    confirmState.confirmText = opts.confirmText || "确定";
    confirmState.cancelText = opts.cancelText || "取消";
    confirmState.danger = !!opts.danger;
  });
}

/**
 * 删除类操作统一入口：skip_delete_confirm 开启时直接通过
 * @param {string} message
 * @param {{ title?: string, confirmText?: string, cancelText?: string, danger?: boolean }} [opts]
 * @returns {Promise<boolean>}
 */
export function appConfirmDelete(message, opts = {}) {
  if (isSkipDeleteConfirm()) return Promise.resolve(true);
  return appConfirm(message, {
    title: opts.title || "确认删除",
    confirmText: opts.confirmText || "删除",
    cancelText: opts.cancelText || "取消",
    danger: opts.danger !== false,
  });
}

/**
 * @param {string} message
 * @param {{ title?: string, confirmText?: string }} [opts]
 * @returns {Promise<void>}
 */
export function appAlert(message, opts = {}) {
  if (pendingResolve) {
    pendingResolve(false);
    pendingResolve = null;
  }
  return new Promise((resolve) => {
    pendingResolve = () => resolve();
    confirmState.open = true;
    confirmState.mode = "alert";
    confirmState.title = opts.title || "提示";
    confirmState.message = String(message || "");
    confirmState.confirmText = opts.confirmText || "知道了";
    confirmState.cancelText = "";
    confirmState.danger = false;
  });
}

export function resolveAppConfirm(ok) {
  if (confirmState.mode === "alert") {
    closeWith(true);
    return;
  }
  closeWith(!!ok);
}

export function cancelAppConfirm() {
  closeWith(confirmState.mode === "alert" ? true : false);
}
