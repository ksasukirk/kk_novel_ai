/**
 * Novel OS 总谱 API
 * 代码路径: kk_novel_ai/src/services/storyClient.js
 */
import { invoke } from "./tauri.js";
import { appState } from "../stores/appState.js";

export async function getPlot() {
  return await invoke("story_plot_get", { root: appState.projectRoot });
}

export async function savePlot(plot) {
  return await invoke("story_plot_save", { root: appState.projectRoot, plot });
}

export async function getTimeline() {
  return await invoke("story_timeline_get", { root: appState.projectRoot });
}

export async function saveTimeline(timeline) {
  return await invoke("story_timeline_save", { root: appState.projectRoot, timeline });
}

export async function getRelations() {
  return await invoke("story_relations_get", { root: appState.projectRoot });
}

export async function saveRelations(relations) {
  return await invoke("story_relations_save", { root: appState.projectRoot, relations });
}

export async function getCanon() {
  return await invoke("story_canon_get", { root: appState.projectRoot });
}

export async function saveCanon(canon) {
  return await invoke("story_canon_save", { root: appState.projectRoot, canon });
}

export async function applyStoryPatch(patch) {
  return await invoke("story_apply_patch", { root: appState.projectRoot, patch });
}

export async function getDashboard() {
  return await invoke("story_dashboard", { root: appState.projectRoot });
}

export async function getStoryboard() {
  return await invoke("story_storyboard_get", { root: appState.projectRoot });
}

export async function saveStoryboard(storyboard) {
  return await invoke("story_storyboard_save", {
    root: appState.projectRoot,
    storyboard,
  });
}

export function newId() {
  if (typeof crypto !== "undefined" && crypto.randomUUID) return crypto.randomUUID();
  return `id-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}
