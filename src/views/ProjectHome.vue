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

const title = ref("未命名小说");
const error = ref("");
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
const createBackdrop = createBackdropDismiss(() => {
  showCreate.value = false;
});

const recentList = computed(() => {
  const list = (appState.settings && appState.settings.recent_projects) || [];
  return Array.isArray(list) ? list : [];
});

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
  const parts = path.replace(/\\/g, "/").split("/");
  return parts.slice(-2).join("/") || path;
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
    const r = await invoke("project_forget_recent", { root: path });
    if (r.settings) appState.settings = r.settings;
    else await refreshSettings();
    if (appState.projectRoot === path) {
      appState.projectRoot = "";
      appState.project = null;
      appState.chapterId = "";
      appState.chapterContent = "";
      dash.value = null;
      stats.value = null;
    }
  } catch (e) {
    error.value = String(e.message || e);
  }
}

/** AI 根据内容生成书名，确认后写入 */
async function onSuggestTitle(item, ev) {
  ev.stopPropagation();
  const path = item && item.path;
  if (!path || titleBusy[path]) return;
  error.value = "";
  titleBusy[path] = true;
  try {
    const r = await project.suggestBookTitle(path);
    const next = (r && r.title) || "";
    if (!next) throw new Error("未生成书名");
    const prev = (r && r.previous_title) || item.title || "未命名小说";
    const ok = await appConfirm(
      `建议书名：「${next}」\n当前：「${prev}」\n\n应用到这部作品？文件夹名不会改动。`,
      {
        title: "AI 生成书名",
        confirmText: "应用",
        cancelText: "不用",
      }
    );
    if (!ok) return;
    await project.applyBookTitle(path, next);
    await refreshSettings();
    appState.statusMessage = `已更新书名：${next}`;
  } catch (e) {
    error.value = String(e.message || e);
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
        <p class="muted">以卡片打开写作作品；点「+」新建。导入小说请到「知识库」。</p>
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

    <div class="work-grid">
      <button type="button" class="work-card work-card-add" @click="openCreateDialog">
        <span class="plus" aria-hidden="true">+</span>
        <span class="add-label">新建作品</span>
      </button>

      <button
        v-for="item in recentList"
        :key="item.path"
        type="button"
        class="work-card"
        :class="{ active: isActive(item.path) }"
        @click="openByPath(item.path)"
      >
        <div class="card-top">
          <span class="card-badge">小说</span>
          <div class="card-actions">
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
        <div class="card-title">{{ item.title || "未命名小说" }}</div>
        <div class="card-path muted">{{ shortPath(item.path) }}</div>
        <div v-if="isActive(item.path)" class="card-active-tag">当前打开</div>
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

    <pre v-if="error" class="out error">{{ error }}</pre>
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
.hidden-file {
  display: none;
}
.work-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
  gap: 14px;
}
.work-card {
  min-height: 148px;
  padding: 14px 14px 16px;
  border: none;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
  text-align: left;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 8px;
  color: var(--text);
  transition: transform 0.15s ease, box-shadow 0.15s ease, background 0.15s ease;
}
.work-card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow);
  background: var(--accent-soft);
}
.work-card.active {
  box-shadow: 0 0 0 2px var(--accent), var(--shadow);
}
.work-card-add {
  align-items: center;
  justify-content: center;
  background: var(--panel-2);
  border: 1.5px dashed color-mix(in srgb, var(--accent) 45%, transparent);
}
.work-card-add:hover {
  background: var(--accent-soft);
  border-style: solid;
}
.plus {
  font-size: 42px;
  font-weight: 300;
  line-height: 1;
  color: var(--accent-hover);
}
.add-label {
  font-size: 13px;
  font-weight: 650;
  color: var(--muted);
}
.card-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.card-actions {
  display: inline-flex;
  align-items: center;
  gap: 4px;
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
.card-badge {
  font-size: 11px;
  font-weight: 700;
  color: var(--accent-hover);
  background: var(--accent-soft);
  border-radius: 999px;
  padding: 2px 8px;
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
.card-title {
  font-size: 16px;
  font-weight: 750;
  line-height: 1.3;
  word-break: break-word;
}
.card-path {
  font-size: 11px;
  margin-top: auto;
  word-break: break-all;
}
.card-active-tag {
  font-size: 11px;
  font-weight: 700;
  color: var(--accent-hover);
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
