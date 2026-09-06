/**
 * Token / 缓存 / 费用展示格式化
 * 代码路径: kk_novel_ai/src/utils/usageFormat.js
 */
import { t } from "../i18n/index.js";

export function usageTotalTokens(u) {
  if (!u) return 0;
  return u.total_tokens || (u.prompt_tokens || 0) + (u.completion_tokens || 0);
}

/** 缓存命中率 0–100；无分项返回 null */
export function cacheHitRate(u) {
  if (!u) return null;
  const hit = u.prompt_cache_hit_tokens || 0;
  const miss = u.prompt_cache_miss_tokens || 0;
  if (hit <= 0 && miss <= 0) return null;
  return Math.round((hit / (hit + miss)) * 100);
}

export function formatCacheHit(u) {
  if (!u) return "";
  const hit = u.prompt_cache_hit_tokens || 0;
  const miss = u.prompt_cache_miss_tokens || 0;
  if (hit <= 0 && miss <= 0) return "";
  const rate = cacheHitRate(u);
  return t("analytics.cacheHitFmt", { n: hit, title: hit + miss, msg: rate });
}

export function formatTokens(u) {
  if (!u) return "";
  const total = usageTotalTokens(u);
  const src = u.source === "api" ? "api" : t("ai.tokenEst");
  const cache = formatCacheHit(u);
  if (cache) return `${total} tok (${src}) · ${cache}`;
  return `${total} tok (${src})`;
}

export function formatCost(cny) {
  if (cny == null || cny === "") return "";
  return `¥${Number(cny).toFixed(4)}`;
}

export function formatMessages(item) {
  const msgs = (item && item.messages) || [];
  if (!msgs.length) return (item && item.instruction) || "";
  return msgs.map((m) => `【${m.role}】\n${m.content || ""}`).join("\n\n---\n\n");
}

/** bucket: ModelBucket 或 global 摘要 */
export function bucketTotalTokens(b) {
  if (!b) return 0;
  return b.total_tokens || (b.prompt_tokens || 0) + (b.completion_tokens || 0);
}

export function bucketCacheRate(b) {
  if (!b) return null;
  const hit = b.prompt_cache_hit_tokens || 0;
  const miss = b.prompt_cache_miss_tokens || 0;
  if (hit <= 0 && miss <= 0) return null;
  return Math.round((hit / (hit + miss)) * 100);
}
