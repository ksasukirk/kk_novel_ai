/**
 * 无履历时按当前设置约算用量（不写账本）
 * 代码路径: kk_novel_ai/src/utils/usageEstimate.js
 */

import {
  isDeepseek,
  resolveDeepseekPrices,
  resolveDeepseekPeak,
} from "./deepseekPricing.js";

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

  const peakNote = deepseek && prices.peak ? "当前按高峰单价约算。" : "";
  const cacheNote = deepseek
    ? `DeepSeek 假设约 ${Math.round(ASSUMED_CACHE_HIT * 100)}% 缓存命中。`
    : "非 DeepSeek 按输入全未命中计。";

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
    note: `估算 · 非实测。按当前设置单次续写约算（规定字数 ${target}、上下文窗 ${recent}）。${cacheNote}${peakNote}`,
  };
}
