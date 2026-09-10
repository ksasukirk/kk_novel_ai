/**
 * 无履历时按当前设置约算用量（不写账本）
 * 代码路径: kk_novel_ai/src/utils/usageEstimate.js
 */

import {
  isDeepseek,
  resolveDeepseekPrices,
  resolveDeepseekPeak,
  resolveDeepseekModelKey,
  DEEPSEEK_OFFICIAL_PRICES,
} from "./deepseekPricing.js";
import { t } from "../i18n/index.js";
import { formatCost } from "./usageFormat.js";

/** 情节扫描切块（与后端 chunk_prose 对齐） */
export const TROPES_SCAN_CHUNK = 7000;
const SCAN_CATALOG_TOKENS = 1200;
const SCAN_COMPLETION_TOKENS = 400;
/** 冻结名单后的保守前缀缓存命中 */
const SCAN_CACHE_HIT = 0.5;

const SYS_OVERHEAD_TOKENS = 800;
/** DeepSeek 粗估缓存命中率 */
const ASSUMED_CACHE_HIT = 0.3;

function resolvePrices(settings) {
  if (isDeepseek(settings)) {
    const p = resolveDeepseekPrices(settings);
    return { hit: p.hit, miss: p.miss, out: p.out, peak: resolveDeepseekPeak(settings) };
  }
  const miss = Number(settings?.price_input_per_1m) || 0;
  const hit =
    Number(settings?.price_cache_hit_per_1m) > 0
      ? Number(settings.price_cache_hit_per_1m)
      : miss;
  const out = Number(settings?.price_output_per_1m) || 0;
  return { hit, miss, out, peak: false };
}

function costFromTokens(prompt, completion, prices, useCacheSplit) {
  const outCost = (completion / 1_000_000) * prices.out;
  if (useCacheSplit) {
    const hit = Math.round(prompt * ASSUMED_CACHE_HIT);
    const miss = prompt - hit;
    return (hit / 1_000_000) * prices.hit + (miss / 1_000_000) * prices.miss + outCost;
  }
  return (prompt / 1_000_000) * prices.miss + outCost;
}

/**
 * @param {object} settings appState.settings
 * @returns {{ mode: 'estimate', perCall: object, scenarios: object, scenarioDaily: array, note: string }}
 */
export function estimateFromSettings(settings) {
  const s = settings || {};
  const target =
    Number(s.writing_target_chars) > 0
      ? Number(s.writing_target_chars)
      : Number(s.max_tokens) > 0
        ? Number(s.max_tokens)
        : 1800;
  const recent = Number(s.recent_window_chars) > 0 ? Number(s.recent_window_chars) : 3000;
  // 中文粗估：约 1 字 ≈ 1 token（略保守）
  const completion = Math.round(target);
  const prompt = Math.round(recent + SYS_OVERHEAD_TOKENS);
  const prices = resolvePrices(s);
  const deepseek = isDeepseek(s);
  const perCost = costFromTokens(prompt, completion, prices, deepseek);

  const scenarios = {
    x10: { calls: 10, cost: perCost * 10, tokens: (prompt + completion) * 10 },
    x50: { calls: 50, cost: perCost * 50, tokens: (prompt + completion) * 50 },
  };

  // 情景：每天 3 次续写，14 天累计花费曲线
  const perDayCalls = 3;
  const scenarioDaily = [];
  let cum = 0;
  const today = new Date();
  today.setHours(12, 0, 0, 0);
  for (let i = 0; i < 14; i++) {
    const d = new Date(today);
    d.setDate(d.getDate() - (13 - i));
    cum += perCost * perDayCalls;
    scenarioDaily.push({
      date: d.toISOString().slice(0, 10),
      cost: cum,
      tokens: (prompt + completion) * perDayCalls * (i + 1),
      calls: perDayCalls * (i + 1),
      prompt: 0,
      completion: 0,
    });
  }

  const peakNote = deepseek && prices.peak ? t("analytics.estPeak") : "";
  const cacheNote = deepseek
    ? t("analytics.estCacheDeepseek", { n: Math.round(ASSUMED_CACHE_HIT * 100) })
    : t("analytics.estCacheOther");

  return {
    mode: "estimate",
    perCall: {
      prompt,
      completion,
      tokens: prompt + completion,
      cost: perCost,
      targetChars: target,
      recentChars: recent,
    },
    scenarios,
    scenarioDaily,
    prices,
    note: t("analytics.estNote", {
      n: target,
      title: recent,
      msg: `${cacheNote}${peakNote}`,
    }),
  };
}

function scanCostFromTokens(prompt, completion, prices, cacheHit) {
  const outCost = (completion / 1_000_000) * prices.out;
  const hit = Math.round(prompt * cacheHit);
  const miss = prompt - hit;
  return (hit / 1_000_000) * prices.hit + (miss / 1_000_000) * prices.miss + outCost;
}

/**
 * 全书扫描约算：每本 ceil(chars/7000) 且至少 1 次。不写账本。
 * @param {{ settings?: object, books?: Array<{ chars?: number }> }} opts
 * @returns {{ calls: number, costIdle: number, costPeak: number, cost: number, deepseek: boolean }}
 */
export function estimateTropesScan(opts) {
  const settings = (opts && opts.settings) || {};
  const books = Array.isArray(opts && opts.books) ? opts.books : [];
  let calls = 0;
  let promptTokens = 0;
  let completionTokens = 0;
  for (const book of books) {
    const chars = Math.max(0, Number(book && book.chars) || 0);
    const n = Math.max(1, Math.ceil(chars / TROPES_SCAN_CHUNK) || 1);
    calls += n;
    let remaining = chars;
    for (let i = 0; i < n; i++) {
      const body = remaining > 0 ? Math.min(TROPES_SCAN_CHUNK, remaining) : 0;
      promptTokens += SCAN_CATALOG_TOKENS + body;
      completionTokens += SCAN_COMPLETION_TOKENS;
      remaining = Math.max(0, remaining - TROPES_SCAN_CHUNK);
    }
  }
  const deepseek = isDeepseek(settings);
  if (deepseek) {
    const key = resolveDeepseekModelKey(settings);
    const idle = DEEPSEEK_OFFICIAL_PRICES[key].idle;
    const peak = DEEPSEEK_OFFICIAL_PRICES[key].peak;
    const costIdle = scanCostFromTokens(promptTokens, completionTokens, idle, SCAN_CACHE_HIT);
    const costPeak = scanCostFromTokens(promptTokens, completionTokens, peak, SCAN_CACHE_HIT);
    return { calls, costIdle, costPeak, cost: costIdle, deepseek: true };
  }
  const miss = Number(settings.price_input_per_1m) || 0;
  const hit =
    Number(settings.price_cache_hit_per_1m) > 0
      ? Number(settings.price_cache_hit_per_1m)
      : miss;
  const out = Number(settings.price_output_per_1m) || 0;
  const cost = scanCostFromTokens(
    promptTokens,
    completionTokens,
    { hit, miss, out },
    0
  );
  return { calls, costIdle: cost, costPeak: cost, cost, deepseek: false };
}

/**
 * @param {{ settings?: object, books?: Array<{ chars?: number }>, n: number, variant: 'all'|'scan' }} opts
 * @returns {string|null}
 */
export function tropesScanConfirmText(opts) {
  try {
    const est = estimateTropesScan({
      settings: opts && opts.settings,
      books: (opts && opts.books) || [],
    });
    if (!est || !Number.isFinite(est.calls) || est.calls < 1) return null;
    const cost = formatCost(est.costIdle);
    const costPeak = formatCost(est.costPeak);
    if (!cost) return null;
    const n = opts && opts.n;
    const variant = (opts && opts.variant) || "scan";
    if (variant === "all") {
      return t(
        est.deepseek ? "project.tropeSummaryAllConfirmCost" : "project.tropeSummaryAllConfirmCostFlat",
        { n, calls: est.calls, cost, costPeak }
      );
    }
    return t(
      est.deepseek ? "trope.scanConfirmCost" : "trope.scanConfirmCostFlat",
      { n, calls: est.calls, cost, costPeak }
    );
  } catch {
    return null;
  }
}
