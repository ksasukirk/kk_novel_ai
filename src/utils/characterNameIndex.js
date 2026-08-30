/**
 * 角色名索引与正文命中分段
 * 代码路径: kk_novel_ai/src/utils/characterNameIndex.js
 */

/** 不作角色名触发的设定标签 */
const TAG_DENY = new Set([
  "真空",
  "清理",
  "软黏",
  "女体",
  "男体",
  "厌接受",
  "憋尿",
  "密友",
  "非亲属",
  "男主",
  "表妹",
  "身份",
  "解剖",
  "可涩",
]);

function isLatinTerm(s) {
  return /^[A-Za-z0-9][A-Za-z0-9_-]*$/.test(s);
}

function isNameLike(s) {
  const t = String(s || "").trim();
  if (!t || t.length > 12) return false;
  if (TAG_DENY.has(t)) return false;
  if (/^[\u4e00-\u9fff·•]{1,6}$/.test(t)) return true;
  if (isLatinTerm(t) && t.length >= 2) return true;
  return false;
}

function hasLatinBoundary(text, start, len) {
  const before = start > 0 ? text[start - 1] : "";
  const after = start + len < text.length ? text[start + len] : "";
  const isWord = (c) => /[A-Za-z0-9_]/.test(c);
  if (before && isWord(before)) return false;
  if (after && isWord(after)) return false;
  return true;
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * 合并本篇优先的角色列表
 * @param {{ local?: any[], global?: any[] }} scoped
 */
export function coalesceCharacters(scoped) {
  const byTitle = new Map();
  const push = (row, scope) => {
    const entry = row.entry || row;
    if (!entry || (entry.kind && entry.kind !== "character")) return;
    const key = (entry.title || "").trim() || entry.id;
    if (!key) return;
    if (scope === "local" || !byTitle.has(key)) {
      byTitle.set(key, {
        ...entry,
        scope,
        _root: row.root || entry._root || "",
      });
    }
  };
  for (const row of scoped.local || []) push(row, "local");
  for (const row of scoped.global || []) push(row, "global");
  // 兼容扁平 list
  if (!scoped.local && !scoped.global && Array.isArray(scoped)) {
    for (const row of scoped) push(row, row.scope || "local");
  }
  return [...byTitle.values()].sort((a, b) =>
    String(a.title).localeCompare(String(b.title), "zh")
  );
}

/**
 * @param {Array} characters
 * @returns {{ terms: Array<{term:string,id:string,entry:any}>, byId: Map<string, any> }}
 */
export function buildCharacterNameIndex(characters) {
  const byId = new Map();
  const terms = [];
  for (const c of characters || []) {
    if (!c || !c.id) continue;
    byId.set(c.id, c);
    const names = new Set();
    const title = String(c.title || "").trim();
    if (title) names.add(title);
    for (const raw of c.keywords || []) {
      let k = String(raw || "").trim();
      if (/^alias:/i.test(k)) k = k.replace(/^alias:/i, "").trim();
      if (isNameLike(k)) names.add(k);
    }
    const selfCall = (c.attrs && (c.attrs["自称"] || c.attrs.self_call)) || "";
    for (const part of String(selfCall).split(/[/／、,，|]/)) {
      const p = part.trim();
      if (isNameLike(p)) names.add(p);
    }
    for (const n of names) {
      terms.push({ term: n, id: c.id, entry: c });
    }
  }
  terms.sort(
    (a, b) =>
      b.term.length - a.term.length ||
      a.term.localeCompare(b.term, "zh")
  );
  return { terms, byId };
}

/**
 * 最长优先命中，返回互不重叠区间
 * @param {string} text
 * @param {Array<{term:string,id:string,entry:any}>} terms
 */
export function findNameHits(text, terms) {
  if (!text || !terms || !terms.length) return [];
  const occupied = new Array(text.length).fill(false);
  const hits = [];
  for (const t of terms) {
    const term = t.term;
    if (!term) continue;
    let from = 0;
    while (from <= text.length - term.length) {
      const i = text.indexOf(term, from);
      if (i < 0) break;
      if (isLatinTerm(term) && !hasLatinBoundary(text, i, term.length)) {
        from = i + 1;
        continue;
      }
      let free = true;
      for (let j = i; j < i + term.length; j++) {
        if (occupied[j]) {
          free = false;
          break;
        }
      }
      if (free) {
        for (let j = i; j < i + term.length; j++) occupied[j] = true;
        hits.push({
          start: i,
          end: i + term.length,
          term,
          id: t.id,
          entry: t.entry,
        });
      }
      from = i + Math.max(1, term.length);
    }
  }
  hits.sort((a, b) => a.start - b.start);
  return hits;
}

/**
 * 生成带角色高亮的 HTML（已转义）
 * @param {string} text
 * @param {Array<{term:string,id:string}>} terms
 */
export function highlightNamesHtml(text, terms) {
  const raw = String(text ?? "");
  if (!raw) return "";
  const hits = findNameHits(raw, terms);
  if (!hits.length) return escapeHtml(raw);
  let html = "";
  let cursor = 0;
  for (const h of hits) {
    if (h.start > cursor) html += escapeHtml(raw.slice(cursor, h.start));
    const slice = raw.slice(h.start, h.end);
    html += `<span class="char-hit" data-char-id="${escapeHtml(h.id)}" data-char-term="${escapeHtml(h.term)}">${escapeHtml(slice)}</span>`;
    cursor = h.end;
  }
  if (cursor < raw.length) html += escapeHtml(raw.slice(cursor));
  return html;
}

export function summaryForCard(entry, maxLen = 180) {
  if (!entry) return "";
  const body = String(entry.content || "").replace(/\s+/g, " ").trim();
  if (!body) return "暂无设定正文";
  if ([...body].length <= maxLen) return body;
  return [...body].slice(0, maxLen).join("") + "…";
}
