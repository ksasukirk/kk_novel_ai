/**
 * 全局自定义确认/提示弹窗（替代 window.confirm / alert）
 * 代码路径: kk_novel_ai/src/services/confirmDialog.js
 */
import { reactive } from "vue";
import { appState } from "../stores/appState.js";
import { t } from "../i18n/index.js";

export const confirmState = reactive({
  open: false,
  mode: "confirm", // confirm | alert
  title: t("common.confirm"),
  message: "",
  confirmText: t("common.ok"),
  cancelText: t("common.cancel"),
  extraText: "",
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
 * @param {{ title?: string, confirmText?: string, cancelText?: string, extraText?: string, danger?: boolean }} [opts]
 * @returns {Promise<boolean | 'extra'>}
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
    confirmState.title = opts.title || t("common.confirm");
    confirmState.message = String(message || "");
    confirmState.confirmText = opts.confirmText || t("common.ok");
    confirmState.cancelText = opts.cancelText || t("common.cancel");
    confirmState.extraText = opts.extraText || "";
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
    title: opts.title || t("common.confirmDelete"),
    confirmText: opts.confirmText || t("common.delete"),
    cancelText: opts.cancelText || t("common.cancel"),
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
    confirmState.title = opts.title || t("common.hint");
    confirmState.message = String(message || "");
    confirmState.confirmText = opts.confirmText || t("common.gotIt");
    confirmState.cancelText = "";
    confirmState.extraText = "";
    confirmState.danger = false;
  });
}

export function resolveAppConfirm(ok) {
  if (confirmState.mode === "alert") {
    closeWith(true);
    return;
  }
  if (ok === "extra") {
    closeWith("extra");
    return;
  }
  closeWith(!!ok);
}

export function cancelAppConfirm() {
  closeWith(confirmState.mode === "alert" ? true : false);
}
