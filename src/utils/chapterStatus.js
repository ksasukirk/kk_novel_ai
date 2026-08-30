/**
 * 目录待写章队列：章节状态判定
 * 代码路径: kk_novel_ai/src/utils/chapterStatus.js
 */

/**
 * @param {string} content
 * @param {string} title
 */
export function isChapterBodyEmpty(content, title) {
  let t = String(content || "").trim();
  if (!t) return true;
  t = t.replace(/<!--\s*kk-gen\b[\s\S]*?<!--\s*\/kk-gen\b[^>]*-->/gi, "").trim();
  const titleRe = new RegExp(
    `^#\\s*${String(title || "").replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*`,
    "i"
  );
  t = t.replace(titleRe, "").trim();
  t = t.replace(/^#\s*[^\n]*\n*/i, "").trim();
  return t.length < 8;
}

/**
 * @param {{ status?: string, summary?: string, beats?: unknown[] } | null} ch
 * @param {{ bodyEmpty?: boolean }} [opts]
 * @returns {"pending"|"writing"|"done"}
 */
export function chapterQueueStatus(ch, opts = {}) {
  if (!ch) return "writing";
  const st = String(ch.status || "").trim();
  if (st === "done" || st === "outline_complete") return "done";
  const hasOutline = !!(
    String(ch.summary || "").trim() ||
    (Array.isArray(ch.beats) && ch.beats.length > 0)
  );
  const bodyEmpty = opts.bodyEmpty;
  if (hasOutline && bodyEmpty === true) return "pending";
  if (bodyEmpty === false) return "writing";
  if (st === "pending" && hasOutline) return "pending";
  if (st === "writing" || st === "draft") return "writing";
  if (hasOutline) return "pending";
  return "writing";
}

/**
 * @param {"pending"|"writing"|"done"} status
 */
export function chapterQueueStatusLabel(status) {
  if (status === "pending") return "待写";
  if (status === "done") return "已完成";
  return "写作中";
}

/**
 * @param {"pending"|"writing"|"done"} status
 */
export function chapterQueueStatusClass(status) {
  return `toc-status-${status}`;
}
