/**
 * 情节/喜好规范文风标签（与后端 trope_styles.rs 对齐）
 * 代码路径: kk_novel_ai/src/utils/tropeStyles.js
 */

export const TROPE_STYLE_IDS = [
  "含蓄",
  "露骨",
  "直言不讳",
  "肮脏",
  "细腻",
  "粗口",
  "克制",
  "淫秽",
];

/** 子串别名 → 规范名（较长别名优先） */
const ALIASES = [
  ["含蓄", "含蓄"],
  ["委婉", "含蓄"],
  ["暗示", "含蓄"],
  ["隐晦", "含蓄"],
  ["露骨", "露骨"],
  ["直白露骨", "露骨"],
  ["explicit", "露骨"],
  ["直言不讳", "直言不讳"],
  ["直白", "直言不讳"],
  ["不掩饰", "直言不讳"],
  ["blunt", "直言不讳"],
  ["肮脏", "肮脏"],
  ["dirty", "肮脏"],
  ["龌龊", "肮脏"],
  ["下贱", "肮脏"],
  ["细腻", "细腻"],
  ["细致", "细腻"],
  ["感官细", "细腻"],
  ["粗口", "粗口"],
  ["脏话", "粗口"],
  ["脏话连篇", "粗口"],
  ["克制", "克制"],
  ["收敛", "克制"],
  ["节制", "克制"],
  ["淫秽", "淫秽"],
  ["秽亵", "淫秽"],
  ["obscene", "淫秽"],
];

const CANON_SET = new Set(TROPE_STYLE_IDS);

function splitRawStyles(raw) {
  return String(raw || "")
    .split(/[,，、;；/|]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

/** @param {string} raw */
export function normalizeTropeStyle(raw) {
  const s = String(raw || "").trim();
  if (!s) return "";
  if (CANON_SET.has(s)) return s;
  const hay = s.replace(/\s+/g, "");
  const hayLower = hay.toLowerCase();
  let best = "";
  let bestLen = 0;
  for (const [alias, canon] of ALIASES) {
    if (alias.length < bestLen) continue;
    const hit = /^[a-z]+$/i.test(alias)
      ? hayLower.includes(alias.toLowerCase())
      : hay.includes(alias);
    if (hit) {
      best = canon;
      bestLen = alias.length;
    }
  }
  return best;
}

/** @param {string|string[]} raw */
export function parseTropeStyles(raw) {
  const parts = Array.isArray(raw) ? raw : splitRawStyles(raw);
  const out = [];
  for (const p of parts) {
    const n = normalizeTropeStyle(p);
    if (n && !out.includes(n)) out.push(n);
  }
  return TROPE_STYLE_IDS.filter((id) => out.includes(id));
}

/** @param {string[]} styles */
export function formatTropeStyles(styles) {
  return parseTropeStyles(styles).join(", ");
}

/**
 * @param {string} prev
 * @param {string|string[]} add
 */
export function mergeTropeStyles(prev, add) {
  return formatTropeStyles([...parseTropeStyles(prev), ...parseTropeStyles(add)]);
}

/**
 * @param {{ attrs?: object }} item
 */
export function itemStyleTags(item) {
  return parseTropeStyles(item && item.attrs && item.attrs.styles);
}

export function itemHasNoStyles(item) {
  return itemStyleTags(item).length === 0;
}

export function styleLabelKey(id) {
  return `trope.styles.${id}`;
}
