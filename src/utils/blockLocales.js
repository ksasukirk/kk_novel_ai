/**
 * 块级多语言译文：text 始终为源语言正文，locales 仅存译文
 * 代码路径: kk_novel_ai/src/utils/blockLocales.js
 */
import { normalizeLocale, UI_LOCALES } from "../i18n/index.js";

export const BLOCK_LOCALE_IDS = UI_LOCALES.map((l) => l.id);

/**
 * @param {unknown} locale
 * @returns {boolean}
 */
export function isKnownLocale(locale) {
  const id = normalizeLocale(locale);
  return BLOCK_LOCALE_IDS.includes(id);
}

/**
 * @param {unknown} raw
 * @returns {Record<string, string>}
 */
export function normalizeLocalesMap(raw) {
  /** @type {Record<string, string>} */
  const out = {};
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return out;
  for (const [k, v] of Object.entries(raw)) {
    const id = normalizeLocale(k);
    if (!BLOCK_LOCALE_IDS.includes(id)) continue;
    const text = String(v ?? "");
    if (text) out[id] = text;
  }
  return out;
}

/**
 * @param {object} block
 * @param {string} [fallback]
 * @returns {string}
 */
export function ensureSourceLocale(block, fallback = "zh-CN") {
  if (!block || typeof block !== "object") return normalizeLocale(fallback);
  const existing = String(block.sourceLocale || "").trim();
  if (existing) {
    block.sourceLocale = normalizeLocale(existing);
    return block.sourceLocale;
  }
  block.sourceLocale = normalizeLocale(fallback);
  return block.sourceLocale;
}

/**
 * @param {object|null|undefined} block
 * @param {string} locale
 * @returns {string}
 */
export function getBlockLocaleText(block, locale) {
  if (!block) return "";
  const id = String(locale || "").trim();
  if (!id) return String(block.text ?? "");
  const normalized = normalizeLocale(id);
  const src = String(block.sourceLocale || "").trim()
    ? normalizeLocale(block.sourceLocale)
    : "";
  if (!src || normalized === src) return String(block.text ?? "");
  const map = block.locales && typeof block.locales === "object" ? block.locales : {};
  return String(map[normalized] ?? "");
}

/**
 * 写入译文（不改 text）
 * @param {object} block
 * @param {string} locale
 * @param {string} text
 * @returns {object}
 */
export function setBlockLocaleText(block, locale, text) {
  if (!block || typeof block !== "object") return block;
  const id = normalizeLocale(locale);
  if (!BLOCK_LOCALE_IDS.includes(id)) return block;
  if (!block.locales || typeof block.locales !== "object" || Array.isArray(block.locales)) {
    block.locales = {};
  }
  const body = String(text ?? "");
  if (body) block.locales[id] = body;
  else delete block.locales[id];
  return block;
}

/**
 * @param {object|null|undefined} block
 * @returns {string[]}
 */
export function listBlockLocales(block) {
  return Object.keys(normalizeLocalesMap(block?.locales));
}

/**
 * @param {object|null|undefined} block
 * @param {string} locale
 * @returns {boolean}
 */
export function hasBlockLocale(block, locale) {
  const id = String(locale || "").trim();
  if (!id) return false;
  const normalized = normalizeLocale(id);
  const src = String(block?.sourceLocale || "").trim()
    ? normalizeLocale(block.sourceLocale)
    : "";
  if (src && normalized === src) return true;
  return !!(block?.locales && String(block.locales[normalized] || "").trim());
}

/**
 * 旧块补默认；禁止把源语言塞进 locales
 * @param {object} block
 * @param {string} [fallbackSource]
 * @returns {object}
 */
export function normalizeBlockLocales(block, fallbackSource = "zh-CN") {
  if (!block || typeof block !== "object") return block;
  ensureSourceLocale(block, fallbackSource);
  block.locales = normalizeLocalesMap(block.locales);
  const src = block.sourceLocale;
  if (src && block.locales[src]) delete block.locales[src];
  return block;
}

/**
 * 从 source 拷贝 sourceLocale/locales 到 target 并规范化
 * @param {object} target
 * @param {object|null|undefined} source
 * @param {string} [fallbackSource]
 * @returns {object}
 */
export function attachLocalesFields(target, source, fallbackSource = "zh-CN") {
  if (!target || typeof target !== "object") return target;
  if (source && typeof source === "object") {
    if (source.sourceLocale) {
      target.sourceLocale = normalizeLocale(source.sourceLocale);
    }
    if (source.locales) {
      target.locales = normalizeLocalesMap(source.locales);
    }
  }
  if (target.sourceLocale || (target.locales && Object.keys(target.locales).length)) {
    return normalizeBlockLocales(target, fallbackSource);
  }
  return target;
}

/**
 * 是否正在阅读译文（非源语言）
 * @param {object|null|undefined} block
 * @param {string} readingLocale
 * @param {string} [fallbackSource]
 */
export function isReadingTranslation(block, readingLocale, fallbackSource = "zh-CN") {
  const id = String(readingLocale || "").trim();
  if (!id) return false;
  const normalized = normalizeLocale(id);
  const src = String(block?.sourceLocale || "").trim()
    ? normalizeLocale(block.sourceLocale)
    : normalizeLocale(fallbackSource);
  return normalized !== src;
}
