/**
 * 块级蒸馏记忆：写块后异步提炼，供下轮续写；支持手动编辑写回
 * 代码路径: kk_novel_ai/src/services/blockDigest.js
 */
import { invoke } from "./tauri.js";
import { appState } from "../stores/appState.js";
import { saveChapter } from "./projectClient.js";
import { sanitizeBlockDigest } from "../utils/blockDigestSanitize.js";

const inFlightKeys = new Set();

function autoDigestEnabled() {
  const s = appState.settings;
  if (!s) return true;
  if (s.writing_auto_digest === false) return false;
  return true;
}

function applyDigestToBlock(blockKey, digest) {
  const cleaned = sanitizeBlockDigest(digest);
  const list = Array.isArray(appState.chapterBlocks)
    ? appState.chapterBlocks.map((b) => ({ ...b }))
    : [];
  const idx = list.findIndex((b) => b.key === blockKey);
  if (idx < 0) return cleaned;
  list[idx] = { ...list[idx], digest: cleaned };
  appState.chapterBlocks = list;
  void import("./projectClient.js").then((m) => {
    if (typeof m.syncBranchDocFromEditor === "function") m.syncBranchDocFromEditor();
  });
  return cleaned;
}

/**
 * @param {{ blockKey: string, text: string, instruction?: string }} opts
 */
export async function runBlockDigest(opts) {
  const blockKey = (opts && opts.blockKey) || "";
  const text = ((opts && opts.text) || "").trim();
  if (!blockKey || !text) return null;
  if (opts && opts.blockType === "illustration") return null;
  {
    const hit = (appState.chapterBlocks || []).find((b) => b && b.key === blockKey);
    if (hit && (hit.type === "illustration" || hit.type === "illus")) return null;
  }
  if (!appState.projectRoot || !appState.chapterId) return null;
  if (!autoDigestEnabled()) return null;
  if (inFlightKeys.has(blockKey)) return null;
  if (appState.generating && !(opts && opts._syncWait)) return null;

  inFlightKeys.add(blockKey);
  const prevStatus = appState.statusMessage;
  appState.statusMessage = "正在提炼本章记忆…";
  try {
    const result = await invoke("writing_run", {
      request: {
        project_root: appState.projectRoot,
        chapter_id: appState.chapterId,
        task: "block_digest",
        selection: text,
        instruction: (opts && opts.instruction) || "",
        block_key: blockKey,
      },
    });
    let digest = String((result && (result.text || result.raw_text)) || "").trim();
    digest = sanitizeBlockDigest(digest);
    if (digest) {
      applyDigestToBlock(blockKey, digest);
      try {
        await saveChapter();
      } catch {
        /* 正文已在，摘要落盘失败不阻断 */
      }
      appState.statusMessage = "块记忆已更新";
    } else {
      appState.statusMessage = prevStatus || "块摘要为空，续写仍可用";
    }
    return digest || null;
  } catch (e) {
    appState.statusMessage = `块摘要失败，续写仍可用：${e.message || e}`;
    return null;
  } finally {
    inFlightKeys.delete(blockKey);
  }
}

function outlineSyncDigestEnabled() {
  const s = appState.settings;
  if (!s) return true;
  if (s.writing_outline_run_sync_digest === false) return false;
  return true;
}

/**
 * 同步等待块蒸馏（按纲续写用）
 * @param {{ blockKey: string, text: string, instruction?: string }} opts
 * @param {{ timeoutMs?: number, force?: boolean }} [waitOpts]
 */
export async function runBlockDigestAndWait(opts, waitOpts = {}) {
  const force = !!(waitOpts && waitOpts.force);
  if (!force && !outlineSyncDigestEnabled()) {
    return runBlockDigest(opts);
  }
  const timeoutMs = (waitOpts && waitOpts.timeoutMs) || 120000;
  const p = runBlockDigest({ ...opts, _syncWait: true });
  const timer = new Promise((resolve) =>
    setTimeout(() => resolve("__timeout__"), timeoutMs)
  );
  const result = await Promise.race([p, timer]);
  if (result === "__timeout__") {
    appState.statusMessage = "块摘要超时，继续下一节拍";
    return null;
  }
  return result;
}

/**
 * 删除指定块在 memory.json 中的笔记（删生成块 / 清空正文时调用）
 * @param {string} blockKey
 */
export async function removeBlockNote(blockKey) {
  const key = String(blockKey || "").trim();
  if (!key || !appState.projectRoot || !appState.chapterId) return false;
  try {
    await invoke("memory_remove_block_note", {
      root: appState.projectRoot,
      chapterId: appState.chapterId,
      blockKey: key,
    });
    return true;
  } catch {
    return false;
  }
}

/**
 * 手动编辑本段记忆：写回 sidecar + memory.json（自动去婴儿词）
 * @param {{ blockKey: string, digest: string }} opts
 */
export async function saveBlockDigestManual(opts) {
  const blockKey = (opts && opts.blockKey) || "";
  if (!blockKey || !appState.projectRoot || !appState.chapterId) return null;
  const cleaned = applyDigestToBlock(blockKey, (opts && opts.digest) || "");
  try {
    const result = await invoke("memory_upsert_block_note", {
      root: appState.projectRoot,
      chapterId: appState.chapterId,
      blockKey,
      summary: cleaned,
    });
    const serverSummary = String((result && result.summary) || cleaned);
    if (serverSummary !== cleaned) {
      applyDigestToBlock(blockKey, serverSummary);
    }
    await saveChapter();
    appState.statusMessage = "本段记忆已保存";
    return serverSummary;
  } catch (e) {
    appState.statusMessage = `本段记忆保存失败：${e.message || e}`;
    throw e;
  }
}
