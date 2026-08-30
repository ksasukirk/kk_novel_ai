/**
 * 生成写入后自动抽取新人物 → 本篇角色 lore
 * 代码路径: kk_novel_ai/src/services/castExtract.js
 */
import { invoke } from "./tauri.js";
import { appState } from "../stores/appState.js";
import { upsertLoreAt } from "./projectClient.js";
import { refreshCharacterNameIndex } from "./characterIndex.js";

const inFlightKeys = new Set();

function autoCastEnabled() {
  const s = appState.settings;
  if (!s) return true;
  if (s.writing_auto_cast === false) return false;
  return true;
}

function normalizeName(s) {
  return String(s || "")
    .trim()
    .replace(/^\[.*?\]\s*/, "")
    .toLowerCase();
}

function collectKnownKeys() {
  const keys = new Set();
  const add = (s) => {
    const n = normalizeName(s);
    if (n) keys.add(n);
  };
  for (const t of appState.characterNameTerms || []) {
    if (t && t.term) add(t.term);
  }
  for (const e of appState.characterList || []) {
    if (!e || e.kind !== "character") continue;
    add(e.title);
    for (const k of e.keywords || []) {
      const raw = String(k || "").trim();
      if (!raw) continue;
      if (raw.startsWith("alias:")) add(raw.slice(6));
      else if (!raw.includes("=")) add(raw);
    }
  }
  return keys;
}

function stripJsonFence(raw) {
  let s = String(raw || "").trim();
  if (s.startsWith("```")) {
    s = s.replace(/^```(?:json)?\s*/i, "").replace(/\s*```$/i, "");
  }
  return s.trim();
}

function parseCharacters(raw) {
  const s = stripJsonFence(raw);
  if (!s) return [];
  try {
    const obj = JSON.parse(s);
    const list = Array.isArray(obj)
      ? obj
      : Array.isArray(obj.characters)
        ? obj.characters
        : [];
    return list
      .map((c) => ({
        title: String((c && c.title) || "").trim(),
        aliases: Array.isArray(c && c.aliases)
          ? c.aliases.map((a) => String(a || "").trim()).filter(Boolean)
          : [],
        content: String((c && c.content) || "").trim(),
      }))
      .filter((c) => c.title && c.title.length <= 12);
  } catch {
    return [];
  }
}

/**
 * @param {{ blockKey?: string, text: string, instruction?: string }} opts
 * @returns {Promise<string[]|null>} 新加入的角色名
 */
export async function runCastExtract(opts) {
  const blockKey = (opts && opts.blockKey) || `cast-${Date.now()}`;
  const text = ((opts && opts.text) || "").trim();
  if (!text || !appState.projectRoot || !appState.chapterId) return null;
  if (!autoCastEnabled()) return null;
  if (inFlightKeys.has(blockKey)) return null;

  inFlightKeys.add(blockKey);
  const prevStatus = appState.statusMessage;
  appState.statusMessage = "正在识别新人物…";
  try {
    try {
      await refreshCharacterNameIndex();
    } catch {
      /* 索引失败仍可抽 */
    }
    const known = collectKnownKeys();
    const result = await invoke("writing_run", {
      request: {
        project_root: appState.projectRoot,
        chapter_id: appState.chapterId,
        task: "cast_extract",
        selection: text,
        instruction: (opts && opts.instruction) || "",
        block_key: blockKey,
      },
    });
    const raw = String((result && (result.text || result.raw_text)) || "");
    const candidates = parseCharacters(raw);
    const added = [];
    for (const c of candidates.slice(0, 5)) {
      if (known.has(normalizeName(c.title))) continue;
      if (c.aliases.some((a) => known.has(normalizeName(a)))) continue;
      const keywords = [c.title, ...c.aliases.map((a) => `alias:${a}`)];
      const content =
        c.content || `${c.title}（生成块自动添加，待补设定）`;
      try {
        await upsertLoreAt(appState.projectRoot, {
          id: "",
          kind: "character",
          title: c.title,
          content,
          keywords,
          links: [],
          attrs: { source: "cast_extract" },
          unique: true,
          sources: [],
          updated_at: "",
        });
        added.push(c.title);
        known.add(normalizeName(c.title));
        for (const a of c.aliases) known.add(normalizeName(a));
      } catch {
        /* 单条失败继续 */
      }
    }
    if (added.length) {
      try {
        await refreshCharacterNameIndex();
      } catch {
        /* ignore */
      }
      appState.castRevision = (Number(appState.castRevision) || 0) + 1;
      appState.statusMessage = `已自动添加本篇角色：${added.join("、")}`;
    } else {
      appState.statusMessage = prevStatus || "无新人物需添加";
    }
    return added;
  } catch (e) {
    appState.statusMessage = `新人物识别失败（不影响正文）：${e.message || e}`;
    return null;
  } finally {
    inFlightKeys.delete(blockKey);
  }
}
