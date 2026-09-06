/**
 * Tauri invoke / event 封装
 * 代码路径: kk_novel_ai/src/services/tauri.js
 */

import { t } from "../i18n/index.js";

export function getTauriInvoke() {
  const api = globalThis.__TAURI__;
  if (api && api.core && typeof api.core.invoke === "function") return api.core.invoke;
  if (api && typeof api.invoke === "function") return api.invoke;
  return null;
}

export async function invoke(cmd, args = {}) {
  const fn = getTauriInvoke();
  if (!fn) throw new Error(t("common.noTauri"));
  return await fn(cmd, args);
}

export async function listen(event, handler) {
  const api = globalThis.__TAURI__;
  if (!api || !api.event || typeof api.event.listen !== "function") {
    throw new Error(t("common.noTauriEvent"));
  }
  return await api.event.listen(event, handler);
}
