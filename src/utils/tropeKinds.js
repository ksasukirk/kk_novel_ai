/**
 * 情节/喜好/文风 lore kind
 * 代码路径: kk_novel_ai/src/utils/tropeKinds.js
 */
export function isTropeKind(kind) {
  const k = String(kind || "").trim();
  return k === "trope" || k === "kink" || k === "style";
}

export function isPlotKind(kind) {
  const k = String(kind || "").trim();
  return k === "trope" || k === "kink";
}

export function tropeKindLabelKey(kind) {
  if (kind === "kink") return "common.kink";
  if (kind === "style") return "common.style";
  return "common.trope";
}
