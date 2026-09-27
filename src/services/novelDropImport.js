/**
 * 窗口拖放 TXT/MD → 自动导入为写作作品
 * 代码路径: kk_novel_ai/src/services/novelDropImport.js
 */
import { reactive } from "vue";
import { filterNovelImportPaths, importNovelFiles, importAutoSummaryEnabled } from "./novelImportFlow.js";
import { t } from "../i18n/index.js";
import { isMobileUx, isTauriMobile } from "../utils/platform.js";

export const novelDropState = reactive({
  hovering: false,
  busy: false,
  lastError: "",
});

let unlisten = null;

function getDragDropHost() {
  const api = globalThis.__TAURI__;
  if (!api) return null;
  try {
    if (api.webview && typeof api.webview.getCurrentWebview === "function") {
      const w = api.webview.getCurrentWebview();
      if (w && typeof w.onDragDropEvent === "function") return w;
    }
  } catch {
    /* ignore */
  }
  try {
    if (api.webviewWindow && typeof api.webviewWindow.getCurrentWebviewWindow === "function") {
      const w = api.webviewWindow.getCurrentWebviewWindow();
      if (w && typeof w.onDragDropEvent === "function") return w;
    }
  } catch {
    /* ignore */
  }
  return null;
}

function eventType(payload) {
  if (!payload) return "";
  if (typeof payload.type === "string") return payload.type.toLowerCase();
  if (payload.type && typeof payload.type === "object") {
    if ("enter" in payload.type) return "enter";
    if ("over" in payload.type) return "over";
    if ("leave" in payload.type) return "leave";
    if ("drop" in payload.type) return "drop";
    if ("cancel" in payload.type) return "cancel";
  }
  if (Array.isArray(payload.paths)) return "drop";
  return "";
}

function eventPaths(payload) {
  if (!payload) return [];
  if (Array.isArray(payload.paths)) return payload.paths;
  const nested = payload.type && typeof payload.type === "object" ? payload.type : null;
  if (nested) {
    for (const key of ["drop", "enter"]) {
      const block = nested[key];
      if (block && Array.isArray(block.paths)) return block.paths;
    }
  }
  return [];
}

async function handleDropPaths(paths) {
  const files = filterNovelImportPaths(paths);
  if (!files.length) {
    novelDropState.lastError = t("project.importNovelDropNone");
    return;
  }
  if (novelDropState.busy) return;
  novelDropState.busy = true;
  novelDropState.lastError = "";
  try {
    const r = await importNovelFiles(files, {
      autoSummary: importAutoSummaryEnabled(),
      openLast: true,
    });
    if (r.failed.length && !r.imported.length) {
      novelDropState.lastError = r.failed[0].msg || t("project.importNovelDropFail");
    } else if (r.failed.length) {
      novelDropState.lastError = t("project.importNovelDropPartial", {
        ok: r.imported.length,
        fail: r.failed.length,
      });
    }
  } catch (e) {
    novelDropState.lastError = e && e.message ? String(e.message) : String(e);
  } finally {
    novelDropState.busy = false;
    novelDropState.hovering = false;
  }
}

/**
 * @param {(msg: string) => void} [onError]
 */
export async function startNovelDropImport(onError) {
  if (isMobileUx() || isTauriMobile()) return () => {};
  if (unlisten) return unlisten;
  const host = getDragDropHost();
  if (!host) return () => {};

  unlisten = await host.onDragDropEvent(async (event) => {
    const payload = event && event.payload != null ? event.payload : event;
    const kind = eventType(payload);
    if (kind === "enter" || kind === "over") {
      novelDropState.hovering = true;
      return;
    }
    if (kind === "leave" || kind === "cancel") {
      novelDropState.hovering = false;
      return;
    }
    if (kind === "drop") {
      novelDropState.hovering = false;
      const paths = eventPaths(payload);
      await handleDropPaths(paths);
      if (novelDropState.lastError && typeof onError === "function") {
        onError(novelDropState.lastError);
      }
    }
  });

  return () => {
    stopNovelDropImport();
  };
}

export function stopNovelDropImport() {
  const fn = unlisten;
  unlisten = null;
  novelDropState.hovering = false;
  if (typeof fn === "function") {
    try {
      fn();
    } catch {
      /* ignore */
    }
  }
}
