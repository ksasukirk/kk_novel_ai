/**
 * 情节/喜好卡 AI 优化写法
 * 代码路径: kk_novel_ai/src/services/tropeRefine.js
 */
import { invoke } from "./tauri.js";
import { appState, bumpTropeRevision } from "../stores/appState.js";
import { upsertLoreAt, ensureTropeLibrary } from "./projectClient.js";
import { refreshTropeIndex } from "./tropeIndex.js";
import { isTropeKind } from "../utils/tropeKinds.js";
import { formatTropeTags, parseTropeTags } from "../utils/tropeCategories.js";
import { t } from "../i18n/index.js";

const inFlight = new Set();

function stripJsonFence(raw) {
  let s = String(raw || "").trim();
  if (s.startsWith("```")) {
    s = s.replace(/^```(?:json)?\s*/i, "").replace(/\s*```$/i, "");
  }
  return s.trim();
}

function parseRefine(raw) {
  const s = stripJsonFence(raw);
  if (!s) return null;
  try {
    const obj = JSON.parse(s);
    const row = obj && Array.isArray(obj.tropes) && obj.tropes[0] ? obj.tropes[0] : obj;
    if (!row || typeof row !== "object") return null;
    const kind = String(row.kind || "").trim() === "kink" ? "kink" : "trope";
    const title = String(row.title || "").trim();
    const titleEn = String(row.title_en || "").trim();
    const content = String(row.content || "").trim();
    const contentEn = String(row.content_en || "").trim();
    const keywords = Array.isArray(row.keywords)
      ? row.keywords.map((k) => String(k || "").trim()).filter(Boolean)
      : [];
    const tags = parseTropeTags(row.tags || []);
    const doText = String(row.do || "").trim();
    const dontText = String(row.dont || "").trim();
    let intensity = String(row.intensity || "").trim();
    if (!/^[1-5]$/.test(intensity)) intensity = "";
    return { kind, title, titleEn, content, contentEn, keywords, tags, doText, dontText, intensity };
  } catch {
    return null;
  }
}

function cardPayload(item) {
  const attrs = (item && item.attrs && typeof item.attrs === "object") ? item.attrs : {};
  return {
    kind: item.kind === "kink" ? "kink" : "trope",
    title: item.title || "",
    title_en: attrs.title_en || "",
    content: item.content || "",
    content_en: attrs.content_en || "",
    keywords: item.keywords || [],
    do: attrs.do || "",
    dont: attrs.dont || "",
    tags: parseTropeTags(attrs.tags || ""),
    intensity: String(attrs.intensity || "3"),
  };
}

/**
 * @param {object} item lore 条目
 * @param {{ instruction?: string, projectRoot?: string }} [opts]
 * @returns {Promise<object|null>} 保存后的 item
 */
export async function runTropeRefine(item, opts = {}) {
  if (!item || !item.id) return null;
  if (inFlight.has(item.id)) return null;
  inFlight.add(item.id);
  const prevStatus = appState.statusMessage;
  appState.statusMessage = t("trope.refining", { title: item.title || item.id });
  try {
    let ensRoot = "";
    try {
      const ens = await ensureTropeLibrary();
      ensRoot = ens.root || "";
    } catch {
      /* ignore */
    }
    const libraryRoot = (opts && opts.libraryRoot) || ensRoot;
    const projectRoot =
      (opts && opts.projectRoot) || appState.projectRoot || libraryRoot;
    if (!projectRoot) throw new Error(t("trope.noPath"));
    const result = await invoke("writing_run", {
      request: {
        project_root: projectRoot,
        chapter_id: appState.chapterId || "",
        task: "trope_refine",
        selection: JSON.stringify(cardPayload(item)),
        instruction: (opts && opts.instruction) || "",
        block_key: `trope-refine-${item.id}`,
      },
    });
    const parsed = parseRefine(String((result && (result.text || result.raw_text)) || ""));
    if (!parsed || !parsed.content) {
      throw new Error(t("trope.refineEmpty"));
    }
    const attrs = { ...((item.attrs && typeof item.attrs === "object") ? item.attrs : {}) };
    if (parsed.intensity) attrs.intensity = parsed.intensity;
    if (parsed.doText) attrs.do = parsed.doText;
    if (parsed.dontText) attrs.dont = parsed.dontText;
    if (parsed.tags.length) attrs.tags = formatTropeTags(parsed.tags);
    if (parsed.titleEn) attrs.title_en = parsed.titleEn;
    if (parsed.contentEn) attrs.content_en = parsed.contentEn;
    const kind = isTropeKind(parsed.kind) ? parsed.kind : item.kind;
    const title =
      parsed.title && parsed.title.length <= 24 ? parsed.title : item.title;
    const keywords = parsed.keywords.length ? parsed.keywords : item.keywords || [];
    const root = item._root || libraryRoot;
    if (!root) throw new Error(t("trope.noPath"));
    const saved = await upsertLoreAt(root, {
      id: item.id,
      kind,
      title,
      content: parsed.content,
      keywords,
      links: item.links || [],
      attrs,
      unique: true,
      sources: item.sources || [],
      updated_at: "",
    });
    try {
      await refreshTropeIndex();
    } catch {
      /* ignore */
    }
    bumpTropeRevision();
    appState.statusMessage = t("trope.refineDone", {
      title: (saved && saved.item && saved.item.title) || title,
    });
    return (saved && saved.item) || null;
  } catch (e) {
    appState.statusMessage = t("trope.refineFailed", { msg: e.message || e });
    throw e;
  } finally {
    inFlight.delete(item.id);
    if (appState.statusMessage === t("trope.refining", { title: item.title || item.id })) {
      appState.statusMessage = prevStatus;
    }
  }
}
