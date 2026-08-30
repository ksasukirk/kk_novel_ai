/**
 * 行级 Diff（红删绿增）
 * 代码路径: kk_novel_ai/src/utils/lineDiff.js
 */

/**
 * @param {string} a
 * @param {string} b
 * @returns {{ type: 'equal'|'add'|'remove', text: string }[]}
 */
export function lineDiff(a, b) {
  const aa = (a || "").replace(/\r\n/g, "\n").split("\n");
  const bb = (b || "").replace(/\r\n/g, "\n").split("\n");
  const n = aa.length;
  const m = bb.length;
  const dp = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      if (aa[i] === bb[j]) dp[i][j] = dp[i + 1][j + 1] + 1;
      else dp[i][j] = Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const out = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (aa[i] === bb[j]) {
      out.push({ type: "equal", text: aa[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push({ type: "remove", text: aa[i] });
      i++;
    } else {
      out.push({ type: "add", text: bb[j] });
      j++;
    }
  }
  while (i < n) {
    out.push({ type: "remove", text: aa[i++] });
  }
  while (j < m) {
    out.push({ type: "add", text: bb[j++] });
  }
  return out;
}
