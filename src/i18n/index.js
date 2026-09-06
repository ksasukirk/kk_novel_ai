/**
 * vue-i18n 入口：界面语言 zh-CN / en / ja
 * 代码路径: kk_novel_ai/src/i18n/index.js
 */
import { createI18n } from "vue-i18n";
import zhCN from "./locales/zh-CN.json" with { type: "json" };
import en from "./locales/en.json" with { type: "json" };
import ja from "./locales/ja.json" with { type: "json" };

export const UI_LOCALES = [
  { id: "zh-CN", native: "中文" },
  { id: "en", native: "English" },
  { id: "ja", native: "日本語" },
];

export function normalizeLocale(raw) {
  const s = String(raw || "").trim();
  if (s === "en" || s === "en-US" || s === "en-GB") return "en";
  if (s === "ja" || s === "ja-JP") return "ja";
  return "zh-CN";
}

export const i18n = createI18n({
  legacy: false,
  locale: "zh-CN",
  fallbackLocale: "zh-CN",
  missingWarn: false,
  fallbackWarn: false,
  messages: {
    "zh-CN": zhCN,
    en,
    ja,
  },
});

export function t(key, values) {
  return i18n.global.t(key, values || {});
}

export function tLocale(locale, key, values) {
  const loc = normalizeLocale(locale);
  if (values && Object.keys(values).length) {
    return i18n.global.t(key, loc, values);
  }
  return i18n.global.t(key, loc);
}

/** 后端取消文案：中 / 英 / 日 */
export function isCancelledMsg(msg) {
  return /取消|cancel|キャンセル/i.test(String(msg || ""));
}

const LOCALE_IDS = ["zh-CN", "en", "ja"];

/** 用任意界面语言的译文去匹配后端/状态字符串 */
export function msgMatchesKey(msg, key) {
  const s = String(msg || "");
  if (!s) return false;
  for (const loc of LOCALE_IDS) {
    const text = tLocale(loc, key);
    if (text && text !== key && s.includes(text)) return true;
  }
  return false;
}

export function compareLocale() {
  const loc = normalizeLocale(i18n.global.locale.value);
  if (loc === "en") return "en";
  if (loc === "ja") return "ja";
  return "zh";
}

export function applyUiLocale(locale) {
  const loc = normalizeLocale(locale);
  i18n.global.locale.value = loc;
  try {
    document.documentElement.lang = loc === "zh-CN" ? "zh-CN" : loc;
  } catch {
    /* ignore */
  }
  return loc;
}
