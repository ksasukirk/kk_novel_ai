/**
 * 预览文本完整性启发式（半截句 / 流未结束）
 * 代码路径: kk_novel_ai/src/utils/previewText.js
 */

/** 像被 max_tokens / 中途插入掐断的收尾 */
export function looksIncomplete(text) {
  const t = (text || "").trim();
  if (!t) return false;
  const lastLine = t.split(/\n/).filter(Boolean).pop() || "";
  const compact = lastLine.replace(/\s+/g, "");
  if (!compact) return false;

  // 单独一字/两字起笔未写完（如「那」「他」「她」）
  if (compact.length <= 2 && !/[。！？…」』"”)]$/.test(compact)) {
    return true;
  }
  // 以逗号顿号冒号破折号收尾且整段不长
  if (/[，、,：:—\-]$/.test(compact) && compact.length < 80) {
    return true;
  }
  // 无句末标点，且末行较短（常见流式截断）
  if (!/[。！？…」』"”)]$/.test(compact) && compact.length < 48) {
    return true;
  }
  return false;
}

export function previewDiffHint(raw, final) {
  const r = (raw || "").length;
  const f = (final || "").length;
  if (!r || r === f) return "";
  return `原始 ${r} 字 → 定稿 ${f} 字`;
}
