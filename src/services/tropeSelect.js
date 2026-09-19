/**
 * 本章勾选情节/喜好（写入 chapter.trope_ids，续写注入）
 * 代码路径: kk_novel_ai/src/services/tropeSelect.js
 */
import { appState } from "../stores/appState.js";
import { aiPanelForm } from "../stores/aiPanelState.js";
import { updateChapterMeta } from "./projectClient.js";

export function selectedTropeIdSet() {
  return new Set((aiPanelForm.selectedTropeIds || []).map(String));
}

export function isTropeSelected(id) {
  return selectedTropeIdSet().has(String(id || ""));
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
