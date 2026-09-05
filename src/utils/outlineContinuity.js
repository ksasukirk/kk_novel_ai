/**
 * 拆章/按纲写：时间线与亲属连续约束
 * 代码路径: kk_novel_ai/src/utils/outlineContinuity.js
 */

export const CONTINUITY_MUST_NOT =
  "禁止把上章已发生的用餐、入睡、出行写成尚未发生；禁止同一顿饭再开一桌；禁止改写已确立的亲属（谁是谁家的孩子、表哥表妹属哪一门）。";

export const CONTINUITY_WRITE_HINT =
  "承接上章收束的时间地点与人物状态；上章已用餐或已吃西瓜则按饭后写，禁止喊开饭；亲属称谓以角色卡为准。";

export function isPlaceholderBookTitle(title) {
  const t = String(title || "").trim();
  return !t || /^未命名小说/.test(t);
}

export function seedTitleFromOutline(outline) {
  const line = String(outline || "")
    .trim()
    .split(/\r?\n/)[0]
    .trim()
    .replace(/[\\/:*?"<>|]/g, "")
    .replace(/\s+/g, "");
  if (!line) return "";
  if ([...line].length <= 28) return line;
  return [...line].slice(0, 24).join("");
}

export function composeMustNot(row, bookOutline) {
  const parts = [];
  const user = String((row && (row.must_not || row.mustNot)) || "").trim();
  if (user) parts.push(user);
  const outline = String(bookOutline || "").trim();
  const outlineChars = [...outline.replace(/\s+/g, "")].length;
  if (outline && outlineChars < 80) {
    parts.push(
      "一句话大纲：只推进该句已有动作；末章须留下未兑现的核心愿望/下场钩子，禁止假装全书已经写完。"
    );
  }
  if (!parts.some((p) => p.includes("禁止把上章已发生的用餐"))) {
    parts.push(CONTINUITY_MUST_NOT);
  }
  return parts.join(" ");
}
