/**
 * Tauri invoke / event 封装
 * 代码路径: kk_novel_ai/src/services/tauri.js
 */

export function getTauriInvoke() {
  const t = globalThis.__TAURI__;
  if (t && t.core && typeof t.core.invoke === "function") return t.core.invoke;
  if (t && typeof t.invoke === "function") return t.invoke;
  return null;
}

export async function invoke(cmd, args = {}) {
  const fn = getTauriInvoke();
  if (!fn) throw new Error("未检测到 Tauri 环境");
  return await fn(cmd, args);
}

export async function listen(event, handler) {
  const t = globalThis.__TAURI__;
  if (!t || !t.event || typeof t.event.listen !== "function") {
    throw new Error("未检测到 Tauri event API");
  }
  return await t.event.listen(event, handler);
}
