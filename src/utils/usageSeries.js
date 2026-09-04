/**
 * 用量日志按日 / 按维聚合（分析页图表）
 * 代码路径: kk_novel_ai/src/utils/usageSeries.js
 */

import { usageTotalTokens } from "./usageFormat.js";

function dayKey(ts) {
  const s = String(ts || "");
  if (s.length >= 10) return s.slice(0, 10);
  return "";
}

function emptyDay(date) {
  return {
    date,
    cost: 0,
    tokens: 0,
    prompt: 0,
    completion: 0,
    calls: 0,
  };
}

function bumpAgg(row, item) {
  row.cost += Number(item.cost_cny) || 0;
  const u = item.usage || {};
  row.prompt += u.prompt_tokens || 0;
  row.completion += u.completion_tokens || 0;
  row.tokens += usageTotalTokens(u);
  row.hit += u.prompt_cache_hit_tokens || 0;
  row.miss += u.prompt_cache_miss_tokens || 0;
  row.calls += 1;
  const ts = String(item.ts || "");
  if (ts && (!row.lastTs || ts > row.lastTs)) row.lastTs = ts;
  if (ts && (!row.firstTs || ts < row.firstTs)) row.firstTs = ts;
}

function finalizeAgg(row) {
  row.hitRate =
    row.hit + row.miss > 0 ? Math.round((row.hit / (row.hit + row.miss)) * 100) : null;
  return row;
}

/** 近 days 天按日桶；无点则填 0 */
export function buildDailySeries(logs, days = 14) {
  const map = new Map();
  const today = new Date();
  today.setHours(12, 0, 0, 0);
  const keys = [];
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(today);
    d.setDate(d.getDate() - i);
    const key = d.toISOString().slice(0, 10);
    keys.push(key);
    map.set(key, emptyDay(key));
  }
  for (const item of logs || []) {
    const k = dayKey(item.ts);
    if (!k || !map.has(k)) continue;
    const row = map.get(k);
    row.cost += Number(item.cost_cny) || 0;
    const u = item.usage || {};
    row.prompt += u.prompt_tokens || 0;
    row.completion += u.completion_tokens || 0;
    row.tokens += usageTotalTokens(u);
    row.calls += 1;
  }
  return keys.map((k) => map.get(k));
}

export function aggregateByModel(logs) {
  const map = new Map();
  for (const item of logs || []) {
    const name = String(item.model_used || "").trim() || "(unknown)";
    let row = map.get(name);
    if (!row) {
      row = { name, cost: 0, tokens: 0, calls: 0 };
      map.set(name, row);
    }
    row.cost += Number(item.cost_cny) || 0;
    row.tokens += usageTotalTokens(item.usage);
    row.calls += 1;
  }
  return [...map.values()].sort((a, b) => b.cost - a.cost || b.tokens - a.tokens);
}

export function aggregateByTask(logs) {
  const map = new Map();
  for (const item of logs || []) {
    const name = String(item.task || "").trim() || "(unknown)";
    let row = map.get(name);
    if (!row) {
      row = { name, cost: 0, tokens: 0, calls: 0 };
      map.set(name, row);
    }
    row.cost += Number(item.cost_cny) || 0;
    row.tokens += usageTotalTokens(item.usage);
    row.calls += 1;
  }
  return [...map.values()].sort((a, b) => b.calls - a.calls || b.cost - a.cost);
}

/** 按作品根目录汇总整体统计 */
export function aggregateByProject(logs) {
  const map = new Map();
  for (const item of logs || []) {
    const root = String(item.project_root || "").trim() || "(none)";
    let row = map.get(root);
    if (!row) {
      row = {
        root,
        cost: 0,
        tokens: 0,
        prompt: 0,
        completion: 0,
        calls: 0,
        hit: 0,
        miss: 0,
        hitRate: null,
        chapters: new Set(),
        lastTs: "",
        firstTs: "",
      };
      map.set(root, row);
    }
    bumpAgg(row, item);
    const ch = String(item.chapter_id || "").trim();
    if (ch && ch !== "_project") row.chapters.add(ch);
  }
  return [...map.values()]
    .map((row) => {
      const chapterCount = row.chapters.size;
      delete row.chapters;
      return finalizeAgg({ ...row, chapterCount });
    })
    .sort((a, b) => String(b.lastTs).localeCompare(String(a.lastTs)));
}

/** 按章节汇总（同一作品内） */
export function aggregateByChapter(logs) {
  const map = new Map();
  for (const item of logs || []) {
    const raw = String(item.chapter_id || "").trim();
    const id = raw || "_project";
    let row = map.get(id);
    if (!row) {
      row = {
        chapterId: id,
        label: id === "_project" ? "作品级（无章节）" : id,
        cost: 0,
        tokens: 0,
        prompt: 0,
        completion: 0,
        calls: 0,
        hit: 0,
        miss: 0,
        hitRate: null,
        lastTs: "",
        firstTs: "",
      };
      map.set(id, row);
    }
    bumpAgg(row, item);
  }
  return [...map.values()]
    .map(finalizeAgg)
    .sort((a, b) => String(b.lastTs).localeCompare(String(a.lastTs)));
}

/** 从当前可见日志汇总 KPI */
export function summarizeLogs(logs) {
  let calls = 0;
  let cost = 0;
  let tokens = 0;
  let hit = 0;
  let miss = 0;
  for (const item of logs || []) {
    calls += 1;
    cost += Number(item.cost_cny) || 0;
    const u = item.usage || {};
    tokens += usageTotalTokens(u);
    hit += u.prompt_cache_hit_tokens || 0;
    miss += u.prompt_cache_miss_tokens || 0;
  }
  const hitRate = hit + miss > 0 ? Math.round((hit / (hit + miss)) * 100) : null;
  return { calls, cost, tokens, hit, miss, hitRate };
}
