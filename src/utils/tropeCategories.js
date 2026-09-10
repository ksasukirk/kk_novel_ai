/**
 * 情节/性癖规范分类标签（与后端 trope_tags.rs 对齐）
 * 代码路径: kk_novel_ai/src/utils/tropeCategories.js
 */

export const TROPE_CATEGORY_IDS = [
  "暴露",
  "公开",
  "后庭",
  "口交",
  "性交",
  "排泄",
  "体罚",
  "羞耻",
  "束缚",
  "控制",
  "群戏",
  "道具",
  "体液",
  "禁忌",
  "撞见",
  "秘密",
];

/** 子串别名 → 规范名（较长别名优先） */
const ALIASES = [
  ["真空", "暴露"],
  ["裙下", "暴露"],
  ["露出", "暴露"],
  ["裸露", "暴露"],
  ["暴露", "暴露"],
  ["半公开", "公开"],
  ["当众", "公开"],
  ["户外", "公开"],
  ["公开", "公开"],
  ["后穴", "后庭"],
  ["屁穴", "后庭"],
  ["肛交", "后庭"],
  ["肛", "后庭"],
  ["后庭", "后庭"],
  ["口侍", "口交"],
  ["口交", "口交"],
  ["抽插", "性交"],
  ["插入", "性交"],
  ["性交", "性交"],
  ["憋尿", "排泄"],
  ["失禁", "排泄"],
  ["排尿", "排泄"],
  ["排泄", "排泄"],
  ["打屁股", "体罚"],
  ["体罚", "体罚"],
  ["羞耻", "羞耻"],
  ["束缚", "束缚"],
  ["捆绑", "束缚"],
  ["遥控", "控制"],
  ["控制", "控制"],
  ["群交", "群戏"],
  ["群戏", "群戏"],
  ["跳蛋", "道具"],
  ["玩具", "道具"],
  ["道具", "道具"],
  ["精液", "体液"],
  ["体液", "体液"],
  ["乱伦", "禁忌"],
  ["禁忌", "禁忌"],
  ["撞见", "撞见"],
  ["被抓", "撞见"],
  ["秘密", "秘密"],
  ["隐瞒", "秘密"],
];

const CANON_SET = new Set(TROPE_CATEGORY_IDS);

function splitRawTags(raw) {
  return String(raw || "")
    .split(/[,，、;；/|]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

/** @param {string} raw */
export function normalizeTropeTag(raw) {
  const s = String(raw || "").trim();
  if (!s) return "";
  if (CANON_SET.has(s)) return s;
  const hay = s.replace(/\s+/g, "");
  let best = "";
  let bestLen = 0;
  for (const [alias, canon] of ALIASES) {
    if (hay.includes(alias) && alias.length >= bestLen) {
      best = canon;
      bestLen = alias.length;
    }
  }
  return best;
}

/** @param {string|string[]} raw */
export function parseTropeTags(raw) {
  const parts = Array.isArray(raw) ? raw : splitRawTags(raw);
  const out = [];
  for (const p of parts) {
    const n = normalizeTropeTag(p);
    if (n && !out.includes(n)) out.push(n);
  }
  return TROPE_CATEGORY_IDS.filter((id) => out.includes(id));
}

/** @param {string[]} tags */
export function formatTropeTags(tags) {
  return parseTropeTags(tags).join(", ");
}

/**
 * @param {string} prev
 * @param {string|string[]} add
 */
export function mergeTropeTags(prev, add) {
  return formatTropeTags([...parseTropeTags(prev), ...parseTropeTags(add)]);
}

/**
 * 旧卡无 tags 时用标题+关键词本地推断，不写盘
 * @param {{ title?: string, keywords?: string[], attrs?: object }} item
 */
export function inferTropeTags(item) {
  const stored = parseTropeTags(item && item.attrs && item.attrs.tags);
  if (stored.length) return stored;
  const hay = [
    (item && item.title) || "",
    ...((item && item.keywords) || []),
  ]
    .join("")
    .replace(/\s+/g, "");
  const out = [];
  for (const [alias, canon] of ALIASES) {
    if (hay.includes(alias) && !out.includes(canon)) out.push(canon);
  }
  return TROPE_CATEGORY_IDS.filter((id) => out.includes(id));
}

export function itemCategoryTags(item) {
  return inferTropeTags(item);
}

export function itemIsUncategorized(item) {
  return itemCategoryTags(item).length === 0;
}

export function categoryLabelKey(id) {
  return `trope.cats.${id}`;
}
