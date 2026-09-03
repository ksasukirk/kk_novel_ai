/**
 * DeepSeek 官方单价与高峰时段推断（与后端 settings.rs 对齐）
 * 代码路径: kk_novel_ai/src/utils/deepseekPricing.js
 */

/** 元/百万 tokens：hit / miss / output */
export const DEEPSEEK_OFFICIAL_PRICES = {
  flash: {
    idle: { hit: 0.05, miss: 1.5, out: 4.5 },
    peak: { hit: 0.1, miss: 3.0, out: 9.0 },
  },
  pro: {
    idle: { hit: 0.15, miss: 4.5, out: 13.5 },
    peak: { hit: 0.3, miss: 9.0, out: 27.0 },
  },
};

export const DEEPSEEK_PEAK_NOTICE =
  "当前为 DeepSeek 高峰时段（周一至周五 9:00–12:00、14:00–18:00 北京时间），API 单价为空闲时段的 2 倍；大批量生成建议改到晚间或周末";

let lastPeakToastAt = 0;
const PEAK_TOAST_COOLDOWN_MS = 30 * 60 * 1000;

export function isDeepseek(settings) {
  if (!settings) return false;
  const u = String(settings.base_url || "").toLowerCase();
  const p = String(settings.api_provider || "");
  return u.includes("deepseek.com") || p === "deepseek_flash" || p === "deepseek_pro";
}

/** 北京时间是否高峰：周一至周五 9–12、14–18 */
export function deepseekPeakNow() {
  const now = new Date();
  const bj = new Date(now.getTime() + (now.getTimezoneOffset() + 480) * 60000);
  const wd = bj.getDay();
  if (wd === 0 || wd === 6) return false;
  const h = bj.getHours();
  return (h >= 9 && h < 12) || (h >= 14 && h < 18);
}

export function resolveDeepseekPeak(settings) {
  const tier = String(settings?.deepseek_pricing_tier || "auto").toLowerCase();
  if (tier === "peak") return true;
  if (tier === "idle" || tier === "off_peak" || tier === "offpeak") return false;
  return deepseekPeakNow();
}

export function resolveDeepseekModelKey(settings, modelUsed = "") {
  const m = String(modelUsed || settings?.model || "").toLowerCase();
  return m.includes("pro") ? "pro" : "flash";
}

export function resolveDeepseekPrices(settings, modelUsed = "") {
  const peak = resolveDeepseekPeak(settings);
  const key = resolveDeepseekModelKey(settings, modelUsed);
  return DEEPSEEK_OFFICIAL_PRICES[key][peak ? "peak" : "idle"];
}

export function deepseekPeakNotice(settings) {
  if (!isDeepseek(settings) || !resolveDeepseekPeak(settings)) return "";
  return DEEPSEEK_PEAK_NOTICE;
}

export function deepseekGeneratingStatusSuffix(settings) {
  if (!isDeepseek(settings) || !resolveDeepseekPeak(settings)) return "";
  return "（DeepSeek 高峰单价 ×2）";
}

/**
 * 生成前高峰提示：toast + 返回文案
 * @param {object} settings
 * @param {{ toastFn?: Function, force?: boolean }} [opts]
 */
export function notifyDeepseekPeakIfNeeded(settings, opts = {}) {
  const notice = deepseekPeakNotice(settings);
  if (!notice) return "";
  const now = Date.now();
  const force = !!opts.force;
  if (!force && now - lastPeakToastAt < PEAK_TOAST_COOLDOWN_MS) {
    return notice;
  }
  lastPeakToastAt = now;
  const toastFn = opts.toastFn;
  if (typeof toastFn === "function") {
    toastFn(notice, { duration: 7500 });
  }
  return notice;
}
