/**
 * 情节/性癖近义判断（与 Rust project/trope_merge.rs 对齐）
 * 代码路径: kk_novel_ai/src/utils/tropeMatch.js
 */

const PUNCT = /[\s，。、；：！？「」『』（）【】《》—…·・,.;:!?'"()[\]{}<>/\\|_~`@#$%^&*+=\u3000-]+/g;

const GENERIC_ALIAS = new Set([
  "高潮",
  "自慰",
  "插入",
  "触碰",
  "口交",
  "手指",
  "身体",
  "亲吻",
  "接吻",
  "射精",
  "湿润",
  "疼痛",
  "暴露",
  "裸体",
  "交合",
  "后入",
  "前戏",
  "磨蹭",
  "轮廓",
]);

export function tropeNorm(s) {
  return String(s || "")
    .trim()
    .replace(/^\[.*?\]\s*/, "")
    .toLowerCase()
    .replace(PUNCT, "");
}

function charLen(s) {
  return Array.from(s || "").length;
}

function jaccardChars(a, b) {
  const sa = new Set(Array.from(a));
  const sb = new Set(Array.from(b));
  if (!sa.size || !sb.size) return 0;
  let inter = 0;
  sa.forEach((c) => {
    if (sb.has(c)) inter += 1;
  });
  return inter / new Set([...sa, ...sb]).size;
}

export function titlesSimilar(a, b) {
  const na = tropeNorm(a);
  const nb = tropeNorm(b);
  if (!na || !nb) return false;
  if (na === nb) return true;
  const [short, long] = charLen(na) <= charLen(nb) ? [na, nb] : [nb, na];
  if (charLen(short) >= 3 && long.includes(short)) return true;
  const sa = new Set(Array.from(na));
  const sb = new Set(Array.from(nb));
  let inter = 0;
  sa.forEach((c) => {
    if (sb.has(c)) inter += 1;
  });
  return inter >= 2 && jaccardChars(na, nb) >= 0.55;
}

function aliasList(entry) {
  const out = [];
  const t = tropeNorm(entry && entry.title);
  if (t) out.push(t);
  for (const k of (entry && entry.keywords) || []) {
    const n = tropeNorm(k);
    if (charLen(n) >= 2 && !out.includes(n)) out.push(n);
  }
  return out;
}

function twoAliasesMakeTitle(aliases, title) {
  const t = tropeNorm(title);
  if (charLen(t) < 3) return false;
  for (let i = 0; i < aliases.length; i += 1) {
    for (let j = 0; j < aliases.length; j += 1) {
      if (i === j) continue;
      if (GENERIC_ALIAS.has(aliases[i]) || GENERIC_ALIAS.has(aliases[j])) continue;
      if (`${aliases[i]}${aliases[j]}` === t) return true;
    }
  }
  return false;
}

function distinctiveKeywordOverlap(a, b) {
  const ka = new Set(
    ((a && a.keywords) || []).map((k) => tropeNorm(k)).filter((k) => charLen(k) >= 2)
  );
  const kb = new Set(
    ((b && b.keywords) || []).map((k) => tropeNorm(k)).filter((k) => charLen(k) >= 2)
  );
  const shared = [...ka].filter((k) => kb.has(k));
  if (shared.length < 2) return false;
  return shared.some((k) => charLen(k) >= 3);
}

export function tropesAreSimilar(a, b) {
  if (!a || !b) return false;
  const ka = a.kind === "kink" ? "kink" : "trope";
  const kb = b.kind === "kink" ? "kink" : "trope";
  if (ka !== kb) return false;
  if (a.id && b.id && a.id === b.id) return true;
  if (titlesSimilar(a.title, b.title)) return true;
  const aa = aliasList(a);
  const ba = aliasList(b);
  const tb = tropeNorm(b.title);
  const ta = tropeNorm(a.title);
  if (aa.some((x) => x === tb || titlesSimilar(x, b.title))) return true;
  if (ba.some((x) => x === ta || titlesSimilar(x, a.title))) return true;
  if (twoAliasesMakeTitle(aa, b.title) || twoAliasesMakeTitle(ba, a.title)) return true;
  return distinctiveKeywordOverlap(a, b);
}

export function findSimilarTrope(list, title, keywords, kind) {
  const probe = {
    id: "",
    kind: kind === "kink" ? "kink" : "trope",
    title: title || "",
    keywords: keywords || [],
  };
  return (list || []).find((e) => tropesAreSimilar(e, probe)) || null;
}
