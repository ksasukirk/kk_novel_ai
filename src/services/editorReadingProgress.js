/**
 * 写作区阅读进度（章节 / 激活分支路径）
 * 代码路径: kk_novel_ai/src/services/editorReadingProgress.js
 */
import { activePathNodes } from "../utils/branchModel.js";

const STORAGE_PREFIX = "kk_editor_reading_progress:";

/**
 * @param {unknown} doc
 * @returns {string}
 */
export function activePathKey(doc) {
  if (!doc) return "";
  try {
    return activePathNodes(doc)
      .map((n) => `${n.id}:${n.activeVariantId || ""}`)
      .join("|");
  } catch {
    return "";
  }
}

function storageKey(projectRoot) {
  return `${STORAGE_PREFIX}${String(projectRoot || "")}`;
}

/**
 * @param {string} projectRoot
 * @returns {Record<string, { scrollTop?: number, blockKey?: string, updatedAt?: number }>}
 */
export function loadProgressMap(projectRoot) {
  if (!projectRoot) return {};
  try {
    const raw = localStorage.getItem(storageKey(projectRoot));
    if (!raw) return {};
    const obj = JSON.parse(raw);
    return obj && typeof obj === "object" && !Array.isArray(obj) ? obj : {};
  } catch {
    return {};
  }
}

/**
 * @param {string} projectRoot
 * @param {Record<string, unknown>} map
 */
function writeProgressMap(projectRoot, map) {
  if (!projectRoot) return;
  try {
    localStorage.setItem(storageKey(projectRoot), JSON.stringify(map));
  } catch {
    /* quota / private mode */
  }
}

/**
 * @param {string} chapterId
 * @param {string} [pathKey]
 */
export function progressEntryKey(chapterId, pathKey = "") {
  const id = String(chapterId || "");
  if (!id) return "";
  const pk = String(pathKey || "");
  return pk ? `${id}@@${pk}` : id;
}

/**
 * @param {string} projectRoot
 * @param {string} chapterId
 * @param {{ scrollTop?: number, blockKey?: string }} entry
 * @param {string} [pathKey]
 */
export function saveChapterProgress(projectRoot, chapterId, entry, pathKey = "") {
  if (!projectRoot || !chapterId || !entry) return;
  const map = loadProgressMap(projectRoot);
  const payload = {
    scrollTop: Math.max(0, Number(entry.scrollTop) || 0),
    blockKey: String(entry.blockKey || ""),
    updatedAt: Date.now(),
  };
  map[progressEntryKey(chapterId, "")] = payload;
  if (pathKey) {
    map[progressEntryKey(chapterId, pathKey)] = payload;
  }
  writeProgressMap(projectRoot, map);
}

/**
 * @param {string} projectRoot
 * @param {string} chapterId
 * @param {string} [pathKey]
 * @returns {{ scrollTop: number, blockKey: string, updatedAt?: number } | null}
 */
export function getChapterProgress(projectRoot, chapterId, pathKey = "") {
  if (!projectRoot || !chapterId) return null;
  const map = loadProgressMap(projectRoot);
  const keyed = pathKey ? map[progressEntryKey(chapterId, pathKey)] : null;
  const fallback = map[progressEntryKey(chapterId, "")];
  const raw = keyed || fallback;
  if (!raw || typeof raw !== "object") return null;
  return {
    scrollTop: Math.max(0, Number(raw.scrollTop) || 0),
    blockKey: String(raw.blockKey || ""),
    updatedAt: raw.updatedAt,
  };
}

/**
 * @param {Element | null | undefined} scroller
 * @param {string} [blockKey]
 */
export function captureScrollerProgress(scroller, blockKey = "") {
  return {
    scrollTop: scroller && typeof scroller.scrollTop === "number" ? scroller.scrollTop : 0,
    blockKey: String(blockKey || ""),
  };
}

/**
 * @param {Element | null | undefined} scroller
 * @param {number} scrollTop
 */
export function applyScrollerProgress(scroller, scrollTop) {
  if (!scroller || typeof scroller.scrollTop !== "number") return;
  const max = Math.max(0, (scroller.scrollHeight || 0) - (scroller.clientHeight || 0));
  scroller.scrollTop = Math.min(Math.max(0, Number(scrollTop) || 0), max);
}
