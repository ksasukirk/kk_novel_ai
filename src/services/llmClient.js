/**
 * LLM / 写作客户端（支持多路并发生成）
 * 代码路径: kk_novel_ai/src/services/llmClient.js
 */
import { invoke } from "./tauri.js";
import { appState } from "../stores/appState.js";
import { getActiveRequestId } from "./guiBridge.js";
import { looksIncomplete } from "../utils/previewText.js";
import {
  deepseekGeneratingStatusSuffix,
  notifyDeepseekPeakIfNeeded,
} from "../utils/deepseekPricing.js";
import { toastWarning } from "./toast.js";
import { isCancelledMsg, t } from "../i18n/index.js";
import { aiPanelForm } from "../stores/aiPanelState.js";
import {
  activeJobCount,
  appendJobDelta,
  bindJobRequestId,
  createGenJob,
  discardJob,
  findJobById,
  findJobByRequestId,
  finishJobError,
  finishJobOk,
  genJobState,
  MAX_PARALLEL_GEN,
  mirrorJobToLegacyPreview,
  refreshLegacyFromJobs,
  syncGeneratingFromJobs,
} from "../stores/genJobs.js";

export async function refreshHealth() {
  try {
    const r = await invoke("llm_health");
    appState.llmOnline = !!(r && r.ok);
    appState.statusMessage = appState.llmOnline ? t("header.llmOnline") : t("header.llmOffline");
    return r;
  } catch (e) {
    appState.llmOnline = false;
    appState.statusMessage = String(e.message || e);
    throw e;
  }
}

export async function listModels() {
  return await invoke("llm_list_models");
}

export async function loadSettings() {
  const r = await invoke("settings_get");
  appState.settings = r.settings;
  appState.deepseekPeakNow = !!r.deepseek_peak_now;
  appState.deepseekPeakNotice = r.deepseek_peak_notice || "";
  appState.llmModel = (r.settings && r.settings.model) || "";
  const { applyEditorTypography } = await import("../utils/editorTypography.js");
  applyEditorTypography(r.settings);
  return r.settings;
}

export async function saveSettings(settings) {
  const r = await invoke("settings_save", { settings });
  appState.settings = r.settings;
  appState.llmModel = (r.settings && r.settings.model) || "";
  const { applyEditorTypography } = await import("../utils/editorTypography.js");
  applyEditorTypography(r.settings);
  return r.settings;
}

function applyResultToJob(job, r) {
  const apiText = typeof r?.text === "string" ? r.text : "";
  const rawText =
    typeof r?.raw_text === "string" && r.raw_text
      ? r.raw_text
      : job.previewRawText || apiText || job.previewText || "";
  const display = (rawText || apiText).trim() ? rawText || apiText : apiText;
  finishJobOk(job, {
    raw_text: display,
    text: display,
    truncated: !!r?.truncated,
    model_used: r?.model_used || "",
    usage: r?.usage || null,
    log_id: r?.log_id || "",
    cost_cny: typeof r?.cost_cny === "number" ? r.cost_cny : 0,
    context_sources: r?.context_sources || null,
  });
  job.lastIncomplete = looksIncomplete(display);
  mirrorJobToLegacyPreview(job);
}

/**
 * 运行写作任务；可与其它 job 并行（上限 MAX_PARALLEL_GEN）
 * @param {object} request
 * @param {{ label?: string, activateVariant?: boolean, job?: object }} [opts]
 */
export async function runWriting(request, opts = {}) {
  const job =
    opts.job ||
    createGenJob({
      label: opts.label,
      activateVariant: opts.activateVariant,
    });
  if (opts.activateVariant === false) job.activateVariant = false;
  if (opts.activateVariant === true) job.activateVariant = true;
  mirrorJobToLegacyPreview(job);
  const n = activeJobCount();
  const peakSuffix = deepseekGeneratingStatusSuffix(appState.settings || {});
  const peakNotice = notifyDeepseekPeakIfNeeded(appState.settings || {}, {
    toastFn: toastWarning,
  });
  appState.statusMessage =
    n > 1
      ? t("status.parallel", { n, max: MAX_PARALLEL_GEN, suffix: peakSuffix })
      : peakNotice
        ? t("status.generatingPeak", { suffix: peakSuffix })
        : t("status.generating");

  let result;
  try {
    const payload = { ...(request || {}) };
    if (payload.selected_trope_ids === undefined) {
      payload.selected_trope_ids = Array.isArray(aiPanelForm.selectedTropeIds)
        ? [...aiPanelForm.selectedTropeIds]
        : [];
    }
    result = await invoke("writing_run", { request: payload });
    const rid = result && result.request_id ? String(result.request_id) : "";
    if (rid) {
      if (!job.requestId) {
        job.requestId = rid;
        bindJobRequestId(rid);
      }
      appState.lastRequestId = rid;
    }
    if (job.status !== "cancelled") {
      applyResultToJob(job, result || {});
    }
  } catch (e) {
    const msg = String(e.message || e);
    const cancelled = isCancelledMsg(msg);
    if (job.status !== "cancelled" && job.status !== "error") {
      finishJobError(job, msg, cancelled);
    }
    appState.statusMessage = cancelled ? t("status.cancelled") : msg;
    if (job.draftPlacement === "editor") {
      discardJob(job);
      refreshLegacyFromJobs();
    }
    syncGeneratingFromJobs();
    throw e;
  } finally {
    syncGeneratingFromJobs();
  }

  if (
    job.draftPlacement === "editor" &&
    job.status === "done" &&
    !job.accepted &&
    !job.skipAutoAccept
  ) {
    const { autoAcceptJobIfNeeded } = await import("./draftAccept.js");
    await autoAcceptJobIfNeeded(job);
  }
  return result;
}

export async function cancelGeneration(requestId) {
  const rid =
    requestId ||
    getActiveRequestId() ||
    appState.lastRequestId ||
    "";
  if (!rid) {
    appState.statusMessage = t("status.cannotCancel");
    return { ok: false, request_id: "" };
  }
  appState.lastRequestId = rid;
  try {
    const r = await invoke("llm_cancel", { requestId: rid });
    if (r && r.ok) {
      appState.statusMessage = t("status.cancelling");
    } else {
      appState.statusMessage = t("status.cancelIneffective");
    }
    return r || { ok: false, request_id: rid };
  } catch (e) {
    appState.statusMessage = t("status.cancelFailed", { msg: e.message || e });
    throw e;
  }
}

export async function cancelJob(jobOrId) {
  const job = typeof jobOrId === "string" ? findJobById(jobOrId) : jobOrId;
  if (!job) return { ok: false };
  if (job.requestId) {
    try {
      await cancelGeneration(job.requestId);
    } catch {
      /* ignore */
    }
  }
  finishJobError(job, t("status.cancelledShort"), true);
  discardJob(job);
  refreshLegacyFromJobs();
  syncGeneratingFromJobs();
  appState.statusMessage = t("status.cancelled");
  return { ok: true, request_id: job.requestId || "" };
}

export async function cancelAllGenerations() {
  const jobs = genJobState.jobs.filter(
    (j) => j.status === "pending" || j.status === "streaming"
  );
  for (const j of [...jobs]) {
    await cancelJob(j);
  }
  return { ok: true, count: jobs.length };
}

/** 供桥接：把 chunk 记到对应 job */
export function noteChunkForRequest(requestId, delta) {
  let job = findJobByRequestId(requestId);
  if (!job && requestId) {
    job = bindJobRequestId(requestId);
  }
  if (!job) return null;
  appendJobDelta(job, delta || "");
  return job;
}

export {
  findJobByRequestId,
  bindJobRequestId,
  MAX_PARALLEL_GEN,
  activeJobCount,
};
