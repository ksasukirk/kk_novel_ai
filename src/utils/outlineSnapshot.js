/**
 * 按纲写后快照是否可用（拒占位句、拒空章）
 * 代码路径: kk_novel_ai/src/utils/outlineSnapshot.js
 */

/** 与 Rust continuity::DUMP_PLACEHOLDER 同源标记 */
export const DUMP_SNAPSHOT_MARK = "写后总结过长或复读正文";

export function isDumpSnapshot(text) {
  return String(text || "").includes(DUMP_SNAPSHOT_MARK);
}

/**
 * 记忆快照能否当作写后总结（40～400 字，且非占位）
 * @param {string} text
 */
export function snapshotLooksValid(text) {
  const t = String(text || "").trim();
  if (!t) return false;
  if (isDumpSnapshot(t)) return false;
  const n = [...t].length;
  return n >= 40 && n <= 400;
}

/**
 * 去掉空白后是否达到实质正文门槛
 * @param {string} content
 * @param {number} [min=80]
 */
export function bodyHasSubstantialProse(content, min = 80) {
  const n = String(content || "").replace(/\s+/g, "").length;
  return n >= min;
}
