/**
 * 块记忆摘要清洗：去掉婴儿相关词汇
 * 代码路径: kk_novel_ai/src/utils/blockDigestSanitize.js
 * 词表需与 src-tauri/src/project/digest_sanitize.rs 保持同步
 */

const BABY_TERMS = [
  "breastfeeding",
  "新生儿",
  "小婴儿",
  "婴儿车",
  "婴儿床",
  "尿不湿",
  "小宝宝",
  "初生儿",
  "哺乳期",
  "满月酒",
  "newborn",
  "infant",
  "diaper",
  "cradle",
  "fetus",
  "婴儿",
  "宝宝",
  "胎儿",
  "襁褓",
  "奶嘴",
  "尿布",
  "摇篮",
  "满月",
  "月子",
  "哺乳",
  "孕妇",
  "怀孕",
  "分娩",
  "产检",
  "胎动",
  "羊水",
  "脐带",
  "奶粉",
  "胎教",
  "幼婴",
  "吃奶",
  "母乳",
  "断奶",
  "育婴",
  "baby",
];

function escapeRegExp(s) {
  return String(s).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function tidyDigest(text) {
  let s = String(text || "");
  for (let i = 0; i < 4; i += 1) {
    const next = s
      .replace(/ {2,}/g, " ")
      .replace(/\n{3,}/g, "\n\n")
      .replace(/，{2,}/g, "，")
      .replace(/、{2,}/g, "、")
      .replace(/。{2,}/g, "。")
      .replace(/；{2,}/g, "；")
      .replace(/：{2,}/g, "：")
      .replace(/，。/g, "。")
      .replace(/。，/g, "。")
      .replace(/（）/g, "")
      .replace(/\(\)/g, "")
      .replace(/「」/g, "")
      .replace(/【】/g, "");
    if (next === s) break;
    s = next;
  }
  return s
    .split(/\s+/)
    .filter(Boolean)
    .join(" ")
    .replace(/^[，、。；：,;:\s]+|[，、。；：,;:\s]+$/g, "");
}

/** @param {string} text */
export function sanitizeBlockDigest(text) {
  let out = String(text || "");
  for (const term of BABY_TERMS) {
    if (!term) continue;
    const re = new RegExp(escapeRegExp(term), "gi");
    out = out.replace(re, "");
  }
  return tidyDigest(out);
}
