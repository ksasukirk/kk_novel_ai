/**
 * 启动检查更新：自定义弹窗确认后下载并启动新版本
 * 代码路径: kk_novel_ai/src/services/updateFlow.js
 */
import { reactive } from "vue";
import {
  checkAppUpdate,
  downloadAppUpdate,
  launchDownloadedUpdate,
} from "./appUpdate.js";
import { appState } from "../stores/appState.js";
import { isTauriMobile } from "../utils/platform.js";

export const updateFlow = reactive({
  open: false,
  phase: "prompt",
  info: null,
  received: 0,
  total: 0,
  startedAt: 0,
  error: "",
});

let scheduled = false;

export function formatUpdateMb(bytes) {
  const n = Number(bytes) || 0;
  return (n / (1024 * 1024)).toFixed(2);
}

/** 从开始下载起的平均速度，单位 MB/s */
export function formatUpdateSpeedMbs(received, startedAtMs) {
  const bytes = Number(received) || 0;
  const start = Number(startedAtMs) || 0;
  const elapsedSec = start > 0 ? (Date.now() - start) / 1000 : 0;
  if (bytes <= 0 || elapsedSec < 0.05) return "0.00";
  return (bytes / (1024 * 1024) / elapsedSec).toFixed(2);
}

export function updateFlowPct() {
  const t = Number(updateFlow.total) || 0;
  const r = Number(updateFlow.received) || 0;
  if (t <= 0) return 0;
  return Math.min(100, Math.round((r / t) * 100));
}

export function scheduleStartupUpdateCheck() {
  if (scheduled) return;
  scheduled = true;
  window.setTimeout(() => {
    void runStartupUpdateCheck();
  }, 2500);
}

async function runStartupUpdateCheck() {
  if (isTauriMobile()) return;
  try {
    const r = await checkAppUpdate();
    if (!r || !r.has_update || !r.latest) return;
    if (!r.download_url && !r.api_download_url) {
      appState.statusMessage = `有新版本 ${r.latest}，可到设置打开 GitHub Release`;
      return;
    }
    updateFlow.info = r;
    updateFlow.phase = "prompt";
    updateFlow.error = "";
    updateFlow.received = 0;
    updateFlow.total = 0;
    updateFlow.startedAt = 0;
    updateFlow.open = true;
  } catch {
    /* 启动检查失败不打扰 */
  }
}

export function dismissUpdateFlow() {
  if (updateFlow.phase === "downloading" || updateFlow.phase === "launching") return;
  updateFlow.open = false;
  updateFlow.phase = "prompt";
  if (updateFlow.info && updateFlow.info.latest) {
    appState.statusMessage = `有新版本 ${updateFlow.info.latest}，可到设置下载`;
  }
}

export async function confirmUpdateDownloadAndLaunch() {
  const info = updateFlow.info;
  if (!info) return;
  updateFlow.phase = "downloading";
  updateFlow.error = "";
  updateFlow.received = 0;
  updateFlow.total = 0;
  updateFlow.startedAt = 0;
  appState.statusMessage = `正在下载 ${info.latest}…`;
  try {
    const r = await downloadAppUpdate(info, (p) => {
      updateFlow.received = Number(p.received) || 0;
      updateFlow.total = Number(p.total) || 0;
      if (!updateFlow.startedAt && updateFlow.received > 0) {
        updateFlow.startedAt = Date.now();
      }
    });
    const path = String((r && r.path) || "").trim();
    if (!path) throw new Error("下载完成但没有文件路径");
    updateFlow.phase = "launching";
    appState.statusMessage = "正在启动新版本…";
    await launchDownloadedUpdate(path);
  } catch (e) {
    updateFlow.phase = "error";
    updateFlow.error = String(e.message || e);
    appState.statusMessage = `更新失败：${updateFlow.error}`;
  }
}
