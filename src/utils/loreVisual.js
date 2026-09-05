/**
 * 角色形象卡：约定 lore.attrs 键
 * 代码路径: kk_novel_ai/src/utils/loreVisual.js
 */

export const VISUAL_ATTR_KEYS = [
  "外貌",
  "发型",
  "瞳色",
  "体态",
  "常服",
  "画风锚",
  "portrait_rel",
];

export function emptyVisualSheet() {
  return {
    外貌: "",
    发型: "",
    瞳色: "",
    体态: "",
    常服: "",
    画风锚: "",
    portrait_rel: "",
  };
}

export function visualFromAttrs(attrs) {
  const a = attrs && typeof attrs === "object" ? attrs : {};
  const out = emptyVisualSheet();
  for (const k of VISUAL_ATTR_KEYS) {
    out[k] = String(a[k] || "");
  }
  return out;
}

export function isVisualAttrKey(k) {
  return VISUAL_ATTR_KEYS.includes(String(k || ""));
}

export function mergeVisualIntoAttrs(attrs, visual) {
  const out = { ...(attrs && typeof attrs === "object" ? attrs : {}) };
  const v = visual && typeof visual === "object" ? visual : {};
  for (const k of VISUAL_ATTR_KEYS) {
    const t = String(v[k] || "").trim();
    if (t) out[k] = t;
    else delete out[k];
  }
  return out;
}

export function formatCharacterSheetLine(entry) {
  if (!entry) return "";
  const v = visualFromAttrs(entry.attrs);
  const bits = [entry.title || ""];
  for (const k of ["外貌", "发型", "瞳色", "体态", "常服", "画风锚"]) {
    if (v[k]) bits.push(`${k}：${v[k]}`);
  }
  return bits.filter(Boolean).join("；");
}

export function formatSheetsForPrompt(entries) {
  const lines = (entries || [])
    .map((e) => formatCharacterSheetLine(e))
    .filter((s) => s && s.trim());
  return lines.length ? lines.join("\n") : "";
}
