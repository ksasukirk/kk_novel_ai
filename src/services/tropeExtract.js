/**
 * 生成写入后自动抽取情节/性癖 → 全局库
 * 代码路径: kk_novel_ai/src/services/tropeExtract.js
 */
import { invoke } from "./tauri.js";
import { appState, bumpTropeRevision } from "../stores/appState.js";
import { upsertLoreAt, ensureCharacterRoster } from "./projectClient.js";
import { refreshTropeIndex } from "./tropeIndex.js";
import { isTropeKind } from "../utils/tropeKinds.js";
import { t } from "../i18n/index.js";

const inFlightKeys = new Set();

function autoTropeEnabled() {
  const s = appState.settings;
  if (!s) return true;
  if (s.writing_auto_trope === false) return false;
  return true;
}

function normalizeName(s) {
  return String(s || "")
    .trim()
    .replace(/^\[.*?\]\s*/, "")
    .toLowerCase();
}

function stripJsonFence(raw) {
  let s = String(raw || "").trim();
  if (s.startsWith("```")) {
    s = s.replace(/^```(?:json)?\s*/i, "").replace(/\s*```$/i, "");
  }
  return s.trim();
}

function parseTropes(raw) {
  const s = stripJsonFence(raw);
  if (!s) return [];
  try {
    const obj = JSON.parse(s);
    const list = Array.isArray(obj)
      ? obj
      : Array.isArray(obj.tropes)
        ? obj.tropes
        : [];
    return list
      .map((row) => ({
        kind: String((row && row.kind) || "trope").trim() === "kink" ? "kink" : "trope",
        title: String((row && (row.matched_title || row.title)) || "").trim(),
        content: String((row && row.content) || "").trim(),
        keywords: Array.isArray(row && row.keywords)
          ? row.keywords.map((k) => String(k || "").trim()).filter(Boolean)
          : [],
        evidence: String((row && row.evidence) || "").trim(),
        matched: Boolean(row && String(row.matched_title || "").trim()),
      }))
      .filter((x) => x.title && x.title.length <= 24 && isTropeKind(x.kind));
  } catch {
    return [];
  }
}

function findExisting(title) {
  const key = normalizeName(title);
  if (!key) return null;
  for (const e of appState.tropeList || []) {
    if (normalizeName(e.title) === key) return e;
    for (const k of e.keywords || []) {
      if (normalizeName(k) === key) return e;
    }
  }
  return null;
}

/**
 * @param {{ blockKey?: string, text: string, instruction?: string, blockType?: string }} opts
 * @returns {Promise<string[]|null>}
 */
export async function runTropeExtract(opts) {
  const blockKey = (opts && opts.blockKey) || `trope-${Date.now()}`;
  const text = ((opts && opts.text) || "").trim();
  const projectRoot = (opts && opts.projectRoot) || appState.projectRoot;
  const chapterId = (opts && opts.chapterId) || appState.chapterId;
  const force = !!(opts && opts.force);
  const quiet = !!(opts && opts.quiet);
  if (!text || !projectRoot || !chapterId) return null;
  if (opts && (opts.blockType === "illustration" || opts.blockType === "illus")) {
    return null;
  }
  if (!force && !autoTropeEnabled()) return null;
  if (inFlightKeys.has(blockKey)) return null;

  inFlightKeys.add(blockKey);
  const prevStatus = appState.statusMessage;
  if (!quiet) {
    appState.statusMessage = t("autoTrope.recognizing");
  }
  try {
    try {
      await refreshTropeIndex();
    } catch {
      /* 索引失败仍可抽 */
    }
    const result = await invoke("writing_run", {
      request: {
        project_root: projectRoot,
        chapter_id: chapterId,
        task: "trope_extract",
        selection: text,
        instruction: (opts && opts.instruction) || "",
        block_key: blockKey,
      },
    });
    const raw = String((result && (result.text || result.raw_text)) || "");
    const candidates = parseTropes(raw);
    const added = [];
    const updated = [];
    let rosterRoot = "";
    for (const row of candidates.slice(0, 5)) {
      const existing = findExisting(row.title);
      const keywords = [...(existing?.keywords || [])];
      for (const k of [row.title, ...row.keywords]) {
        if (k && !keywords.includes(k)) keywords.push(k);
      }
      const attrs = { ...(existing?.attrs || {}) };
      if (row.evidence) {
        const prev = String(attrs.evidence || "").trim();
        attrs.evidence = prev && !prev.includes(row.evidence)
          ? `${prev}; ${row.evidence}`
          : prev || row.evidence;
      }
      let content = existing?.content || "";
      if (row.content) {
        content = content && !content.includes(row.content)
          ? `${content}\n${row.content}`
          : content || row.content;
      }
      try {
        let root = existing?._root || "";
        if (!root) {
          if (!rosterRoot) {
            const ens = await ensureCharacterRoster();
            rosterRoot = ens.root || "";
          }
          root = rosterRoot;
        }
        if (!root) continue;
        await upsertLoreAt(root, {
          id: existing?.id || "",
          kind: existing?.kind || row.kind,
          title: existing?.title || row.title,
          content,
          keywords,
          links: existing?.links || [],
          attrs,
          unique: true,
          sources: existing?.sources || [],
          updated_at: "",
        });
        if (existing) updated.push(existing.title || row.title);
        else added.push(row.title);
      } catch {
        /* 单条失败继续 */
      }
    }
    if (added.length || updated.length) {
      try {
        await refreshTropeIndex();
      } catch {
        /* ignore */
      }
      bumpTropeRevision();
      if (!quiet) {
        if (added.length) {
          appState.statusMessage = t("autoTrope.added", {
            names: added.join(t("common.listSep")),
          });
        } else {
          appState.statusMessage = t("autoTrope.updated", {
            names: updated.join(t("common.listSep")),
          });
        }
      }
    } else if (!quiet) {
      appState.statusMessage = prevStatus || t("autoTrope.none");
    }
    return added;
  } catch (e) {
    if (!quiet) {
      appState.statusMessage = t("autoTrope.failed", { msg: e.message || e });
    }
    return null;
  } finally {
    inFlightKeys.delete(blockKey);
  }
}
