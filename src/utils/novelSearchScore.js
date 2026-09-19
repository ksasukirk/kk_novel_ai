/**
 * 作品页本地关键词 / 近义打分（与 Rust work_catalog::score_local 对齐）
 * 代码路径: kk_novel_ai/src/utils/novelSearchScore.js
 */
import { titlesSimilar, tropeNorm } from "./tropeMatch.js";

const PUNCT_SPLIT = /[\s，。、；：！？「」『』（）【】《》—…·・,.;:!?'"()[\]{}<>/\\|_~`@#$%^&*+=\u3000]+/;

export function normalizeRoot(s) {
  return String(s || "")
    .replace(/\\/g, "/")
    .replace(/\/+$/, "")
    .toLowerCase();
}

export function tokenizeQuery(q) {
  const raw = String(q || "").trim().toLowerCase();
  if (!raw) return [];
  const out = [raw];
  for (const part of raw.split(PUNCT_SPLIT)) {
    const p = String(part || "").trim();
    if (p.length >= 2 && p !== raw && !out.includes(p)) out.push(p);
  }
  return out;
}

function containsCi(hay, token) {
  return String(hay || "").toLowerCase().includes(String(token || "").toLowerCase());
}

function tropesHay(card) {
  const list = Array.isArray(card && card.tropes) ? card.tropes : [];
  return list.join(" ");
}

/**
 * @param {object} card
 * @param {string[]} tokens
 */
export function scoreCard(card, tokens) {
  const c = card || {};
  let score = 0;
  const fields = [];
  const push = (name) => {
    if (!fields.includes(name)) fields.push(name);
  };
  const tropes = tropesHay(c);
  const tropeList = Array.isArray(c.tropes) ? c.tropes : [];
  for (const tok of tokens) {
    if (containsCi(c.title, tok)) {
      score += 100;
      push("title");
    } else if (titlesSimilar(c.title, tok) || tropeNorm(c.title).includes(tropeNorm(tok))) {
      score += 70;
      push("title");
    }
    const tropeHit = containsCi(tropes, tok) || tropeList.some((t) => titlesSimilar(t, tok));
    if (tropeHit) {
      score += containsCi(tropes, tok) ? 50 : 35;
      push("tropes");
    }
    if (containsCi(c.outline, tok)) {
      score += 25;
      push("outline");
    }
    if (containsCi(c.chapters, tok)) {
      score += 25;
      push("chapters");
    }
    if (containsCi(c.memory, tok)) {
      score += 25;
      push("memory");
    }
    if (containsCi(c.genre, tok) || containsCi(c.style, tok)) {
      score += 20;
      push("genre");
    }
    if (containsCi(c.root, tok)) {
      score += 10;
      push("path");
    }
  }
  return {
    root: c.root || "",
    title: c.title || "",
    score,
    reason: "",
    match_fields: fields,
  };
}

/**
 * @param {object[]} cards
 * @param {string} query
 */
export function scoreCatalog(cards, query) {
  const tokens = tokenizeQuery(query);
  if (!tokens.length) return [];
  const list = Array.isArray(cards) ? cards : [];
  return list
    .map((c) => scoreCard(c, tokens))
    .filter((h) => h.score > 0)
    .sort((a, b) => b.score - a.score || String(a.title).localeCompare(String(b.title)));
}
