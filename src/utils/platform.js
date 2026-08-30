/**
 * 平台探测：移动端布局 / Tauri Android 能力分流
 * 代码路径: kk_novel_ai/src/utils/platform.js
 */

const MOBILE_MQ = "(max-width: 720px)";

/** 视口是否按手机布局（可响应式变化） */
export function isMobileViewport() {
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia(MOBILE_MQ).matches;
}

/** 是否运行在 Tauri Android（或其它移动壳） */
export function isTauriMobile() {
  try {
    const t = globalThis.__TAURI__;
    if (!t) return false;
    const os = t.os || t.core?.os;
    // 兼容不同 API 暴露方式
    if (typeof navigator !== "undefined") {
      const ua = navigator.userAgent || "";
      if (/Android/i.test(ua) && t) return true;
    }
    if (os && typeof os.type === "function") {
      // 同步不可用时退回 UA
    }
  } catch {
    /* ignore */
  }
  if (typeof navigator !== "undefined") {
    return /Android/i.test(navigator.userAgent || "");
  }
  return false;
}

/** 手机端体验：窄屏或真机 Android */
export function isMobileUx() {
  return isTauriMobile() || isMobileViewport();
}

/**
 * 订阅窄屏断点变化
 * @param {(mobile: boolean) => void} cb
 * @returns {() => void} unsubscribe
 */
export function watchMobileViewport(cb) {
  if (typeof window === "undefined" || !window.matchMedia) {
    return () => {};
  }
  const mql = window.matchMedia(MOBILE_MQ);
  const handler = () => cb(mql.matches);
  if (typeof mql.addEventListener === "function") {
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }
  mql.addListener(handler);
  return () => mql.removeListener(handler);
}
