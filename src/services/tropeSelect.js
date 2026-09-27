/**
 * 本章勾选情节/喜好（写入 chapter.trope_ids，续写注入）
 * 代码路径: kk_novel_ai/src/services/tropeSelect.js
 */
import { appState } from "../stores/appState.js";
import { aiPanelForm } from "../stores/aiPanelState.js";
import { updateChapterMeta } from "./projectClient.js";
import { displayTropeTitle } from "../utils/tropeI18n.js";
import { tLocale } from "../i18n/index.js";

export function selectedTropeIdSet() {
  return new Set((aiPanelForm.selectedTropeIds || []).map(String));
}

export function isTropeSelected(id) {
  return selectedTropeIdSet().has(String(id || ""));
}

/** @returns {object[]} */
export function selectedTropeEntries() {
  const ids = selectedTropeIdSet();
  if (!ids.size) return [];
  return (appState.tropeList || []).filter((it) => it && ids.has(String(it.id || "")));
}

/**
 * 无创作提示时，用已选情节/喜好拼自由发挥种子
 * @param {string} [locale]
 * @returns {string}
 */
export function buildFreePlaySeedFromTropes(locale) {
  const items = selectedTropeEntries();
  if (!items.length) return "";
  const list = items
    .map((it) => {
      const title = displayTropeTitle(it) || String(it.title || "").trim();
      return title ? `- ${title}` : "";
    })
    .filter(Boolean)
    .join("\n");
  if (!list) return "";
  const loc = locale || appState.settings?.writing_locale || "zh-CN";
  return tLocale(loc, "ai.freePlayFromTropes", { list });
}

async function persist(ids) {
  if (!appState.projectRoot || !appState.chapterId) return;
  try {
    await updateChapterMeta(appState.chapterId, { trope_ids: [...ids] });
  } catch (e) {
    console.warn("[tropeSelect] persist", e);
  }
}

export function setTropeSelection(ids) {
  const next = [...new Set((ids || []).map(String).filter(Boolean))];
  aiPanelForm.selectedTropeIds = next;
  void persist(next);
}

export function toggleTropeSelection(id) {
  const key = String(id || "");
  if (!key) return;
  const ids = [...(aiPanelForm.selectedTropeIds || [])].map(String);
  const i = ids.indexOf(key);
  if (i >= 0) ids.splice(i, 1);
  else ids.push(key);
  setTropeSelection(ids);
}

export function clearTropeSelection() {
  setTropeSelection([]);
}
