/**
 * GitHub Release 检查更新 / 下载
 * 代码路径: kk_novel_ai/src/services/appUpdate.js
 */
import { invoke, listen } from "./tauri.js";

/** 本仓库公开地址；与 src-tauri/src/update.rs 的 GITHUB_REPO 一致 */
export const GITHUB_REPO_URL = "https://github.com/ksasukirk/kk_novel_ai";

export async function getAppAbout() {
  try {
    const r = await invoke("app_version");
    const version = String((r && r.version) || "").trim();
    const githubUrl = String((r && r.github_url) || "").trim() || GITHUB_REPO_URL;
    return { version, githubUrl };
  } catch {
    return { version: "", githubUrl: GITHUB_REPO_URL };
  }
}

export async function getAppVersion() {
  const about = await getAppAbout();
  return about.version;
}

export async function checkAppUpdate() {
  return await invoke("update_check");
}

export async function downloadAppUpdate(info, onProgress) {
  let unlisten = () => {};
  try {
    unlisten = await listen("update-download-progress", (event) => {
      const p = (event && event.payload) || {};
      if (typeof onProgress === "function") onProgress(p);
    });
  } catch {
    unlisten = () => {};
  }
  try {
    return await invoke("update_download", {
      downloadUrl: info.download_url || "",
      apiDownloadUrl: info.api_download_url || "",
      assetName: info.asset_name || "",
      latest: info.latest || "",
    });
  } finally {
    try {
      unlisten();
    } catch {
      /* ignore */
    }
  }
}

export async function revealDownloadedUpdate(path) {
  return await invoke("update_reveal", { path });
}

export async function launchDownloadedUpdate(path) {
  return await invoke("update_launch_and_quit", { path });
}

export async function openExternalUrl(url) {
  const target = String(url || GITHUB_REPO_URL).trim() || GITHUB_REPO_URL;
  try {
    await invoke("open_external_url", { url: target });
  } catch {
    if (typeof window !== "undefined") {
      window.open(target, "_blank", "noopener");
    }
  }
}
