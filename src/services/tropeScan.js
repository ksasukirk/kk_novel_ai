/**
 * 全书情节/性癖扫描：调后端 tropes_scan，写入全局仓
 * 代码路径: kk_novel_ai/src/services/tropeScan.js
 */
import { reactive } from "vue";
import { invoke, listen } from "./tauri.js";
import { appState, bumpTropeRevision } from "../stores/appState.js";
import { refreshTropeIndex } from "./tropeIndex.js";
import { t } from "../i18n/index.js";

export const tropeScanState = reactive({
  running: false,
  cancelled: false,
  requestId: "",
  current: 0,
  total: 0,
  title: "",
  added: 0,
  updated: 0,
  skipped: 0,
  error: "",
});

let unlistenStart = null;
let unlistenProgress = null;
let unlistenError = null;

function applyProgress(payload) {
  if (!payload) return;
  if (payload.request_id && tropeScanState.requestId && payload.request_id !== tropeScanState.requestId) {
    return;
  }
  if (payload.request_id) tropeScanState.requestId = payload.request_id;
  if (payload.current != null) tropeScanState.current = Number(payload.current) || 0;
  if (payload.total != null) tropeScanState.total = Number(payload.total) || 0;
  if (payload.title != null) tropeScanState.title = String(payload.title || "");
  if (payload.added != null) tropeScanState.added = Number(payload.added) || 0;
  if (payload.updated != null) tropeScanState.updated = Number(payload.updated) || 0;
  if (payload.skipped != null) tropeScanState.skipped = Number(payload.skipped) || 0;
}

async function bindEvents() {
  await unbindEvents();
  unlistenStart = await listen("tropes-scan-start", (event) => {
    applyProgress(event && event.payload);
  });
  unlistenProgress = await listen("tropes-scan-progress", (event) => {
    applyProgress(event && event.payload);
  });
  unlistenError = await listen("tropes-scan-error", (event) => {
    const p = event && event.payload;
    if (p && p.error) tropeScanState.error = String(p.error);
  });
}

async function unbindEvents() {
  const fns = [unlistenStart, unlistenProgress, unlistenError];
  unlistenStart = null;
  unlistenProgress = null;
  unlistenError = null;
  for (const fn of fns) {
    try {
      if (typeof fn === "function") await fn();
    } catch {
      /* ignore */
    }
  }
}

export function cancelTropeScan() {
  const rid = tropeScanState.requestId;
  if (!rid) return;
  void invoke("llm_cancel", { requestId: rid }).catch(() => {});
}

/**
 * @param {string} root
 * @param {{ from?: number, to?: number }} [opts]
 */
export async function scanTropesFromRoot(root, opts = {}) {
  const projectRoot = String(root || "").trim();
  if (!projectRoot) throw new Error(t("trope.scanNeedProject"));
  if (tropeScanState.running) return null;

  tropeScanState.running = true;
  tropeScanState.cancelled = false;
  tropeScanState.requestId = "";
  tropeScanState.current = 0;
  tropeScanState.total = 0;
  tropeScanState.title = "";
  tropeScanState.added = 0;
  tropeScanState.updated = 0;
  tropeScanState.skipped = 0;
  tropeScanState.error = "";
  appState.statusMessage = t("trope.scanProgress", {
    current: 0,
    total: 0,
    added: 0,
    updated: 0,
  });

  await bindEvents();
  try {
    const r = await invoke("tropes_scan", {
      root: projectRoot,
      from: opts.from ?? 1,
      to: opts.to ?? 0,
    });
    tropeScanState.cancelled = !!(r && r.cancelled);
    tropeScanState.added = Array.isArray(r && r.added) ? r.added.length : tropeScanState.added;
    tropeScanState.updated = Array.isArray(r && r.updated) ? r.updated.length : tropeScanState.updated;
    tropeScanState.skipped = Number((r && r.skipped) || 0);
    try {
      await refreshTropeIndex();
    } catch {
      /* ignore */
    }
    bumpTropeRevision();
    if (tropeScanState.cancelled) {
      appState.statusMessage = t("trope.scanCancelled", {
        added: tropeScanState.added,
        updated: tropeScanState.updated,
      });
    } else {
      appState.statusMessage = t("trope.scanDone", {
        added: tropeScanState.added,
        updated: tropeScanState.updated,
        skipped: tropeScanState.skipped,
      });
    }
    return r;
  } catch (e) {
    tropeScanState.error = String(e.message || e);
    appState.statusMessage = t("trope.scanFailed", { msg: tropeScanState.error });
    throw e;
  } finally {
    await unbindEvents();
    tropeScanState.running = false;
  }
}
