/**
 * 流式生成进度估算
 * 代码路径: kk_novel_ai/src/utils/genProgress.js
 */

/**
 * 目标字数：优先用规定字数 writing_target_chars；否则由 max_tokens 反推
 * @param {number} maxTokens
 * @param {number} [writingTargetChars]
 */
export function estimateTargetChars(maxTokens, writingTargetChars) {
  const tc = Number(writingTargetChars);
  if (Number.isFinite(tc) && tc >= 200) return Math.round(tc);
  const mt = Number(maxTokens) || 2048;
  // 与后端一致：max_tokens ≈ 规定字数 × 1.8（允许超出）
  return Math.max(480, Math.round(mt / 1.8));
}

/**
 * @param {number} streamedChars
 * @param {number} targetChars
 * @param {boolean} generating
 * @param {boolean} finished
 */
export function calcGenProgressPct(streamedChars, targetChars, generating, finished) {
  if (finished) return 100;
  if (!generating) return 0;
  const target = Math.max(1, targetChars || 800);
  const n = Math.max(0, streamedChars || 0);
  if (n <= 0) return 2;
  // 未完成前最高 92%，避免早早顶满
  return Math.min(92, Math.max(2, Math.round((n / target) * 100)));
}
