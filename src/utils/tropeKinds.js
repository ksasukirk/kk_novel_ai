/**
 * 情节/性癖 lore kind
 * 代码路径: kk_novel_ai/src/utils/tropeKinds.js
 */
export function isTropeKind(kind) {
  const k = String(kind || "").trim();
  return k === "trope" || k === "kink";
}

export function tropeKindLabelKey(kind) {
  return kind === "kink" ? "common.kink" : "common.trope";
}
