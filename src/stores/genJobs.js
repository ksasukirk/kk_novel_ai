/**
 * 多路并发生成任务表
 * 代码路径: kk_novel_ai/src/stores/genJobs.js
 */
import { computed, reactive } from "vue";
import { appState } from "./appState.js";
import { calcGenProgressPct, estimateTargetChars } from "../utils/genProgress.js";

/** 同时进行的写作任务上限 */
export const MAX_PARALLEL_GEN = 3;

/**
 * @typedef {object} GenJob
 * @property {string} id
 * @property {string} requestId
 * @property {"pending"|"streaming"|"done"|"error"|"cancelled"} status
 * @property {string} previewText
 * @property {string} previewRawText
 * @property {number} targetChars
 * @property {number} streamChars
 * @property {number} progressPct
 * @property {string} error
 * @property {string} label
 * @property {string} draftPlacement
 * @property {string} draftTask
 * @property {string} draftSelection
 * @property {string} draftInstruction
 * @property {string} draftPersistInstruction
 * @property {string} draftRewriteBlockKey
 * @property {string} draftAnchorBlockKey
 * @property {string} draftBranchMode
 * @property {string} draftBranchNodeId
 * @property {string} draftForkFromVariantId
 * @property {boolean} activateVariant
 * @property {string} lastModelUsed
 * @property {object|null} lastUsage
 * @property {string} lastLogId
 * @property {number} lastCostCny
 * @property {object|null} lastContextSources
 * @property {boolean} lastTruncated
 * @property {boolean} lastIncomplete
 * @property {boolean} accepted
 */

export const genJobState = reactive({
  /** @type {GenJob[]} */
  jobs: [],
  /** 等待绑定 llm-start.request_id 的 job.id 队列 */
  pendingBindIds: [],
});

export const activeGenJobs = computed(() =>
  genJobState.jobs.filter((j) => j.status === "pending" || j.status === "streaming")
);

export const visibleGenJobs = computed(() =>
  genJobState.jobs.filter(
    (j) =>
      j.draftPlacement === "editor" &&
      (j.status === "pending" ||
        j.status === "streaming" ||
        ((j.status === "done" || j.status === "error") && !j.accepted && (j.previewText || j.previewRawText)))
  )
);

export function activeJobCount() {
  return genJobState.jobs.filter((j) => j.status === "pending" || j.status === "streaming")
    .length;
}

export function canStartMoreJobs(n = 1) {
  return activeJobCount() + n <= MAX_PARALLEL_GEN;
}

export function findJobById(id) {
  return genJobState.jobs.find((j) => j.id === id) || null;
}

export function findJobByRequestId(rid) {
  if (!rid) return null;
  return genJobState.jobs.find((j) => j.requestId === rid) || null;
}

/** AI 面板正在跑的「同步总谱」，不要当静默后台吞掉 */
export function hasUserFacingStorySyncJob() {
  return genJobState.jobs.some((j) => {
    const t = j.draftTask || "";
    if (t !== "story_sync" && t !== "sync_story") return false;
    return j.status === "pending" || j.status === "streaming";
  });
}

export function jobsForAnchor(blockKey) {
  if (!blockKey) return [];
  return visibleGenJobs.value.filter((j) => j.draftAnchorBlockKey === blockKey);
}

export function trailingVisibleJobs() {
  return visibleGenJobs.value.filter((j) => !j.draftAnchorBlockKey);
}

function defaultTargetChars() {
  const mt = appState.settings && appState.settings.max_tokens;
  const tc = appState.settings && appState.settings.writing_target_chars;
  let target = estimateTargetChars(mt, tc);
  if (appState.draftTask === "same_slot_variant") {
    target = Math.max(200, Number(tc) || target || 1800);
  }
  return target;
}

/**
 * 从当前 appState.draft* 冻结一份 job 元数据并入队
 * @param {{ label?: string, activateVariant?: boolean }} [opts]
 */
export function createGenJob(opts = {}) {
  if (!canStartMoreJobs(1)) {
    throw new Error(`最多同时 ${MAX_PARALLEL_GEN} 路生成，请等一路完成或取消`);
  }
  const id = `job-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
  const targetChars = defaultTargetChars();
  /** @type {GenJob} */
  const job = reactive({
    id,
    requestId: "",
    status: "pending",
    previewText: "",
    previewRawText: "",
    targetChars,
    streamChars: 0,
    progressPct: 0,
    error: "",
    label: opts.label || appState.draftTask || "生成",
    draftPlacement: appState.draftPlacement || "editor",
    draftTask: appState.draftTask || "",
    draftSelection: appState.draftSelection || "",
    draftInstruction: appState.draftInstruction || "",
    draftPersistInstruction: appState.draftPersistInstruction || "",
    draftRewriteBlockKey: appState.draftRewriteBlockKey || "",
    draftAnchorBlockKey: appState.draftAnchorBlockKey || "",
    draftBranchMode: appState.draftBranchMode || "",
    draftBranchNodeId: appState.draftBranchNodeId || "",
    draftForkFromVariantId: appState.draftForkFromVariantId || "",
    activateVariant: opts.activateVariant !== false,
    lastModelUsed: "",
    lastUsage: null,
    lastLogId: "",
    lastCostCny: 0,
    lastContextSources: null,
    lastTruncated: false,
    lastIncomplete: false,
    accepted: false,
    draftActiveBeatId: appState.draftActiveBeatId || "",
    lastWrittenBlockKey: "",
  });
  genJobState.jobs.push(job);
  genJobState.pendingBindIds.push(id);
  syncGeneratingFromJobs();
  return job;
}

export function bindJobRequestId(requestId) {
  if (!requestId) return null;
  const existing = findJobByRequestId(requestId);
  if (existing) return existing;
  while (genJobState.pendingBindIds.length) {
    const id = genJobState.pendingBindIds.shift();
    const job = findJobById(id);
    if (!job) continue;
    if (job.requestId) continue;
    job.requestId = requestId;
    if (job.status === "pending") job.status = "streaming";
    appState.lastRequestId = requestId;
    return job;
  }
  return null;
}

export function appendJobDelta(job, delta) {
  if (!job || !delta) return;
  job.previewText += delta;
  job.previewRawText += delta;
  job.streamChars = [...(job.previewRawText || "")].length;
  job.progressPct = calcGenProgressPct(job.streamChars, job.targetChars, true, false);
  if (job.status === "pending") job.status = "streaming";
  syncAggregateProgress();
  mirrorJobToLegacyPreview(job);
}

export function finishJobOk(job, payload = {}) {
  if (!job) return;
  const raw =
    (typeof payload.raw_text === "string" && payload.raw_text) ||
    (typeof payload.text === "string" && payload.text) ||
    job.previewRawText ||
    job.previewText ||
    "";
  job.previewRawText = raw;
  job.previewText = raw;
  job.lastTruncated = !!payload.truncated;
  job.lastModelUsed = payload.model_used || job.lastModelUsed || "";
  job.lastUsage = payload.usage || null;
  job.lastLogId = payload.log_id || "";
  job.lastCostCny = typeof payload.cost_cny === "number" ? payload.cost_cny : 0;
  if (payload.context_sources) job.lastContextSources = payload.context_sources;
  job.streamChars = [...(job.previewRawText || "")].length;
  job.progressPct = 100;
  job.status = "done";
  syncGeneratingFromJobs();
  mirrorJobToLegacyPreview(job);
}

export function finishJobError(job, error, cancelled = false) {
  if (!job) return;
  job.error = String(error || "");
  job.status = cancelled ? "cancelled" : "error";
  job.progressPct = 0;
  syncGeneratingFromJobs();
}

export function removeGenJob(jobOrId) {
  const id = typeof jobOrId === "string" ? jobOrId : jobOrId?.id;
  if (!id) return;
  const idx = genJobState.jobs.findIndex((j) => j.id === id);
  if (idx >= 0) genJobState.jobs.splice(idx, 1);
  genJobState.pendingBindIds = genJobState.pendingBindIds.filter((x) => x !== id);
  syncGeneratingFromJobs();
}

export function clearJobPreview(job) {
  if (!job) return;
  job.previewText = "";
  job.previewRawText = "";
  job.accepted = true;
  removeGenJob(job);
}

/** 取消/失败时清掉尚未接受的编辑区 job */
export function discardJob(job) {
  if (!job) return;
  clearJobPreview(job);
}

export function syncGeneratingFromJobs() {
  const n = activeJobCount();
  appState.generating = n > 0;
  if (n === 0) {
    appState.genProgressPct = 0;
    appState.genStreamChars = 0;
  } else {
    syncAggregateProgress();
  }
}

function syncAggregateProgress() {
  const active = genJobState.jobs.filter(
    (j) => j.status === "pending" || j.status === "streaming"
  );
  if (!active.length) return;
  const chars = active.reduce((s, j) => s + (j.streamChars || 0), 0);
  const pct =
    active.reduce((s, j) => s + (j.progressPct || 0), 0) / Math.max(1, active.length);
  appState.genStreamChars = chars;
  appState.genProgressPct = Math.round(pct);
  appState.genTargetChars = active[0]?.targetChars || appState.genTargetChars;
  appState.statusMessage = `并发生成 ${active.length}/${MAX_PARALLEL_GEN} · ${chars} 字`;
}

/** 兼容旧 UI：把焦点 job 镜像到全局 preview* */
export function mirrorJobToLegacyPreview(job) {
  if (!job) return;
  appState.previewText = job.previewText || "";
  appState.previewRawText = job.previewRawText || "";
  appState.lastTruncated = !!job.lastTruncated;
  appState.lastIncomplete = !!job.lastIncomplete;
  appState.lastModelUsed = job.lastModelUsed || "";
  appState.lastUsage = job.lastUsage;
  appState.lastLogId = job.lastLogId || "";
  appState.lastCostCny = job.lastCostCny || 0;
  appState.lastContextSources = job.lastContextSources;
  if (job.requestId) appState.lastRequestId = job.requestId;
  // 草稿元数据也镜像，供尚未改完的路径读取
  appState.draftPlacement = job.draftPlacement || "";
  appState.draftTask = job.draftTask || "";
  appState.draftSelection = job.draftSelection || "";
  appState.draftInstruction = job.draftInstruction || "";
  appState.draftPersistInstruction = job.draftPersistInstruction || "";
  appState.draftRewriteBlockKey = job.draftRewriteBlockKey || "";
  appState.draftAnchorBlockKey = job.draftAnchorBlockKey || "";
  appState.draftBranchMode = job.draftBranchMode || "";
  appState.draftBranchNodeId = job.draftBranchNodeId || "";
  appState.draftForkFromVariantId = job.draftForkFromVariantId || "";
}

/** 卸掉某 job 后，把仍可见的另一路镜像回全局；没有则清空草稿元数据 */
export function refreshLegacyFromJobs() {
  const next = visibleGenJobs.value[0] || null;
  if (next) {
    mirrorJobToLegacyPreview(next);
    return;
  }
  appState.previewText = "";
  appState.previewRawText = "";
  appState.lastTruncated = false;
  appState.lastIncomplete = false;
  appState.draftPlacement = "";
  appState.draftTask = "";
  appState.draftSelection = "";
  appState.draftInstruction = "";
  appState.draftPersistInstruction = "";
  appState.draftRewriteBlockKey = "";
  appState.draftAnchorBlockKey = "";
  appState.draftBranchMode = "";
  appState.draftBranchNodeId = "";
  appState.draftForkFromVariantId = "";
  appState.lastContextSources = null;
}
