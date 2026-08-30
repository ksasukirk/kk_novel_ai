/**
 * 知识库 API
 * 代码路径: kk_novel_ai/src/services/kbClient.js
 */
import { invoke } from "./tauri.js";
import { appState, snapshotWritingIfNeeded, isKbProject } from "../stores/appState.js";
import * as project from "./projectClient.js";

export async function listRegistry() {
  return await invoke("kb_registry_list");
}

function applyKb(r) {
  snapshotWritingIfNeeded();
  appState.projectRoot = r.root || "";
  appState.project = r.project || null;
  appState.activeNav = "knowledge";
  if (r.project && r.project.chapters && r.project.chapters.length) {
    appState.chapterId = r.project.chapters[0].id;
  } else {
    appState.chapterId = "";
    appState.chapterContent = "";
  }
  appState.dirty = false;
}

export async function openUniversal() {
  const r = await invoke("kb_universal_open");
  applyKb(r);
  return r;
}

export async function openKnowledgeBase(root) {
  const r = await invoke("project_open", { root });
  applyKb(r);
  return r;
}

export async function syncKb(root) {
  return await invoke("kb_sync", { root: root || appState.projectRoot });
}

export async function syncAll() {
  return await invoke("kb_sync_all");
}

export async function rebuildUniversalRag() {
  return await invoke("kb_universal_rebuild_rag");
}

export async function migrateKb(root, opts = {}) {
  return await invoke("kb_migrate", {
    root,
    sourceFile: opts.sourceFile ?? null,
    sync: opts.sync ?? true,
  });
}

export async function importIntoKb(root, file, title) {
  snapshotWritingIfNeeded();
  const r = await invoke("import_txt", { root, file, title });
  const opened = await invoke("project_open", { root: r.root || root });
  applyKb(opened);
  appState.kbSubNav = "home";
  return opened;
}

export function kbIsUniversal() {
  return appState.project && appState.project.kind === "universal";
}

export function kbIsSingleBook() {
  return appState.project && appState.project.kind === "knowledge_base";
}

export { isKbProject };
