/**
 * 生成写入后自动增量同步总谱（故事线 / 时间线 / 关系 / Canon）
 * 代码路径: kk_novel_ai/src/services/storySync.js
 */
import { invoke } from "./tauri.js";
import { reactive } from "vue";
import { appState } from "../stores/appState.js";
import {
  applyStoryPatch,
  getCanon,
  getTimeline,
  saveCanon,
  savePlot,
  saveRelations,
  saveTimeline,
} from "./storyClient.js";
import { refreshCharacterNameIndex } from "./characterIndex.js";
import { peekChapterBranchDoc } from "./projectClient.js";
import { contentFromActivePath } from "../utils/branchModel.js";
import { isChapterBodyEmpty } from "../utils/chapterStatus.js";

const inFlightKeys = new Set();

function autoStorySyncEnabled() {
  const s = appState.settings;
  if (!s) return true;
  if (s.writing_auto_story_sync === false) return false;
  return true;
}

function stripJsonFence(raw) {
  let s = String(raw || "").trim();
  if (s.startsWith("```")) {
    s = s.replace(/^```(?:json)?\s*/i, "").replace(/\s*```$/i, "");
  }
  return s.trim();
}

function looksLikeId(s) {
  const t = String(s || "").trim();
  if (!t) return false;
  if (/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(t)) {
    return true;
  }
  return t.length >= 12 && !/[\u4e00-\u9fff]/.test(t);
}

function buildNameIndex() {
  const byId = { ...(appState.characterById || {}) };
  const byTitle = new Map();
  const addTitle = (title, id) => {
    const t = String(title || "").trim();
    if (!t || !id) return;
    const key = t.toLowerCase();
    if (!byTitle.has(key)) byTitle.set(key, id);
  };
  for (const e of appState.characterList || []) {
    if (!e || !e.id) continue;
    byId[e.id] = e;
    addTitle(e.title, e.id);
    for (const k of e.keywords || []) {
      const raw = String(k || "").trim();
      if (raw.startsWith("alias:")) addTitle(raw.slice(6), e.id);
    }
  }
  for (const t of appState.characterNameTerms || []) {
    if (t && t.term && t.id) addTitle(t.term, t.id);
  }
  return { byId, byTitle };
}

export function resolveEndpoint(raw, index) {
  const s = String(raw || "").trim();
  if (!s) return "";
  if (index.byId[s]) return s;
  const hit = index.byTitle.get(s.toLowerCase());
  if (hit) return hit;
  const stripped = s.replace(/(表哥|表妹|姐姐|哥哥|叔叔|阿姨)$/u, "");
  if (stripped && stripped !== s) {
    const h2 = index.byTitle.get(stripped.toLowerCase());
    if (h2) return h2;
  }
  if (looksLikeId(s)) return s;
  return "";
}

function parsePatch(raw) {
  const s = stripJsonFence(raw);
  if (!s || s === "{}" || s === "[]") return {};
  try {
    const obj = JSON.parse(s);
    return obj && typeof obj === "object" && !Array.isArray(obj) ? obj : {};
  } catch {
    return null;
  }
}

function nonemptyArray(v) {
  return Array.isArray(v) && v.length > 0 ? v : null;
}

/**
 * 洗净模型 patch：人名对成 lore id，丢掉无头边，不覆盖锁定 Canon。
 * @param {object} patch
 * @param {{ lockedFactIds?: Set<string>, index?: ReturnType<typeof buildNameIndex>, chapterId?: string }} opts
 */
export function sanitizeStoryPatch(patch, opts = {}) {
  if (!patch || typeof patch !== "object") return {};
  const index = opts.index || buildNameIndex();
  const locked = opts.lockedFactIds || new Set();
  const chapterId = opts.chapterId || appState.chapterId || "";
  const out = {};

  const arcs = nonemptyArray(patch.arcs);
  if (arcs) {
    const next = arcs
      .map((a) => ({
        id: String((a && a.id) || ""),
        kind: String((a && a.kind) || "sub"),
        title: String((a && a.title) || "").trim(),
        goal: String((a && a.goal) || ""),
        status: String((a && a.status) || "active"),
        progress_note: String((a && a.progress_note) || ""),
        related_lore_ids: Array.isArray(a && a.related_lore_ids) ? a.related_lore_ids : [],
      }))
      .filter((a) => a.title);
    if (next.length) out.arcs = next;
  }

  const promises = nonemptyArray(patch.promises);
  if (promises) {
    const next = promises
      .map((p) => ({
        id: String((p && p.id) || ""),
        text: String((p && p.text) || "").trim(),
        status: String((p && p.status) || "open"),
        planted_chapter_id: (p && p.planted_chapter_id) || chapterId || null,
        arc_id: (p && p.arc_id) || null,
      }))
      .filter((p) => p.text);
    if (next.length) out.promises = next;
  }

  const events = nonemptyArray(patch.events);
  if (events) {
    const next = events
      .map((e) => {
        const participants = Array.isArray(e && e.participant_lore_ids)
          ? e.participant_lore_ids.map((id) => resolveEndpoint(id, index)).filter(Boolean)
          : [];
        const chs = Array.isArray(e && e.chapter_ids) ? e.chapter_ids.filter(Boolean) : [];
        if (chapterId && !chs.length) chs.push(chapterId);
        return {
          id: String((e && e.id) || ""),
          story_time: String((e && e.story_time) || "").trim(),
          title: String((e && e.title) || "").trim(),
          summary: String((e && e.summary) || "").trim(),
          location: String((e && e.location) || ""),
          chapter_ids: chs,
          participant_lore_ids: participants,
        };
      })
      .filter((e) => e.title || e.summary);
    if (next.length) out.events = next;
  }

  const edges = nonemptyArray(patch.edges);
  if (edges) {
    const next = edges
      .map((e) => {
        const from_id = resolveEndpoint((e && (e.from_id || e.from)) || "", index);
        const to_id = resolveEndpoint((e && (e.to_id || e.to)) || "", index);
        if (!from_id || !to_id || from_id === to_id) return null;
        const str = Number(e && e.strength);
        return {
          id: String((e && e.id) || ""),
          from_id,
          to_id,
          kind: String((e && e.kind) || "related") || "related",
          label: String((e && e.label) || ""),
          strength: Number.isFinite(str) ? Math.min(5, Math.max(1, str)) : 3,
          public: e && e.public === false ? false : true,
        };
      })
      .filter(Boolean);
    if (next.length) out.edges = next;
  }

  const facts = nonemptyArray(patch.facts);
  if (facts) {
    const next = facts
      .map((f) => {
        const id = String((f && f.id) || "");
        if (id && locked.has(id)) return null;
        const text = String((f && f.text) || "").trim();
        if (!text) return null;
        const evidence = Array.isArray(f && f.evidence_chapter_ids)
          ? f.evidence_chapter_ids.filter(Boolean)
          : [];
        if (chapterId && !evidence.length) evidence.push(chapterId);
        return {
          id,
          text,
          locked: !!(f && f.locked),
          evidence_chapter_ids: evidence,
          related_lore_ids: Array.isArray(f && f.related_lore_ids) ? f.related_lore_ids : [],
          tags: Array.isArray(f && f.tags) ? f.tags : [],
        };
      })
      .filter(Boolean);
    if (next.length) out.facts = next;
  }

  return out;
}

function patchHasFields(patch) {
  return ["arcs", "promises", "events", "edges", "facts"].some(
    (k) => Array.isArray(patch[k]) && patch[k].length
  );
}

/**
 * @param {{ blockKey?: string, text: string, instruction?: string, chapterId?: string, force?: boolean, quiet?: boolean, bumpRevision?: boolean, branchContextText?: string }} opts
 * @returns {Promise<string[]|null>} 已更新的字段名
 */
export async function runStorySync(opts) {
  const blockKey = (opts && opts.blockKey) || `story-sync-${Date.now()}`;
  const text = ((opts && opts.text) || "").trim();
  const chapterId = (opts && opts.chapterId) || appState.chapterId;
  const quiet = !!(opts && opts.quiet);
  const bumpRevision = opts && opts.bumpRevision === false ? false : true;
  if (!text || !appState.projectRoot || !chapterId) return null;
  if (!(opts && opts.force) && !autoStorySyncEnabled()) return null;
  if (inFlightKeys.has(blockKey)) return null;

  inFlightKeys.add(blockKey);
  const prevStatus = appState.statusMessage;
  if (!quiet) appState.statusMessage = "正在同步总谱…";
  try {
    try {
      await refreshCharacterNameIndex();
    } catch {
      /* 索引失败仍可同步 */
    }

    const request = {
      project_root: appState.projectRoot,
      chapter_id: chapterId,
      task: "story_sync",
      selection: text,
      instruction: (opts && opts.instruction) || "",
      block_key: blockKey,
    };
    const branchText = opts && opts.branchContextText;
    if (branchText && String(branchText).trim()) {
      request.branch_context_text = String(branchText);
    }
    const result = await invoke("writing_run", {
      request,
    });
    const raw = String((result && (result.text || result.raw_text)) || "");
    const parsed = parsePatch(raw);
    if (parsed == null) {
      if (!quiet) appState.statusMessage = "总谱同步未解析到 JSON（不影响正文）";
      return null;
    }

    let lockedFactIds = new Set();
    try {
      const c = await getCanon();
      const facts = (c && c.canon && c.canon.facts) || [];
      for (const f of facts) {
        if (f && f.id && f.locked) lockedFactIds.add(f.id);
      }
    } catch {
      lockedFactIds = new Set();
    }

    const clean = sanitizeStoryPatch(parsed, {
      lockedFactIds,
      index: buildNameIndex(),
      chapterId,
    });
    if (!patchHasFields(clean)) {
      if (!quiet) appState.statusMessage = prevStatus || "总谱无新条目";
      return [];
    }

    const applied = await applyStoryPatch(clean);
    const updated = Array.isArray(applied && applied.updated) ? applied.updated : [];
    if (bumpRevision) {
      appState.storyRevision = (Number(appState.storyRevision) || 0) + 1;
    }
    if (!quiet) {
      appState.statusMessage = updated.length
        ? `已同步总谱：${updated.join("、")}`
        : "已同步总谱";
    }
    return updated;
  } catch (e) {
    const msg = String((e && e.message) || e || "");
    if (/无可识别字段/.test(msg)) {
      if (!quiet) appState.statusMessage = prevStatus || "总谱无新条目";
      return [];
    }
    if (!quiet) appState.statusMessage = `总谱同步失败（不影响正文）：${msg}`;
    return null;
  } finally {
    inFlightKeys.delete(blockKey);
  }
}

export const storyRebuildState = reactive({
  running: false,
  cancelled: false,
  index: 0,
  total: 0,
  chapterTitle: "",
  ok: 0,
  failed: [],
});

export function cancelStoryRebuild() {
  storyRebuildState.cancelled = true;
  void import("./llmClient.js")
    .then((m) => m.cancelGeneration())
    .catch(() => {});
}

export async function resetUnlockedStoryStores() {
  let calendar = "";
  try {
    const t = await getTimeline();
    calendar = String((t && t.timeline && t.timeline.calendar_note) || "");
  } catch {
    calendar = "";
  }
  let locked = [];
  try {
    const c = await getCanon();
    locked = ((c && c.canon && c.canon.facts) || []).filter((f) => f && f.locked);
  } catch {
    locked = [];
  }
  await savePlot({ arcs: [], promises: [] });
  await saveTimeline({ calendar_note: calendar, events: [] });
  await saveRelations({ edges: [] });
  await saveCanon({ facts: locked });
}

/**
 * 按已有章节正文逐章重建总谱（故事线 / 时间线 / 关系 / 未锁定 Canon）。
 * 不改章节正文；本章焦点与节拍不在此重跑。
 */
export async function rebuildStoryFromExistingWork() {
  if (storyRebuildState.running) {
    throw new Error("总谱重建正在进行");
  }
  storyRebuildState.running = true;
  storyRebuildState.cancelled = false;
  storyRebuildState.index = 0;
  storyRebuildState.total = 0;
  storyRebuildState.chapterTitle = "";
  storyRebuildState.ok = 0;
  storyRebuildState.failed = [];

  try {
    const chapters = ((appState.project && appState.project.chapters) || []).filter(
      (c) => c && c.id
    );
    if (!chapters.length) throw new Error("作品还没有章节");

    const targets = [];
    for (const ch of chapters) {
      let text = "";
      if (ch.id === appState.chapterId) {
        const live = appState.chapterBranchDoc;
        text = contentFromActivePath(live || { nodes: [], plains: [] });
        if (!String(text || "").trim()) text = appState.chapterContent || "";
      } else {
        const doc = await peekChapterBranchDoc(ch.id);
        text = contentFromActivePath(doc || { nodes: [], plains: [] });
      }
      if (isChapterBodyEmpty(text, ch.title)) continue;
      targets.push({ ch, text: String(text || "").trim() });
    }
    if (!targets.length) throw new Error("没有可读取的章节正文");

    storyRebuildState.total = targets.length;
    await resetUnlockedStoryStores();
    appState.storyRevision = (Number(appState.storyRevision) || 0) + 1;
    try {
      await refreshCharacterNameIndex();
    } catch {
      /* ignore */
    }

    for (let i = 0; i < targets.length; i++) {
      if (storyRebuildState.cancelled) {
        appState.statusMessage = `总谱重建已取消（已完成 ${storyRebuildState.ok}/${targets.length} 章）`;
        break;
      }
      const { ch, text } = targets[i];
      storyRebuildState.index = i + 1;
      storyRebuildState.chapterTitle = ch.title || `第${i + 1}章`;
      appState.statusMessage = `正在按正文重建总谱 ${i + 1}/${targets.length} · ${storyRebuildState.chapterTitle}`;
      const updated = await runStorySync({
        blockKey: `rebuild:${ch.id}`,
        text,
        chapterId: ch.id,
        force: true,
        quiet: true,
        bumpRevision: false,
        branchContextText: text,
        instruction:
          "根据本章已有正文重建总谱增量；只写本章新出现的情节、时间、关系和事实；不要编造本章没写的事。",
      });
      if (storyRebuildState.cancelled) {
        appState.statusMessage = `总谱重建已取消（已完成 ${storyRebuildState.ok}/${targets.length} 章）`;
        break;
      }
      if (updated == null) {
        storyRebuildState.failed.push(storyRebuildState.chapterTitle);
      } else {
        storyRebuildState.ok += 1;
      }
    }

    appState.storyRevision = (Number(appState.storyRevision) || 0) + 1;
    if (storyRebuildState.cancelled) {
      return { ...storyRebuildState, cancelled: true };
    }
    const failN = storyRebuildState.failed.length;
    appState.statusMessage = failN
      ? `总谱已按正文重建：成功 ${storyRebuildState.ok} 章，失败 ${failN} 章（${storyRebuildState.failed.join("、")}）`
      : `总谱已按正文重建：${storyRebuildState.ok} 章`;
    return { ...storyRebuildState, cancelled: false };
  } finally {
    storyRebuildState.running = false;
  }
}
