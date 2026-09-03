/**
 * 作品 / 章节 / 设定 API
 * 代码路径: kk_novel_ai/src/services/projectClient.js
 */
import { invoke } from "./tauri.js";
import { appState } from "../stores/appState.js";
import { syncBookOutlineFromProject } from "../stores/aiPanelState.js";
import {
  blocksFromContent,
} from "../utils/genBlock.js";
import {
  activePathBlocks,
  branchDocForPersist,
  collapseChapterSectionsToWholeChapter,
  contentFromActivePath,
  isBranchDoc,
  migrateBlocksToBranchDoc,
  normalizeBranchDoc,
  parseSidecarToBranchDoc,
  syncDocFromBlocks,
} from "../utils/branchModel.js";

export async function pickDirectory() {
  return await invoke("pick_directory");
}

/** 选择含多个作品的父目录（批量导入用） */
export async function pickImportDirectory() {
  return await invoke("pick_import_directory");
}

/**
 * 扫描父目录下的 project.json，登记到最近作品/知识库列表（不切换当前打开）
 * @param {string} root
 * @param {{ maxDepth?: number }} [opts]
 */
export async function importProjectsFromDirectory(root, opts = {}) {
  const payload = { root };
  if (opts.maxDepth != null) payload.maxDepth = opts.maxDepth;
  return await invoke("project_import_directory", payload);
}

export async function createProject(root, title) {
  const r = await invoke("project_create", { root, title });
  applyProject(r);
  return r;
}

/** 默认在软件运行根/novels 下创建（重名文件夹自动加数字） */
export async function createProjectInNovels(title) {
  const r = await invoke("project_create_in_novels", { title });
  applyProject(r);
  return r;
}

export async function novelsDirInfo() {
  return await invoke("novels_dir_info");
}

export async function openProject(root) {
  const r = await invoke("project_open", { root });
  applyProject(r);
  await migrateLegacyChapterSections(root);
  return r;
}

export async function getProject(root) {
  const r = await invoke("project_get", { root });
  applyProject(r);
  return r;
}

function applyProject(r) {
  appState.projectRoot = r.root || "";
  appState.project = r.project || null;
  if (r.project && r.project.chapters && r.project.chapters.length) {
    if (!appState.chapterId || !r.project.chapters.some((c) => c.id === appState.chapterId)) {
      appState.chapterId = r.project.chapters[0].id;
    }
  } else {
    appState.chapterId = "";
    appState.chapterContent = "";
    appState.chapterBlocks = [];
    appState.chapterBranchDoc = null;
  }
  syncBookOutlineFromProject(r.project);
}

function branchDocFromChapterPayload(r) {
  const sidecar = r && r.blocks;
  if (
    isBranchDoc(sidecar) ||
    (sidecar && typeof sidecar === "object" && !Array.isArray(sidecar) && sidecar.nodes)
  ) {
    return parseSidecarToBranchDoc(sidecar);
  }
  if (Array.isArray(sidecar) && sidecar.length) {
    return migrateBlocksToBranchDoc(sidecar);
  }
  return migrateBlocksToBranchDoc(blocksFromContent(r.content || "", sidecar));
}

/** 打开作品时：把各章残留多小节合并为整章一块并落盘 */
export async function migrateLegacyChapterSections(root) {
  const projectRoot = root || appState.projectRoot;
  if (!projectRoot) return { migrated: 0 };
  let project = appState.project;
  if (!project || appState.projectRoot !== projectRoot) {
    const r = await invoke("project_get", { root: projectRoot });
    project = r.project;
  }
  const chapters = (project && project.chapters) || [];
  let migrated = 0;
  for (const ch of chapters) {
    if (!ch || !ch.id) continue;
    const cr = await invoke("chapter_read", {
      root: projectRoot,
      chapterId: ch.id,
    });
    const { doc: next, changed } = collapseChapterSectionsToWholeChapter(
      branchDocFromChapterPayload(cr)
    );
    if (!changed) continue;
    await invoke("chapter_write", {
      root: projectRoot,
      chapterId: ch.id,
      content: contentFromActivePath(next),
      blocks: branchDocForPersist(next),
    });
    migrated += 1;
    if (appState.chapterId === ch.id) {
      applyBranchDoc(next);
      appState.dirty = false;
    }
  }
  if (migrated > 0) {
    appState.statusMessage = `已自动合并 ${migrated} 章残留小节为整章`;
  }
  return { migrated };
}

/** 用分支文档刷新编辑器投影 */
export function applyBranchDoc(doc) {
  const next = normalizeBranchDoc(
    doc && isBranchDoc(doc) ? doc : migrateBlocksToBranchDoc(doc || [])
  );
  appState.chapterBranchDoc = next;
  appState.chapterBlocks = activePathBlocks(next);
  appState.chapterContent = contentFromActivePath(next);
  return next;
}

/** 把当前编辑器块写回分支文档（编辑正文后） */
export function syncBranchDocFromEditor() {
  if (!appState.chapterBranchDoc) {
    appState.chapterBranchDoc = migrateBlocksToBranchDoc(appState.chapterBlocks || []);
  } else {
    appState.chapterBranchDoc = syncDocFromBlocks(
      appState.chapterBranchDoc,
      appState.chapterBlocks || []
    );
  }
  return appState.chapterBranchDoc;
}

export async function loadChapter(chapterId) {
  if (!appState.projectRoot || !chapterId) return;
  const r = await invoke("chapter_read", {
    root: appState.projectRoot,
    chapterId,
  });
  appState.chapterId = chapterId;
  const { doc: next, changed } = collapseChapterSectionsToWholeChapter(
    branchDocFromChapterPayload(r)
  );
  applyBranchDoc(next);
  if (changed) {
    appState.dirty = true;
    await saveChapter();
    appState.statusMessage = "已自动合并本章残留小节为整章";
  } else {
    appState.dirty = false;
  }
  return r;
}

/** 只读某章块列表（不切换当前编辑章），供侧栏 TOC */
export async function peekChapterBlocks(chapterId) {
  if (!appState.projectRoot || !chapterId) return [];
  const r = await invoke("chapter_read", {
    root: appState.projectRoot,
    chapterId,
  });
  const { doc } = collapseChapterSectionsToWholeChapter(branchDocFromChapterPayload(r));
  return activePathBlocks(doc);
}

/** 只读某章分支文档 */
export async function peekChapterBranchDoc(chapterId) {
  if (!appState.projectRoot || !chapterId) return null;
  const r = await invoke("chapter_read", {
    root: appState.projectRoot,
    chapterId,
  });
  const { doc } = collapseChapterSectionsToWholeChapter(branchDocFromChapterPayload(r));
  return doc;
}

export async function saveChapter() {
  if (!appState.projectRoot || !appState.chapterId) return;
  syncBranchDocFromEditor();
  const doc = appState.chapterBranchDoc || migrateBlocksToBranchDoc(appState.chapterBlocks || []);
  const blocks = activePathBlocks(doc);
  const content = contentFromActivePath(doc);
  if (appState.chapterBlocks !== blocks) {
    appState.chapterBlocks = blocks;
  }
  if (appState.chapterContent !== content) {
    appState.chapterContent = content;
  }
  await invoke("chapter_write", {
    root: appState.projectRoot,
    chapterId: appState.chapterId,
    content,
    blocks: branchDocForPersist(doc),
  });
  appState.dirty = false;
  appState.statusMessage = "章节已保存";
}

export async function createChapter(title, summary = "", opts = {}) {
  const load = opts.load !== false;
  const r = await invoke("chapter_create", {
    root: appState.projectRoot,
    title,
    summary,
  });
  await getProject(appState.projectRoot);
  if (load && r.chapter) await loadChapter(r.chapter.id);
  return r;
}

export async function deleteChapter(chapterId) {
  await invoke("chapter_delete", { root: appState.projectRoot, chapterId });
  await getProject(appState.projectRoot);
}

export async function updateChapterMeta(chapterId, patch) {
  const body = {
    root: appState.projectRoot,
    chapterId,
    title: patch.title,
    summary: patch.summary,
    status: patch.status,
  };
  if (patch.patch) {
    body.patch = patch.patch;
  } else {
    // 允许直接传扩展字段
    const ext = {};
    for (const k of [
      "pov_lore_id",
      "focus_arc_ids",
      "must_do",
      "must_not",
      "reader_knows",
      "character_knows",
      "beats",
    ]) {
      if (patch[k] !== undefined) ext[k] = patch[k];
    }
    if (Object.keys(ext).length) body.patch = ext;
  }
  const r = await invoke("chapter_update_meta", body);
  await getProject(appState.projectRoot);
  return r;
}

export async function getBeatProgress(chapterId) {
  const r = await invoke("beat_progress_get", {
    root: appState.projectRoot,
    chapterId,
  });
  return (r && r.progress) || { current_beat_id: "", beats: {}, updated_at: "" };
}

export async function advanceBeatProgress(chapterId, beatId) {
  const r = await invoke("beat_progress_advance", {
    root: appState.projectRoot,
    chapterId,
    beatId,
  });
  return (r && r.progress) || null;
}

export async function resetBeatProgress(chapterId) {
  return await invoke("beat_progress_reset", {
    root: appState.projectRoot,
    chapterId,
  });
}

export async function skipBeatProgress(chapterId, beatId) {
  const r = await invoke("beat_progress_skip", {
    root: appState.projectRoot,
    chapterId,
    beatId,
  });
  return (r && r.progress) || null;
}

export async function saveProjectMeta(project) {
  await invoke("project_save_meta", { root: appState.projectRoot, project });
  appState.project = project;
  syncBookOutlineFromProject(project);
}

/** AI 根据大纲/章纲/正文建议书名（不写入） */
export async function suggestBookTitle(root) {
  return await invoke("project_suggest_title", { root });
}

/** 作品有效内容量；is_empty 与 AI 起书名阈值一致 */
export async function getContentSubstance(root) {
  return await invoke("project_content_substance", { root });
}

/** 写入书名并刷新最近列表；可选同步重命名作品文件夹（重名自动加数字） */
export async function applyBookTitle(root, title, { renameFolder = false } = {}) {
  const r = await invoke("project_apply_title", { root, title, renameFolder });
  if (r.settings) appState.settings = r.settings;
  const newRoot = (r && r.root) || root;
  const payload = r.project;
  if (payload && payload.project && appState.projectRoot === root) {
    appState.projectRoot = newRoot;
    appState.project = payload.project;
    syncBookOutlineFromProject(payload.project);
  }
  return r;
}

/** 从最近列表移除（不删磁盘） */
export async function forgetRecentProject(root) {
  const r = await invoke("project_forget_recent", { root });
  if (r.settings) appState.settings = r.settings;
  return r;
}

/** 从最近列表移除；purge 为 true 时删除含 project.json 的作品目录 */
export async function deleteProject(root, { purge = false } = {}) {
  return await invoke("project_delete", { root, purge });
}

export async function listLore() {
  return await invoke("lore_list", { root: appState.projectRoot });
}

export async function listLoreAt(root) {
  return await invoke("lore_list", { root });
}

export async function listLoreScoped() {
  return await invoke("lore_list_scoped", { root: appState.projectRoot });
}

export async function ensureCharactersLink() {
  const r = await invoke("project_ensure_characters_link", {
    root: appState.projectRoot,
  });
  if (r.project) {
    appState.project = r.project;
  }
  return r;
}

export async function ensureCharacterRoster() {
  return await invoke("character_roster_ensure");
}

export async function upsertLore(entry) {
  return await upsertLoreAt(appState.projectRoot, entry);
}

export async function upsertLoreAt(root, entry) {
  return await invoke("lore_upsert", { root, entry });
}

export async function deleteLore(loreId) {
  return await deleteLoreAt(appState.projectRoot, loreId);
}

export async function deleteLoreAt(root, loreId) {
  return await invoke("lore_delete", { root, loreId });
}

export async function exportTxt(output) {
  return await invoke("export_txt", { root: appState.projectRoot, output });
}

export async function exportPdf(output) {
  return await invoke("export_pdf", { root: appState.projectRoot, output });
}

export async function exportEpub(output) {
  return await invoke("export_epub", { root: appState.projectRoot, output });
}

export async function pickFile(title = "选择文件", extensions = ["txt", "md"]) {
  return await invoke("pick_file", { title, extensions });
}

export async function importTxt(root, file, title) {
  const r = await invoke("import_txt", { root, file, title });
  applyProject({
    root: r.root || root,
    project: null,
  });
  // 重新打开以加载完整 project
  return await openProject(r.root || root);
}

export async function importDistill(root, opts = {}) {
  return await invoke("import_distill", {
    root: root || appState.projectRoot,
    from: opts.from ?? 1,
    to: opts.to ?? 20,
    apply: opts.apply ?? "none",
    resume: opts.resume ?? false,
    jobId: opts.jobId ?? null,
    instruction: opts.instruction ?? "",
  });
}

export async function importApplyPending(root, jobId) {
  return await invoke("import_apply_pending", {
    root: root || appState.projectRoot,
    jobId,
  });
}

export async function loadGenLogs(limit = 50) {
  const r = await invoke("gen_log_list", { limit });
  appState.genLogs = r.items || [];
  return r.items;
}

export async function loadUsageSummary(root = null) {
  const r = await invoke("usage_summary", { root: root || null });
  appState.usageSummary = r;
  return r;
}

/** 导出作品 ZIP 备份到应用缓存，返回 path/filename/bytes */
export async function exportProjectBackup(root = appState.projectRoot) {
  return await invoke("project_export_backup", { root });
}

/** 从 base64 ZIP 导入作品 */
export async function importProjectBackupBase64(dataB64, title = null) {
  const r = await invoke("project_import_backup_base64", {
    dataB64,
    title,
  });
  if (r.root) applyProject(r);
  return r;
}

/** 读取导出缓存文件为 base64，供浏览器下载/分享 */
export async function readExportFileBase64(path) {
  return await invoke("export_file_read_base64", { path });
}

export async function platformInfo() {
  return await invoke("platform_info");
}

/** 把 base64 触发为浏览器下载（手机侧载分享入口） */
export function downloadBase64File(filename, base64, mime = "application/octet-stream") {
  const bin = atob(base64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  const blob = new Blob([bytes], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename || "download.bin";
  a.rel = "noopener";
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 2000);
}

/** 读本地 File 为 base64（无 data: 前缀） */
export function fileToBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result || "");
      const idx = result.indexOf(",");
      resolve(idx >= 0 ? result.slice(idx + 1) : result);
    };
    reader.onerror = () => reject(reader.error || new Error("读取文件失败"));
    reader.readAsDataURL(file);
  });
}
