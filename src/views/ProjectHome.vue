<!--
  作品首页：网格卡片 + 新建 +
  代码路径: kk_novel_ai/src/views/ProjectHome.vue
-->
<script setup>
import { computed, onMounted, reactive, ref, watch } from "vue";
import { appState, isKbProject } from "../stores/appState.js";
import * as project from "../services/projectClient.js";
import { loadSettings } from "../services/llmClient.js";
import { invoke } from "../services/tauri.js";
import { getDashboard } from "../services/storyClient.js";
import * as kb from "../services/kbClient.js";
import { appConfirm, appConfirmDelete } from "../services/confirmDialog.js";
import { createBackdropDismiss } from "../utils/backdropDismiss.js";
import { isMobileUx } from "../utils/platform.js";
import { useToastError } from "../services/toast.js";

const title = ref("未命名小说");
const error = useToastError();
const stats = ref(null);
const goalInput = ref(2000);
const dash = ref(null);
const showCreate = ref(false);
const creating = ref(false);
const novelsDirHint = ref("");
const mobileUx = ref(isMobileUx());
const backupInput = ref(null);
/** path -> 正在 AI 生成书名 */
const titleBusy = reactive({});
/** 多选模式 */
const selectMode = ref(false);
const selectedPaths = ref([]);
const bulkBusy = ref(false);
const createBackdrop = createBackdropDismiss(() => {
  showCreate.value = false;
});

const recentList = computed(() => {
  const list = (appState.settings && appState.settings.recent_projects) || [];
  return Array.isArray(list) ? list : [];
});

const selectedCount = computed(() => selectedPaths.value.length);

const allSelected = computed(
  () => recentList.value.length > 0 && selectedCount.value === recentList.value.length
);

function isSelected(path) {
  return selectedPaths.value.includes(path);
}

function toggleSelectMode() {
  selectMode.value = !selectMode.value;
  if (!selectMode.value) selectedPaths.value = [];
}

function toggleSelect(path) {
  if (isSelected(path)) {
    selectedPaths.value = selectedPaths.value.filter((p) => p !== path);
  } else {
    selectedPaths.value = [...selectedPaths.value, path];
  }
}

function selectAllRecent() {
  selectedPaths.value = recentList.value.map((item) => item.path);
}

function clearSelection() {
  selectedPaths.value = [];
}

function onCardClick(item) {
  if (selectMode.value) {
    toggleSelect(item.path);
    return;
  }
  openByPath(item.path);
}

function clearActiveProjectIfNeeded(paths) {
  if (!paths.includes(appState.projectRoot)) return;
  appState.projectRoot = "";
  appState.project = null;
  appState.chapterId = "";
  appState.chapterContent = "";
  dash.value = null;
  stats.value = null;
}

async function refreshNovelsHint() {
  try {
    const r = await project.novelsDirInfo();
    novelsDirHint.value = r.novels_dir || "";
  } catch {
    novelsDirHint.value = "";
  }
}

async function refreshSettings() {
  try {
    await loadSettings();
  } catch {
    /* ignore */
  }
}

async function openCreateDialog() {
  showCreate.value = true;
  await refreshNovelsHint();
}

async function onCreateConfirm() {
  error.value = "";
  creating.value = true;
  try {
    await project.createProjectInNovels(title.value.trim() || "未命名小说");
    await refreshSettings();
    await refreshStats();
    showCreate.value = false;
    appState.activeNav = "editor";
    if (appState.chapterId) await project.loadChapter(appState.chapterId);
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    creating.value = false;
  }
}

/** 仍支持自选目录创建（高级） */
async function onCreateInPickedDir() {
  error.value = "";
  creating.value = true;
  try {
    const picked = await project.pickDirectory();
    await project.createProject(picked.path, title.value.trim() || "未命名小说");
    await refreshSettings();
    await refreshStats();
    showCreate.value = false;
    appState.activeNav = "editor";
    if (appState.chapterId) await project.loadChapter(appState.chapterId);
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    creating.value = false;
  }
}

async function refreshStats() {
  if (!appState.projectRoot) {
    stats.value = null;
    dash.value = null;
    return;
  }
  try {
    const r = await invoke("stats_get", { root: appState.projectRoot });
    stats.value = r.stats || null;
    if (stats.value && stats.value.goal_chars) goalInput.value = stats.value.goal_chars;
  } catch {
    stats.value = null;
  }
  try {
    dash.value = await getDashboard();
  } catch {
    dash.value = null;
  }
  try {
    await project.loadUsageSummary(appState.projectRoot);
  } catch {
    /* ignore */
  }
}

watch(() => appState.projectRoot, refreshStats, { immediate: true });
onMounted(refreshSettings);

function isActive(path) {
  return path && appState.projectRoot && path === appState.projectRoot;
}

function shortPath(path) {
  if (!path) return "";
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  if (parts.length <= 2) return parts.join("/") || path;
  return parts.slice(-2).join("/");
}

async function openByPath(path) {
  error.value = "";
  try {
    const r = await project.openProject(path);
    if (r.project && isKbProject(r.project)) {
      // 知识库目录：交给知识库页，不占用写作导航
      await kb.openKnowledgeBase(path);
      appState.kbSubNav = "home";
      appState.activeNav = "knowledge";
      appState.statusMessage = "已转到知识库视图";
      return;
    }
    await refreshSettings();
    await refreshStats();
    appState.activeNav = "editor";
    if (appState.chapterId) await project.loadChapter(appState.chapterId);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onBrowseOpen() {
  error.value = "";
  try {
    const picked = await project.pickDirectory();
    await openByPath(picked.path);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

/** 选父目录，检测并批量导入其下所有作品到最近列表 */
async function onImportDirectoryProjects() {
  error.value = "";
  try {
    const picked = await project.pickImportDirectory();
    const parent = picked && picked.path;
    if (!parent) throw new Error("未选择目录");
    appState.statusMessage = `正在扫描「${parent}」…`;
    const r = await project.importProjectsFromDirectory(parent, { maxDepth: 2 });
    if (r.settings) appState.settings = r.settings;
    else await refreshSettings();
    const msg =
      r.message ||
      `已导入写作 ${r.imported_novels || 0}、知识库 ${r.imported_knowledge || 0}`;
    appState.statusMessage = msg;
    if ((r.found || 0) === 0) {
      error.value = msg;
    } else if (Array.isArray(r.failed) && r.failed.length) {
      error.value = `${msg}；失败 ${r.failed.length} 个`;
    }
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onForget(path, ev) {
  ev.stopPropagation();
  error.value = "";
  if (
    !(await appConfirmDelete("从最近列表移除该作品？", {
      title: "移除作品",
    }))
  ) {
    return;
  }
  try {
    await project.forgetRecentProject(path);
    clearActiveProjectIfNeeded([path]);
    selectedPaths.value = selectedPaths.value.filter((p) => p !== path);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

/** 批量 AI 生成书名并直接应用 */
async function onBulkSuggestTitles() {
  const paths = selectedPaths.value.slice();
  if (!paths.length || bulkBusy.value) return;
  error.value = "";
  const ok = await appConfirm(
    `为已选 ${paths.length} 部作品 AI 生成书名并应用？\n文件夹名不会改动；内容过少或知识库会跳过。`,
    {
      title: "批量 AI 生成书名",
      confirmText: "开始",
      cancelText: "取消",
    }
  );
  if (!ok) return;
  bulkBusy.value = true;
  let okN = 0;
  const failed = [];
  try {
    for (const path of paths) {
      titleBusy[path] = true;
      try {
        const r = await project.suggestBookTitle(path);
        const next = (r && r.title) || "";
        if (!next) throw new Error("未生成书名");
        await project.applyBookTitle(path, next);
        okN += 1;
      } catch (e) {
        const item = recentList.value.find((x) => x.path === path);
        failed.push(`${item && item.title ? item.title : shortPath(path)}: ${e.message || e}`);
      } finally {
        titleBusy[path] = false;
      }
    }
    await refreshSettings();
    const tail = failed.length ? `；失败 ${failed.length} 个` : "";
    appState.statusMessage = `批量书名：成功 ${okN}${tail}`;
    if (failed.length) error.value = failed.slice(0, 5).join("\n");
  } finally {
    bulkBusy.value = false;
  }
}

/** 批量从最近列表移除 */
async function onBulkForget() {
  const paths = selectedPaths.value.slice();
  if (!paths.length || bulkBusy.value) return;
  error.value = "";
  if (
    !(await appConfirmDelete(`从最近列表移除已选 ${paths.length} 部作品？`, {
      title: "批量移除",
      confirmText: "移除",
    }))
  ) {
    return;
  }
  bulkBusy.value = true;
  try {
    for (const path of paths) {
      await project.forgetRecentProject(path);
    }
    clearActiveProjectIfNeeded(paths);
    selectedPaths.value = [];
    appState.statusMessage = `已从列表移除 ${paths.length} 部作品`;
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    bulkBusy.value = false;
  }
}

/** 批量彻底删除作品目录 */
async function onBulkPurge() {
  const paths = selectedPaths.value.slice();
  if (!paths.length || bulkBusy.value) return;
  error.value = "";
  if (
    !(await appConfirmDelete(
      `彻底删除已选 ${paths.length} 部作品目录？\n磁盘文件不可恢复；无 project.json 或受保护路径会跳过。`,
      {
        title: "批量彻底删除",
        confirmText: "彻底删除",
      }
    ))
  ) {
    return;
  }
  bulkBusy.value = true;
  let purged = 0;
  const failed = [];
  try {
    for (const path of paths) {
      try {
        const r = await project.deleteProject(path, { purge: true });
        if (r && r.purged) purged += 1;
      } catch (e) {
        const item = recentList.value.find((x) => x.path === path);
        failed.push(`${item && item.title ? item.title : shortPath(path)}: ${e.message || e}`);
      }
    }
    clearActiveProjectIfNeeded(paths);
    await refreshSettings();
    selectedPaths.value = selectedPaths.value.filter((p) => !paths.includes(p));
    const tail = failed.length ? `；失败 ${failed.length} 个` : "";
    appState.statusMessage = `已彻底删除 ${purged} 部作品${tail}`;
    if (failed.length) error.value = failed.slice(0, 5).join("\n");
  } finally {
    bulkBusy.value = false;
  }
}

const EMPTY_CONTENT_MSG = "内容太少，请先写全书大纲、章纲或正文再生成书名";

/** 空内容作品：提示是否彻底删除 */
async function offerPurgeEmptyProject(item, path) {
  const name = (item && item.title) || "未命名小说";
  const purge = await appConfirmDelete(
    `「${name}」几乎没有任何大纲或正文，无法 AI 生成书名。\n\n是否彻底删除该作品目录？不可恢复。`,
    {
      title: "空内容作品",
      confirmText: "彻底删除",
    }
  );
  if (!purge) {
    error.value = EMPTY_CONTENT_MSG;
    return;
  }
  await project.deleteProject(path, { purge: true });
  clearActiveProjectIfNeeded([path]);
  selectedPaths.value = selectedPaths.value.filter((p) => p !== path);
  await refreshSettings();
  appState.statusMessage = `已彻底删除空内容作品：${name}`;
}

/** AI 根据内容生成书名，确认后写入 */
async function onSuggestTitle(item, ev) {
  ev.stopPropagation();
  const path = item && item.path;
  if (!path || titleBusy[path]) return;
  error.value = "";
  titleBusy[path] = true;
  try {
    const check = await project.getContentSubstance(path);
    if (check && check.is_empty) {
      await offerPurgeEmptyProject(item, path);
      return;
    }
    const r = await project.suggestBookTitle(path);
    const next = (r && r.title) || "";
    if (!next) throw new Error("未生成书名");
    const prev = (r && r.previous_title) || item.title || "未命名小说";
    const choice = await appConfirm(
      `建议书名：「${next}」\n当前：「${prev}」\n\n「应用」只改书名；「应用并重命名文件夹」会同步改目录名（重名自动加 2、3…）。`,
      {
        title: "AI 生成书名",
        confirmText: "应用",
        cancelText: "不用",
        extraText: "应用并重命名文件夹",
      }
    );
    if (!choice) return;
    const renameFolder = choice === "extra";
    const applied = await project.applyBookTitle(path, next, { renameFolder });
    await refreshSettings();
    if (renameFolder && applied && applied.folder_renamed) {
      const folder = applied.folder_name || next;
      selectedPaths.value = selectedPaths.value.map((p) => (p === path ? applied.root : p));
      appState.statusMessage = `已更新书名并重命名文件夹：${folder}`;
    } else if (renameFolder && applied && !applied.folder_renamed) {
      appState.statusMessage = `已更新书名（文件夹已是「${applied.folder_name || next}」）`;
    } else {
      appState.statusMessage = `已更新书名：${next}`;
    }
  } catch (e) {
    const msg = String(e.message || e);
    if (msg.includes("内容太少")) {
      await offerPurgeEmptyProject(item, path);
      return;
    }
    error.value = msg;
  } finally {
    titleBusy[path] = false;
  }
}

async function onSaveGoal() {
  if (!appState.projectRoot) return;
  try {
    await invoke("stats_set_goal", {
      root: appState.projectRoot,
      goalChars: Number(goalInput.value) || 2000,
    });
    await refreshStats();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onExportTxt() {
  error.value = "";
  if (!appState.projectRoot) {
    error.value = "请先打开作品";
    return;
  }
  try {
    const picked = await project.pickDirectory();
    const stem = String((appState.project && appState.project.title) || "novel")
      .replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_")
      .trim() || "novel";
    const out = `${picked.path}\\${stem}.txt`;
    await project.exportTxt(out);
    appState.statusMessage = `已导出 TXT：${out}`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onExportPdf() {
  error.value = "";
  if (!appState.projectRoot) {
    error.value = "请先打开作品";
    return;
  }
  try {
    const picked = await project.pickDirectory();
    const stem = String((appState.project && appState.project.title) || "novel")
      .replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_")
      .trim() || "novel";
    const out = `${picked.path}\\${stem}.pdf`;
    await project.exportPdf(out);
    appState.statusMessage = `已导出 PDF：${out}`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onExportEpub() {
  error.value = "";
  if (!appState.projectRoot) {
    error.value = "请先打开作品";
    return;
  }
  try {
    const picked = await project.pickDirectory();
    const stem = String((appState.project && appState.project.title) || "novel")
      .replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_")
      .trim() || "novel";
    const out = `${picked.path}\\${stem}.epub`;
    await project.exportEpub(out);
    appState.statusMessage = `已导出 EPUB：${out}`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onExportBackup() {
  error.value = "";
  if (!appState.projectRoot) {
    error.value = "请先打开作品";
    return;
  }
  try {
    const r = await project.exportProjectBackup(appState.projectRoot);
    const file = await project.readExportFileBase64(r.path);
    project.downloadBase64File(file.filename || r.filename, file.base64, "application/zip");
    appState.statusMessage = `已导出备份：${file.filename || r.filename}`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onImportBackupPick() {
  if (backupInput.value) backupInput.value.click();
}

async function onImportBackupFile(ev) {
  error.value = "";
  const file = ev.target && ev.target.files && ev.target.files[0];
  if (ev.target) ev.target.value = "";
  if (!file) return;
  try {
    const b64 = await project.fileToBase64(file);
    const r = await project.importProjectBackupBase64(b64, null);
    await refreshSettings();
    await refreshStats();
    appState.activeNav = "editor";
    if (appState.chapterId) await project.loadChapter(appState.chapterId);
    appState.statusMessage = `已导入：${(r.project && r.project.title) || "作品"}`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

const todayKey = computed(() => {
  const d = new Date();
  const m = `${d.getMonth() + 1}`.padStart(2, "0");
  const day = `${d.getDate()}`.padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
});

const todayChars = computed(() => {
  if (!stats.value || !stats.value.daily) return 0;
  return stats.value.daily[todayKey.value] || 0;
});

const goal = computed(() => (stats.value && stats.value.goal_chars) || goalInput.value || 2000);

const progressPct = computed(() => {
  const g = goal.value || 1;
  return Math.min(100, Math.round((todayChars.value / g) * 100));
});

const heatDays = computed(() => {
  const daily = (stats.value && stats.value.daily) || {};
  const out = [];
  const now = new Date();
  for (let i = 83; i >= 0; i--) {
    const d = new Date(now);
    d.setDate(now.getDate() - i);
    const key = `${d.getFullYear()}-${`${d.getMonth() + 1}`.padStart(2, "0")}-${`${d.getDate()}`.padStart(2, "0")}`;
    const n = daily[key] || 0;
    let level = 0;
    if (n > 0) level = 1;
    if (n >= 500) level = 2;
    if (n >= 1500) level = 3;
    if (n >= 3000) level = 4;
    out.push({ key, n, level });
  }
  return out;
});
</script>

<template>
  <section class="panel">
    <div class="page-head">
      <div>
        <h1 class="panel-heading">作品</h1>
        <p class="muted">以卡片打开写作作品；点「+」新建。可用「导入目录下作品」批量登记；正文导入请到「知识库」。</p>
      </div>
      <div class="head-actions">
        <button
          v-if="!mobileUx"
          type="button"
          class="app-btn"
          @click="onBrowseOpen"
        >
          打开其它目录
        </button>
        <button
          v-if="!mobileUx"
          type="button"
          class="app-btn"
          title="选择父目录，自动发现并登记其下所有含 project.json 的作品"
          @click="onImportDirectoryProjects"
        >
          导入目录下作品
        </button>
        <button type="button" class="app-btn" @click="onImportBackupPick">导入备份</button>
        <button
          type="button"
          class="app-btn"
          :disabled="!appState.projectRoot"
          @click="onExportBackup"
        >
          导出备份
        </button>
        <button
          v-if="!mobileUx"
          type="button"
          class="app-btn"
          @click="appState.activeNav = 'knowledge'"
        >
          去知识库导入
        </button>
        <button
          v-if="!mobileUx"
          type="button"
          class="app-btn"
          :disabled="!appState.projectRoot"
          @click="onExportTxt"
        >
          导出 TXT
        </button>
        <button
          v-if="!mobileUx"
          type="button"
          class="app-btn"
          :disabled="!appState.projectRoot"
          @click="onExportPdf"
        >
          导出 PDF
        </button>
        <button
          v-if="!mobileUx"
          type="button"
          class="app-btn"
          :disabled="!appState.projectRoot"
          @click="onExportEpub"
        >
          导出 EPUB
        </button>
        <input
          ref="backupInput"
          type="file"
          accept=".zip,application/zip"
          class="hidden-file"
          @change="onImportBackupFile"
        />
      </div>
    </div>

    <div v-if="recentList.length" class="select-bar">
      <button
        type="button"
        class="app-btn"
        :class="{ 'app-btn-primary': selectMode }"
        @click="toggleSelectMode"
      >
        {{ selectMode ? "完成多选" : "多选" }}
      </button>
      <template v-if="selectMode">
        <span class="select-hint muted">
          已选 {{ selectedCount }} / {{ recentList.length }}
        </span>
        <button
          type="button"
          class="app-btn"
          :disabled="bulkBusy || !recentList.length"
          @click="allSelected ? clearSelection() : selectAllRecent()"
        >
          {{ allSelected ? "取消全选" : "全选" }}
        </button>
        <button
          type="button"
          class="app-btn"
          :disabled="bulkBusy || selectedCount < 1"
          @click="onBulkSuggestTitles"
        >
          {{ bulkBusy ? "处理中…" : "AI 生成名称" }}
        </button>
        <button
          type="button"
          class="app-btn"
          :disabled="bulkBusy || selectedCount < 1"
          @click="onBulkForget"
        >
          从列表移除
        </button>
        <button
          type="button"
          class="app-btn app-btn-danger"
          :disabled="bulkBusy || selectedCount < 1"
          @click="onBulkPurge"
        >
          彻底删除
        </button>
      </template>
    </div>

    <div class="work-grid">
      <button type="button" class="work-bar work-bar-add" @click="openCreateDialog">
        <span class="plus" aria-hidden="true">+</span>
        <span class="add-label">新建作品</span>
      </button>

      <button
        v-for="item in recentList"
        :key="item.path"
        type="button"
        class="work-bar"
        :class="{
          active: isActive(item.path),
          selected: selectMode && isSelected(item.path),
        }"
        :title="item.path"
        @click="onCardClick(item)"
      >
        <span
          v-if="selectMode"
          class="bar-check"
          :class="{ on: isSelected(item.path) }"
          aria-hidden="true"
        />
        <div class="bar-body">
          <div class="bar-head">
            <span class="row-badge">小说</span>
            <span v-if="isActive(item.path)" class="row-active-tag">当前</span>
            <div v-if="!selectMode" class="row-actions" @click.stop>
              <span
                class="card-ai-title"
                :class="{ busy: titleBusy[item.path] }"
                title="AI 根据内容重新生成书名"
                @click="onSuggestTitle(item, $event)"
              >{{ titleBusy[item.path] ? "…" : "AI" }}</span>
              <span
                class="card-forget"
                title="从列表移除"
                @click="onForget(item.path, $event)"
              >×</span>
            </div>
          </div>
          <span class="row-title">{{ item.title || "未命名小说" }}</span>
          <span class="row-path muted">{{ shortPath(item.path) }}</span>
        </div>
      </button>
    </div>

    <div
      v-if="showCreate"
      class="create-mask"
      @mousedown="createBackdrop.onMouseDown"
      @click="createBackdrop.onClick"
    >
      <div class="create-dialog">
        <h2>新建作品</h2>
        <p class="muted">
          默认创建在软件运行目录下的
          <code>novels</code>
          文件夹；每本书单独一夹，重名自动加数字（如「书名2」）。
        </p>
        <p v-if="novelsDirHint" class="muted novels-hint" :title="novelsDirHint">
          当前路径：{{ novelsDirHint }}
        </p>
        <div class="field">
          <label class="field-label">新书书名</label>
          <input v-model="title" type="text" placeholder="书名" @keydown.enter.prevent="onCreateConfirm" />
        </div>
        <div class="dialog-actions">
          <button type="button" class="app-btn" @click="showCreate = false">取消</button>
          <button
            type="button"
            class="app-btn"
            :disabled="creating"
            title="自行选择空目录创建"
            @click="onCreateInPickedDir"
          >
            自选目录…
          </button>
          <button
            type="button"
            class="app-btn app-btn-primary"
            :disabled="creating"
            @click="onCreateConfirm"
          >
            {{ creating ? "创建中…" : "创建" }}
          </button>
        </div>
      </div>
    </div>

    <div v-if="appState.projectRoot && dash" class="stats-block">
      <h2 class="sub-head">叙事仪表盘</h2>
      <p class="muted">
        当前故事日：{{ dash.current_story_time || "（无）" }} ·
        未回收承诺 {{ dash.open_promises ?? 0 }} ·
        锁定 Canon {{ dash.locked_canon ?? 0 }} ·
        关系边 {{ dash.edge_count ?? 0 }} ·
        事件 {{ dash.event_count ?? 0 }}
      </p>
      <p v-if="dash.main_arc" class="muted">
        主线：{{ dash.main_arc.title }}（{{ dash.main_arc.status }}）— {{ dash.main_arc.progress_note || "无进度备注" }}
      </p>
      <ul v-if="dash.active_arcs && dash.active_arcs.length" class="arc-list">
        <li v-for="a in dash.active_arcs" :key="a.id">
          [{{ a.kind }}] {{ a.title }} · {{ a.status }}
        </li>
      </ul>
      <button type="button" class="app-btn" @click="appState.activeNav = 'story'">打开总谱</button>
    </div>

    <div v-if="appState.projectRoot" class="stats-block">
      <h2 class="sub-head">码字看板</h2>
      <p v-if="appState.usageSummary && appState.usageSummary.global" class="muted">
        Token 累计：全局
        {{
          appState.usageSummary.global.total_tokens ||
          (appState.usageSummary.global.prompt_tokens || 0) +
            (appState.usageSummary.global.completion_tokens || 0)
        }}
        tok · ¥{{ Number(appState.usageSummary.global.cost_cny || 0).toFixed(4) }}
        <span v-if="appState.usageSummary.project">
          · 本作品
          {{
            (appState.usageSummary.project.prompt_tokens || 0) +
            (appState.usageSummary.project.completion_tokens || 0)
          }}
          tok
        </span>
      </p>
      <p class="muted">今日 {{ todayChars }} / 目标 {{ goal }} 字（{{ progressPct }}%）</p>
      <div class="bar">
        <div class="bar-fill" :style="{ width: progressPct + '%' }" />
      </div>
      <div class="goal-row">
        <input v-model.number="goalInput" type="number" min="100" step="100" />
        <button type="button" class="app-btn" @click="onSaveGoal">保存日目标</button>
        <button type="button" class="app-btn" @click="refreshStats">刷新统计</button>
      </div>
      <div class="heat" title="近 12 周码字热力">
        <div
          v-for="d in heatDays"
          :key="d.key"
          class="heat-cell"
          :class="'lv' + d.level"
          :title="`${d.key}: ${d.n} 字`"
        />
      </div>
    </div>

  </section>
</template>

<style scoped>
.panel {
  min-height: calc(100% - 8px);
}
.page-head {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 16px;
}
.head-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.select-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}
.select-hint {
  font-size: 12px;
}
.hidden-file {
  display: none;
}
.work-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 10px;
}
.work-bar {
  min-height: 72px;
  max-height: 88px;
  padding: 10px 12px;
  border: none;
  border-radius: var(--radius-md);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
  text-align: left;
  cursor: pointer;
  display: flex;
  flex-direction: row;
  align-items: stretch;
  gap: 8px;
  color: var(--text);
  transition: box-shadow 0.15s ease, background 0.15s ease, transform 0.15s ease;
}
.work-bar.selected {
  box-shadow: 0 0 0 2px var(--accent), var(--shadow-sm);
  background: var(--accent-soft);
}
.bar-check {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  margin-top: 2px;
  border-radius: 4px;
  border: 1.5px solid color-mix(in srgb, var(--muted) 55%, transparent);
  background: var(--panel);
  position: relative;
}
.bar-check.on {
  border-color: var(--accent);
  background: var(--accent);
}
.bar-check.on::after {
  content: "";
  position: absolute;
  left: 5px;
  top: 2px;
  width: 5px;
  height: 9px;
  border: solid #fff;
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}
.work-bar:hover {
  transform: translateY(-1px);
  box-shadow: var(--shadow);
  background: var(--accent-soft);
}
.work-bar.active {
  box-shadow: 0 0 0 2px var(--accent), var(--shadow-sm);
}
.work-bar-add {
  flex-direction: row;
  align-items: center;
  justify-content: center;
  gap: 8px;
  background: var(--panel-2);
  border: 1.5px dashed color-mix(in srgb, var(--accent) 45%, transparent);
  box-shadow: none;
}
.work-bar-add:hover {
  background: var(--accent-soft);
  border-style: solid;
  transform: none;
}
.bar-body {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  flex: 1;
}
.bar-head {
  display: flex;
  align-items: center;
  gap: 6px;
}
.plus {
  font-size: 22px;
  font-weight: 300;
  line-height: 1;
  color: var(--accent-hover);
}
.add-label {
  font-size: 13px;
  font-weight: 650;
  color: var(--muted);
}
.row-badge {
  flex-shrink: 0;
  font-size: 10px;
  font-weight: 700;
  color: var(--accent-hover);
  background: var(--accent-soft);
  border-radius: 999px;
  padding: 1px 7px;
}
.row-title {
  font-size: 14px;
  font-weight: 750;
  line-height: 1.25;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.row-path {
  font-size: 11px;
  line-height: 1.3;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.row-active-tag {
  flex-shrink: 0;
  font-size: 10px;
  font-weight: 700;
  color: var(--accent-hover);
}
.row-actions {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
  margin-left: auto;
}
.card-ai-title {
  min-width: 22px;
  height: 22px;
  padding: 0 5px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--accent-hover);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.02em;
  line-height: 1;
}
.card-ai-title:hover {
  background: var(--accent-soft);
}
.card-ai-title.busy {
  opacity: 0.65;
  pointer-events: none;
}
.card-forget {
  width: 22px;
  height: 22px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--muted);
  font-size: 16px;
  line-height: 1;
}
.card-forget:hover {
  background: rgba(0, 0, 0, 0.06);
  color: var(--error);
}
.create-mask {
  position: fixed;
  inset: 0;
  z-index: 80;
  background: rgba(20, 16, 24, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}
.create-dialog {
  width: min(420px, 100%);
  padding: 22px;
  border-radius: var(--radius-lg);
  background: var(--panel);
  box-shadow: var(--shadow);
}
.create-dialog h2 {
  margin: 0 0 8px;
  font-size: 17px;
}
.novels-hint {
  font-size: 12px;
  margin: 0 0 10px;
  word-break: break-all;
}
.create-dialog code {
  font-size: 12px;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--chip-bg, #f0f0f0);
}
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 14px;
}
.stats-block {
  margin-top: 22px;
  padding: 16px 18px;
  border-radius: var(--radius-lg);
  background: var(--panel-2);
  box-shadow: var(--shadow-sm);
}
.sub-head {
  font-size: 15px;
  margin: 0 0 8px;
}
.bar {
  height: 10px;
  border-radius: 999px;
  background: var(--accent-soft);
  overflow: hidden;
  margin: 8px 0 12px;
}
.bar-fill {
  height: 100%;
  background: var(--accent);
  border-radius: 999px;
  transition: width 0.2s ease;
  box-shadow: var(--shadow-nav);
}
.goal-row {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.goal-row input {
  width: 120px;
}
.arc-list {
  margin: 8px 0 12px;
  padding-left: 18px;
  color: var(--muted);
  font-size: 13px;
}
.heat {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  grid-auto-flow: column;
  grid-template-rows: repeat(12, 12px);
  gap: 3px;
  max-width: 220px;
}
.heat-cell {
  border-radius: 3px;
  background: var(--chip-bg);
  box-shadow: inset 0 0 0 1px var(--divider);
}
.heat-cell.lv1 { background: rgba(244, 63, 94, 0.25); }
.heat-cell.lv2 { background: rgba(244, 63, 94, 0.45); }
.heat-cell.lv3 { background: rgba(244, 63, 94, 0.7); }
.heat-cell.lv4 { background: rgba(244, 63, 94, 0.95); }
.error {
  color: var(--error);
}
</style>
