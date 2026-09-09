/**
 * 全书情节/性癖扫描：调后端 tropes_scan，写入全局仓
 * 代码路径: kk_novel_ai/src/services/tropeScan.js
 */
import { reactive } from "vue";
import { invoke, listen } from "./tauri.js";
import { appState, bumpTropeRevision } from "../stores/appState.js";
import { refreshTropeIndex } from "./tropeIndex.js";
import { t } from "../i18n/index.js";
import { formatCost, formatTokens, usageTotalTokens } from "../utils/usageFormat.js";

export const tropeScanState = reactive({
  running: false,
  batchRunning: false,
  batchIndex: 0,
  batchTotal: 0,
  batchTitle: "",
  cancelled: false,
  requestId: "",
  root: "",
  current: 0,
  total: 0,
  title: "",
  added: 0,
  updated: 0,
  skipped: 0,
  error: "",
  chunk: 0,
  chunks: 0,
  step: 0,
  steps: 0,
  pct: 0,
  tokens: 0,
  promptTokens: 0,
  completionTokens: 0,
  cacheHit: 0,
  cacheMiss: 0,
  usageSource: "",
  calls: 0,
  costCny: 0,
  model: "",
});

let unlistenStart = null;
let unlistenProgress = null;
let unlistenError = null;

export function scanUsageObject(state = tropeScanState) {
  return {
    prompt_tokens: Number(state.promptTokens) || 0,
    completion_tokens: Number(state.completionTokens) || 0,
    total_tokens: Number(state.tokens) || 0,
    prompt_cache_hit_tokens: Number(state.cacheHit) || 0,
    prompt_cache_miss_tokens: Number(state.cacheMiss) || 0,
    source: state.usageSource || "estimate",
  };
}

export function formatScanUsage(state = tropeScanState) {
  const u = scanUsageObject(state);
  const calls = Number(state.calls) || 0;
  if (!calls && !usageTotalTokens(u)) return "";
  const parts = [formatTokens(u), formatCost(state.costCny)];
  if (calls) parts.push(t("trope.scanCalls", { n: calls }));
  return parts.filter(Boolean).join(" · ");
}

function resetUsageFields() {
  tropeScanState.chunk = 0;
  tropeScanState.chunks = 0;
  tropeScanState.step = 0;
  tropeScanState.steps = 0;
  tropeScanState.pct = 0;
  tropeScanState.tokens = 0;
  tropeScanState.promptTokens = 0;
  tropeScanState.completionTokens = 0;
  tropeScanState.cacheHit = 0;
  tropeScanState.cacheMiss = 0;
  tropeScanState.usageSource = "";
  tropeScanState.calls = 0;
  tropeScanState.costCny = 0;
  tropeScanState.model = "";
}

function applyReportUsage(r) {
  if (!r) return;
  if (r.total_tokens != null) tropeScanState.tokens = Number(r.total_tokens) || 0;
  if (r.prompt_tokens != null) tropeScanState.promptTokens = Number(r.prompt_tokens) || 0;
  if (r.completion_tokens != null) {
    tropeScanState.completionTokens = Number(r.completion_tokens) || 0;
  }
  if (r.prompt_cache_hit_tokens != null) {
    tropeScanState.cacheHit = Number(r.prompt_cache_hit_tokens) || 0;
  }
  if (r.prompt_cache_miss_tokens != null) {
    tropeScanState.cacheMiss = Number(r.prompt_cache_miss_tokens) || 0;
  }
  if (r.usage_source != null) tropeScanState.usageSource = String(r.usage_source || "");
  if (r.calls != null) tropeScanState.calls = Number(r.calls) || 0;
  if (r.cost_cny != null) tropeScanState.costCny = Number(r.cost_cny) || 0;
  if (r.model_used != null) tropeScanState.model = String(r.model_used || "");
  if (tropeScanState.steps > 0) {
    tropeScanState.pct = Math.round((tropeScanState.step / tropeScanState.steps) * 100);
  } else if (!tropeScanState.cancelled) {
    tropeScanState.pct = 100;
  }
}

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
  if (payload.chunk != null) tropeScanState.chunk = Number(payload.chunk) || 0;
  if (payload.chunks != null) tropeScanState.chunks = Number(payload.chunks) || 0;
  if (payload.step != null) tropeScanState.step = Number(payload.step) || 0;
  if (payload.steps != null) tropeScanState.steps = Number(payload.steps) || 0;
  if (payload.pct != null) tropeScanState.pct = Number(payload.pct) || 0;
  if (payload.tokens != null) tropeScanState.tokens = Number(payload.tokens) || 0;
  if (payload.prompt_tokens != null) tropeScanState.promptTokens = Number(payload.prompt_tokens) || 0;
  if (payload.completion_tokens != null) {
    tropeScanState.completionTokens = Number(payload.completion_tokens) || 0;
  }
  if (payload.cache_hit != null) tropeScanState.cacheHit = Number(payload.cache_hit) || 0;
  if (payload.cache_miss != null) tropeScanState.cacheMiss = Number(payload.cache_miss) || 0;
  if (payload.usage_source != null) tropeScanState.usageSource = String(payload.usage_source || "");
  if (payload.calls != null) tropeScanState.calls = Number(payload.calls) || 0;
  if (payload.cost_cny != null) tropeScanState.costCny = Number(payload.cost_cny) || 0;
  if (payload.model_used != null) tropeScanState.model = String(payload.model_used || "");
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

let batchAbort = false;

export function isTropeScanBusy() {
  return !!(tropeScanState.running || tropeScanState.batchRunning);
}

export function cancelTropeScan() {
  batchAbort = true;
  const rid = tropeScanState.requestId;
  if (!rid) return;
  void invoke("llm_cancel", { requestId: rid }).catch(() => {});
}

/**
 * 按顺序扫描多本（未总结 / 已修改）。一本失败继续下一本；取消则停。
 * @param {{ path: string, title?: string }[]|string[]} entries
 */
export async function scanTropesQueue(entries) {
  const list = (entries || [])
    .map((e) => (typeof e === "string" ? { path: e, title: "" } : e))
    .filter((e) => e && String(e.path || "").trim());
  if (!list.length) {
    return { ok: 0, empty: 0, failed: 0, cancelled: false, total: 0 };
  }
  if (tropeScanState.running || tropeScanState.batchRunning) return null;

  batchAbort = false;
  tropeScanState.batchRunning = true;
  tropeScanState.batchTotal = list.length;
  tropeScanState.batchIndex = 0;
  tropeScanState.batchTitle = "";
  const result = { ok: 0, empty: 0, failed: 0, cancelled: false, total: list.length };
  try {
    for (let i = 0; i < list.length; i += 1) {
      if (batchAbort) {
        result.cancelled = true;
        break;
      }
      const item = list[i];
      const path = String(item.path).trim();
      tropeScanState.batchIndex = i + 1;
      tropeScanState.batchTitle = item.title || "";
      appState.statusMessage = t("project.tropeSummaryAllProgress", {
        current: i + 1,
        total: list.length,
        title: item.title || path,
      });
      try {
        const r = await scanTropesFromRoot(path);
        if (batchAbort || (r && r.cancelled)) {
          result.cancelled = true;
          break;
        }
        if (!r) {
          result.failed += 1;
          continue;
        }
        if (Number(r.scanned || 0) === 0) result.empty += 1;
        else result.ok += 1;
      } catch {
        result.failed += 1;
        if (batchAbort) {
          result.cancelled = true;
          break;
        }
      }
    }
  } finally {
    tropeScanState.batchRunning = false;
    tropeScanState.batchIndex = 0;
    tropeScanState.batchTotal = 0;
    tropeScanState.batchTitle = "";
  }
  return result;
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
  tropeScanState.root = projectRoot;
  tropeScanState.cancelled = false;
  tropeScanState.requestId = "";
  tropeScanState.current = 0;
  tropeScanState.total = 0;
  tropeScanState.title = "";
  tropeScanState.added = 0;
  tropeScanState.updated = 0;
  tropeScanState.skipped = 0;
  tropeScanState.error = "";
  resetUsageFields();
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
    applyReportUsage(r);
    try {
      await refreshTropeIndex();
    } catch {
      /* ignore */
    }
    bumpTropeRevision();
    const usage = formatScanUsage();
    if (tropeScanState.cancelled) {
      appState.statusMessage =
        t("trope.scanCancelled", {
          added: tropeScanState.added,
          updated: tropeScanState.updated,
        }) + (usage ? ` · ${usage}` : "");
    } else {
      appState.statusMessage =
        t("trope.scanDone", {
          added: tropeScanState.added,
          updated: tropeScanState.updated,
          skipped: tropeScanState.skipped,
        }) + (usage ? ` · ${usage}` : "");
    }
    return r;
  } catch (e) {
    tropeScanState.error = String(e.message || e);
    appState.statusMessage = t("trope.scanFailed", { msg: tropeScanState.error });
    throw e;
  } finally {
    await unbindEvents();
    tropeScanState.running = false;
    tropeScanState.root = "";
  }
}
