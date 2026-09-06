/**
 * 拆章/按纲写：时间线与亲属连续约束
 * 代码路径: kk_novel_ai/src/utils/outlineContinuity.js
 */
import { appState } from "../stores/appState.js";
import { tLocale } from "../i18n/index.js";

const WRITING_LOCALES = ["zh-CN", "en", "ja"];

function writingT(key, values) {
  return tLocale(appState.settings?.writing_locale || "zh-CN", key, values);
}

export function continuityMustNot() {
  return writingT("continuity.mustNot");
}

export function continuityWriteHint() {
  return writingT("continuity.writeHint");
}

/** @deprecated 请用 continuityMustNot()；保留名称以免旧引用炸 */
export function CONTINUITY_MUST_NOT() {
  return continuityMustNot();
}

/** @deprecated 请用 continuityWriteHint() */
export function CONTINUITY_WRITE_HINT() {
  return continuityWriteHint();
}

export function isPlaceholderBookTitle(title) {
  const s = String(title || "").trim();
  if (!s) return true;
  for (const loc of WRITING_LOCALES) {
    const untitled = tLocale(loc, "project.untitled");
    if (untitled && (s === untitled || s.startsWith(untitled))) return true;
  }
  return /^未命名小说/.test(s);
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

function alreadyHasContinuityMustNot(parts) {
  const texts = WRITING_LOCALES.map((loc) => tLocale(loc, "continuity.mustNot")).filter(Boolean);
  return parts.some((p) => texts.some((text) => p.includes(text) || text.includes(p)));
}

export function composeMustNot(row, bookOutline) {
  const parts = [];
  const user = String((row && (row.must_not || row.mustNot)) || "").trim();
  if (user) parts.push(user);
  const outline = String(bookOutline || "").trim();
  const outlineChars = [...outline.replace(/\s+/g, "")].length;
  if (outline && outlineChars < 80) {
    parts.push(writingT("continuity.oneLineHint"));
  }
  if (!alreadyHasContinuityMustNot(parts)) {
    parts.push(continuityMustNot());
  }
  return parts.join(" ");
}
