/**
 * 全书情节/喜好扫描 + 按正文重建章纲/生成块
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
  /** tropes | structure */
  phase: "",
  root: "",
  current: 0,
  total: 0,
  title: "",
  added: 0,
  updated: 0,
  skipped: 0,
  rebuilt: 0,
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

let unlistenFns = [];

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
  tropeScanState.rebuilt = 0;
}

function applyReportUsage(r, { accumulate = false } = {}) {
  if (!r) return;
  const add = (cur, next) => (accumulate ? (Number(cur) || 0) + (Number(next) || 0) : Number(next) || 0);
  if (r.total_tokens != null) tropeScanState.tokens = add(tropeScanState.tokens, r.total_tokens);
  if (r.prompt_tokens != null) {
    tropeScanState.promptTokens = add(tropeScanState.promptTokens, r.prompt_tokens);
  }
  if (r.completion_tokens != null) {
    tropeScanState.completionTokens = add(tropeScanState.completionTokens, r.completion_tokens);
  }
  if (r.prompt_cache_hit_tokens != null) {
    tropeScanState.cacheHit = add(tropeScanState.cacheHit, r.prompt_cache_hit_tokens);
  }
  if (r.prompt_cache_miss_tokens != null) {
    tropeScanState.cacheMiss = add(tropeScanState.cacheMiss, r.prompt_cache_miss_tokens);
  }
  if (r.usage_source != null) tropeScanState.usageSource = String(r.usage_source || "");
  if (r.calls != null) tropeScanState.calls = add(tropeScanState.calls, r.calls);
  if (r.cost_cny != null) tropeScanState.costCny = add(tropeScanState.costCny, r.cost_cny);
  if (r.model_used != null) tropeScanState.model = String(r.model_used || "");
  if (r.rebuilt != null) tropeScanState.rebuilt = Number(r.rebuilt) || 0;
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
  if (payload.phase != null) tropeScanState.phase = String(payload.phase || "");
  if (payload.current != null) tropeScanState.current = Number(payload.current) || 0;
  if (payload.total != null) tropeScanState.total = Number(payload.total) || 0;
  if (payload.title != null) tropeScanState.title = String(payload.title || "");
  if (payload.added != null) tropeScanState.added = Number(payload.added) || 0;
  if (payload.updated != null) tropeScanState.updated = Number(payload.updated) || 0;
  if (payload.skipped != null) tropeScanState.skipped = Number(payload.skipped) || 0;
  if (payload.rebuilt != null) tropeScanState.rebuilt = Number(payload.rebuilt) || 0;
  if (payload.chunk != null) tropeScanState.chunk = Number(payload.chunk) || 0;
  if (payload.chunks != null) tropeScanState.chunks = Number(payload.chunks) || 0;
  if (payload.step != null) tropeScanState.step = Number(payload.step) || 0;
  if (payload.steps != null) tropeScanState.steps = Number(payload.steps) || 0;
  if (payload.pct != null) tropeScanState.pct = Number(payload.pct) || 0;
  // structure 进度里的 usage 是阶段内累计；勿覆盖 phase1 账本，结束时再 accumulate
  if (tropeScanState.phase !== "structure") {
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

  if (tropeScanState.phase === "structure") {
    appState.statusMessage = t("trope.structureProgress", {
      current: tropeScanState.current,
      total: tropeScanState.total,
      title: tropeScanState.title || "",
      rebuilt: tropeScanState.rebuilt,
    });
  }
}

async function bindEvents() {
  await unbindEvents();
  const pairs = [
    ["tropes-scan-start", applyProgress],
    ["tropes-scan-progress", applyProgress],
    [
      "tropes-scan-error",
      (event) => {
        const p = event && event.payload;
        if (p && p.error) tropeScanState.error = String(p.error);
      },
    ],
    ["rebuild-structure-start", applyProgress],
    ["rebuild-structure-progress", applyProgress],
    [
      "rebuild-structure-error",
      (event) => {
        const p = event && event.payload;
        if (p && p.error) tropeScanState.error = String(p.error);
      },
    ],
  ];
  for (const [name, handler] of pairs) {
    unlistenFns.push(await listen(name, handler));
  }
}

async function unbindEvents() {
  const fns = unlistenFns;
  unlistenFns = [];
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

async function refreshOpenProjectIfMatch(projectRoot) {
  if (!projectRoot || appState.projectRoot !== projectRoot) return;
  try {
    const project = await import("./projectClient.js");
    await project.getProject(projectRoot);
    if (appState.chapterId) {
      await project.loadChapter(appState.chapterId);
    }
  } catch {
    /* 刷新失败不挡总结结果 */
  }
}

/**
 * phase2：按正文重建章纲 + 单生成块
 * @param {string} projectRoot
 * @param {{ from?: number, to?: number }} [opts]
 */
async function rebuildStructurePhase(projectRoot, opts = {}) {
  if (batchAbort || tropeScanState.cancelled) {
    tropeScanState.cancelled = true;
    return { cancelled: true };
  }
  tropeScanState.phase = "structure";
  tropeScanState.requestId = "";
  tropeScanState.current = 0;
  tropeScanState.total = 0;
  tropeScanState.title = "";
  tropeScanState.rebuilt = 0;
  appState.statusMessage = t("trope.structureStarting");
  const r = await invoke("rebuild_structure_from_prose", {
    root: projectRoot,
    from: opts.from ?? 1,
    to: opts.to ?? 0,
  });
  tropeScanState.cancelled = !!(r && r.cancelled) || batchAbort;
  applyReportUsage(r, { accumulate: true });
  return r;
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
        if (Number(r.scanned || 0) === 0 && Number(r.rebuilt || 0) === 0) result.empty += 1;
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
 * @param {{ from?: number, to?: number, skipStructure?: boolean }} [opts]
 */
export async function scanTropesFromRoot(root, opts = {}) {
  const projectRoot = String(root || "").trim();
  if (!projectRoot) throw new Error(t("trope.scanNeedProject"));
  if (tropeScanState.running) return null;

  tropeScanState.running = true;
  tropeScanState.root = projectRoot;
  tropeScanState.cancelled = false;
  tropeScanState.requestId = "";
  tropeScanState.phase = "tropes";
  tropeScanState.current = 0;
  tropeScanState.total = 0;
  tropeScanState.title = "";
  tropeScanState.added = 0;
  tropeScanState.updated = 0;
  tropeScanState.skipped = 0;
  tropeScanState.rebuilt = 0;
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
    tropeScanState.cancelled = !!(r && r.cancelled) || batchAbort;
    tropeScanState.added = Array.isArray(r && r.added) ? r.added.length : tropeScanState.added;
    tropeScanState.updated = Array.isArray(r && r.updated) ? r.updated.length : tropeScanState.updated;
    tropeScanState.skipped = Number((r && r.skipped) || 0);
    applyReportUsage(r);
    if (!tropeScanState.cancelled) {
      try {
        const { ensureTropeLibrary } = await import("./projectClient.js");
        await ensureTropeLibrary({ maintain: true });
      } catch {
        /* 维护失败不挡扫描结果 */
      }
    }
    try {
      await refreshTropeIndex();
    } catch {
      /* ignore */
    }

    let structure = null;
    if (!tropeScanState.cancelled && !opts.skipStructure) {
      try {
        structure = await rebuildStructurePhase(projectRoot, opts);
        if (structure) {
          r.rebuilt = Number(structure.rebuilt || 0);
          r.structure_skipped = Number(structure.skipped || 0);
          r.structure_failed = Array.isArray(structure.failed) ? structure.failed.length : 0;
          if (structure.cancelled) r.cancelled = true;
        }
      } catch (e) {
        tropeScanState.error = String(e.message || e);
        appState.statusMessage = t("trope.structureFailed", { msg: tropeScanState.error });
        // 情节扫描已成功：不因结构重建失败整单失败
        r.structure_error = tropeScanState.error;
      }
    }

    bumpTropeRevision();
    await refreshOpenProjectIfMatch(projectRoot);

    const usage = formatScanUsage();
    if (tropeScanState.cancelled) {
      appState.statusMessage =
        t("trope.scanCancelled", {
          added: tropeScanState.added,
          updated: tropeScanState.updated,
        }) + (usage ? ` · ${usage}` : "");
    } else {
      appState.statusMessage =
        t("trope.scanDoneWithStructure", {
          added: tropeScanState.added,
          updated: tropeScanState.updated,
          skipped: tropeScanState.skipped,
          rebuilt: tropeScanState.rebuilt,
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
    tropeScanState.phase = "";
  }
}
