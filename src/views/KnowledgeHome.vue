<!--
  知识库视图（页内子导航，不替换全局侧栏）
  代码路径: kk_novel_ai/src/views/KnowledgeHome.vue
-->
<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { appState, isKbProject } from "../stores/appState.js";
import { loadSettings } from "../services/llmClient.js";
import { invoke } from "../services/tauri.js";
import * as project from "../services/projectClient.js";
import * as kb from "../services/kbClient.js";
import LoreView from "./LoreView.vue";
import StoryView from "./StoryView.vue";
import { appConfirmDelete } from "../services/confirmDialog.js";
import { createBackdropDismiss } from "../utils/backdropDismiss.js";

const error = ref("");
const showImport = ref(false);
const importBackdrop = createBackdropDismiss(() => {
  showImport.value = false;
});
const importing = ref(false);
const importTitle = ref("问道红尘");
const distillBusy = ref(false);
const syncBusy = ref(false);
const registry = ref(null);
const corpusError = ref("");

const subTabs = [
  { id: "home", label: "库列表" },
  { id: "entities", label: "实体" },
  { id: "story", label: "关系总谱" },
  { id: "corpus", label: "语料" },
];

const subNav = computed({
  get: () => appState.kbSubNav || "home",
  set: (v) => {
    appState.kbSubNav = v;
  },
});

const kbOpen = computed(() => isKbProject(appState.project));
const isUniversal = computed(() => appState.project && appState.project.kind === "universal");
const chapters = computed(() => (appState.project && appState.project.chapters) || []);
const currentChapterTitle = computed(() => {
  const ch = chapters.value.find((c) => c.id === appState.chapterId);
  return ch ? ch.title : "未选章";
});
const wordCount = computed(() => (appState.chapterContent || "").replace(/\s/g, "").length);

const recentKb = computed(() => {
  const list = (appState.settings && appState.settings.recent_knowledge_bases) || [];
  return Array.isArray(list) ? list : [];
});

const entryList = computed(() => {
  const fromReg = (registry.value && registry.value.entries) || [];
  if (fromReg.length) return fromReg;
  return recentKb.value.map((x) => ({
    path: x.path,
    title: x.title,
    kind: "knowledge_base",
  }));
});

async function refresh() {
  try {
    await loadSettings();
  } catch {
    /* ignore */
  }
  try {
    registry.value = await kb.listRegistry();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

onMounted(refresh);

function shortPath(path) {
  if (!path) return "";
  const parts = path.replace(/\\/g, "/").split("/");
  return parts.slice(-2).join("/") || path;
}

function ensureKbForWorkbench() {
  if (!kbOpen.value) {
    error.value = "请先在「库列表」打开通用库或单书知识库";
    subNav.value = "home";
    return false;
  }
  return true;
}

function setSub(id) {
  error.value = "";
  if (id !== "home" && !ensureKbForWorkbench()) return;
  subNav.value = id;
}

async function openUniversal() {
  error.value = "";
  try {
    await kb.openUniversal();
    await refresh();
    subNav.value = "entities";
    appState.statusMessage = "已打开通用知识库";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function openByPath(path) {
  error.value = "";
  try {
    await kb.openKnowledgeBase(path);
    await refresh();
    subNav.value = "entities";
    if (appState.chapterId) await project.loadChapter(appState.chapterId);
    appState.statusMessage = `已打开知识库`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onImportConfirm() {
  error.value = "";
  importing.value = true;
  try {
    const filePicked = await project.pickFile("选择小说 TXT（导入为知识库）", ["txt", "md"]);
    const dirPicked = await project.pickDirectory();
    const name = importTitle.value.trim() || "未命名知识库";
    appState.statusMessage = "正在导入为知识库…";
    await kb.importIntoKb(dirPicked.path, filePicked.path, name);
    await refresh();
    showImport.value = false;
    subNav.value = "home";
    appState.statusMessage = `知识库「${name}」已就绪`;
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    importing.value = false;
  }
}

async function onDistill() {
  error.value = "";
  if (!kb.kbIsSingleBook()) {
    error.value = "请先打开单书知识库再蒸馏（通用库不可蒸馏）";
    return;
  }
  distillBusy.value = true;
  try {
    appState.statusMessage = "正在蒸馏前 20 章…";
    const r = await project.importDistill(appState.projectRoot, {
      from: 1,
      to: 20,
      apply: "auto",
      resume: true,
    });
    await refresh();
    appState.statusMessage = `蒸馏完成：实体 ${r.entity_count ?? 0}（已尝试同步通用库）`;
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    distillBusy.value = false;
  }
}

async function onSyncCurrent() {
  error.value = "";
  if (!kbOpen.value || isUniversal.value) {
    error.value = "请打开单书知识库再同步到通用库";
    return;
  }
  syncBusy.value = true;
  try {
    const r = await kb.syncKb(appState.projectRoot);
    appState.statusMessage = `已同步到通用库：lore ${r.lore_count ?? 0}`;
    await refresh();
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    syncBusy.value = false;
  }
}

async function onSyncAll() {
  error.value = "";
  syncBusy.value = true;
  try {
    await kb.syncAll();
    appState.statusMessage = "全部知识库已同步到通用库";
    await refresh();
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    syncBusy.value = false;
  }
}

async function onForget(path, ev) {
  ev.stopPropagation();
  if (
    !(await appConfirmDelete("从最近列表移除该知识库？", {
      title: "移除知识库",
    }))
  ) {
    return;
  }
  try {
    const r = await invoke("project_forget_recent", { root: path });
    if (r.settings) appState.settings = r.settings;
    else await refresh();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function selectCorpusChapter(id) {
  corpusError.value = "";
  try {
    await project.loadChapter(id);
  } catch (e) {
    corpusError.value = String(e.message || e);
  }
}

watch(
  () => appState.kbSubNav,
  async (v) => {
    if (v === "corpus" && appState.chapterId && !appState.chapterContent) {
      try {
        await project.loadChapter(appState.chapterId);
      } catch {
        /* ignore */
      }
    }
  }
);
</script>

<template>
  <section class="panel kb-shell">
    <div class="page-head">
      <div>
        <h1 class="panel-heading">知识库</h1>
        <p class="muted">
          一书一库：导入只作语料与证据；通用库聚合全部来源。写作请用侧栏「作品 / 写作」，本页不替换全局导航。
        </p>
      </div>
      <div class="head-actions">
        <button type="button" class="app-btn" @click="showImport = true">导入进知识库</button>
        <button
          type="button"
          class="app-btn"
          :disabled="!kb.kbIsSingleBook() || distillBusy"
          @click="onDistill"
        >
          {{ distillBusy ? "蒸馏中…" : "蒸馏前20章" }}
        </button>
        <button
          type="button"
          class="app-btn"
          :disabled="syncBusy || !kb.kbIsSingleBook()"
          @click="onSyncCurrent"
        >
          同步当前到通用库
        </button>
        <button type="button" class="app-btn" :disabled="syncBusy" @click="onSyncAll">全部同步</button>
      </div>
    </div>

    <nav class="kb-subnav" aria-label="知识库子导航">
      <button
        v-for="t in subTabs"
        :key="t.id"
        type="button"
        class="kb-subtab"
        :class="{ active: subNav === t.id }"
        @click="setSub(t.id)"
      >
        {{ t.label }}
      </button>
      <span v-if="kbOpen" class="kb-current muted">
        当前：{{ isUniversal ? "通用知识库" : appState.project.title }}
      </span>
    </nav>

    <p v-if="error" class="error">{{ error }}</p>

    <!-- 库列表 -->
    <div v-if="subNav === 'home'" class="kb-pane">
      <div class="work-grid">
        <button type="button" class="work-card work-card-uni" @click="openUniversal">
          <div class="card-top">
            <span class="card-badge">通用</span>
          </div>
          <div class="card-title">通用知识库</div>
          <div class="card-path muted">聚合所有已导入小说 · 带来源</div>
          <div v-if="isUniversal" class="card-active-tag">当前打开</div>
        </button>

        <button type="button" class="work-card work-card-add" @click="showImport = true">
          <span class="plus" aria-hidden="true">+</span>
          <span class="add-label">导入小说为知识库</span>
        </button>

        <button
          v-for="item in entryList"
          :key="item.path"
          type="button"
          class="work-card"
          :class="{
            active: appState.projectRoot === item.path && kbOpen && !isUniversal,
          }"
          @click="openByPath(item.path)"
        >
          <div class="card-top">
            <span class="card-badge">单书</span>
            <span class="card-forget" title="从最近移除" @click="onForget(item.path, $event)">×</span>
          </div>
          <div class="card-title">{{ item.title || "未命名知识库" }}</div>
          <div class="card-path muted">{{ shortPath(item.path) }}</div>
          <div
            v-if="appState.projectRoot === item.path && kbOpen && !isUniversal"
            class="card-active-tag"
          >
            当前打开
          </div>
        </button>
      </div>
    </div>

    <!-- 实体 / 总谱：复用现有视图，仍读当前 projectRoot（此时应为 KB） -->
    <div v-else-if="subNav === 'entities'" class="kb-pane kb-embed">
      <LoreView />
    </div>
    <div v-else-if="subNav === 'story'" class="kb-pane kb-embed">
      <StoryView />
    </div>

    <!-- 语料只读 -->
    <div v-else-if="subNav === 'corpus'" class="kb-pane corpus-layout">
      <aside class="chapter-tree">
        <div class="tree-head">语料章节</div>
        <button
          v-for="c in chapters"
          :key="c.id"
          type="button"
          class="chap-btn"
          :class="{ active: c.id === appState.chapterId }"
          @click="selectCorpusChapter(c.id)"
        >
          {{ c.title }}
        </button>
        <p v-if="!chapters.length" class="muted pad">通用库无章节语料；请打开单书库。</p>
      </aside>
      <div class="corpus-main">
        <div class="corpus-toolbar">
          <strong>{{ currentChapterTitle }}</strong>
          <span class="muted">{{ wordCount }} 字 · 只读</span>
        </div>
        <textarea
          class="corpus-area"
          :value="appState.chapterContent"
          readonly
          placeholder="选择左侧章节浏览证据语料…"
        />
        <pre v-if="corpusError" class="error">{{ corpusError }}</pre>
      </div>
    </div>

    <div
      v-if="showImport"
      class="create-mask"
      @mousedown="importBackdrop.onMouseDown"
      @click="importBackdrop.onClick"
    >
      <div class="create-dialog">
        <h2>导入为知识库</h2>
        <p class="muted">先选 TXT，再选空目录。将创建 kind=knowledge_base，不会进入写作工程。</p>
        <div class="field">
          <label class="field-label">知识库名称</label>
          <input
            v-model="importTitle"
            type="text"
            placeholder="书名"
            @keydown.enter.prevent="onImportConfirm"
          />
        </div>
        <div class="dialog-actions">
          <button type="button" class="app-btn" @click="showImport = false">取消</button>
          <button
            type="button"
            class="app-btn app-btn-primary"
            :disabled="importing"
            @click="onImportConfirm"
          >
            {{ importing ? "导入中…" : "选择文件与目录" }}
          </button>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.kb-shell {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}
.page-head {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
  flex-wrap: wrap;
  margin-bottom: 12px;
}
.head-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.kb-subnav {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-bottom: 14px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--border);
}
.kb-subtab {
  border: 1px solid var(--border);
  background: var(--panel);
  color: inherit;
  border-radius: 999px;
  padding: 6px 14px;
  cursor: pointer;
  font-size: 13px;
}
.kb-subtab.active {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 600;
}
.kb-current {
  margin-left: auto;
  font-size: 12px;
}
.kb-pane {
  flex: 1;
  min-height: 0;
}
.kb-embed {
  overflow: auto;
}
.work-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 12px;
}
.work-card {
  text-align: left;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 14px;
  background: var(--panel);
  cursor: pointer;
  color: inherit;
}
.work-card:hover,
.work-card.active {
  border-color: var(--accent);
  box-shadow: var(--shadow-sm);
}
.work-card-uni {
  background: linear-gradient(145deg, var(--accent-soft), var(--panel));
}
.work-card-add {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 120px;
  border-style: dashed;
}
.plus {
  font-size: 28px;
  line-height: 1;
  color: var(--muted);
}
.add-label {
  margin-top: 6px;
  color: var(--muted);
  font-size: 13px;
}
.card-top {
  display: flex;
  justify-content: space-between;
  margin-bottom: 8px;
}
.card-badge {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent);
}
.card-forget {
  opacity: 0.5;
  padding: 0 4px;
}
.card-title {
  font-weight: 700;
  margin-bottom: 4px;
}
.card-path {
  font-size: 12px;
}
.card-active-tag {
  margin-top: 8px;
  font-size: 12px;
  color: var(--accent);
}
.corpus-layout {
  display: flex;
  gap: 10px;
  min-height: 420px;
  height: calc(100vh - 260px);
}
.chapter-tree {
  width: 200px;
  flex-shrink: 0;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
  padding: 8px;
}
.tree-head {
  font-size: 12px;
  font-weight: 700;
  margin-bottom: 8px;
  color: var(--muted);
}
.chap-btn {
  display: block;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  color: inherit;
  padding: 6px 8px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
}
.chap-btn:hover,
.chap-btn.active {
  background: var(--accent-soft);
}
.pad {
  padding: 8px;
  font-size: 12px;
}
.corpus-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
  overflow: hidden;
}
.corpus-toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border);
}
.corpus-area {
  flex: 1;
  width: 100%;
  border: none;
  resize: none;
  padding: 12px 14px;
  background: transparent;
  color: inherit;
  font-family: var(--editor-font-family, var(--font-mono, ui-monospace, monospace));
  font-size: var(--editor-font-size, 13px);
  line-height: 1.6;
}
.create-mask {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(0, 0, 0, 0.35);
  display: flex;
  align-items: center;
  justify-content: center;
}
.create-dialog {
  width: min(420px, 92vw);
  padding: 20px;
  border-radius: var(--radius-lg);
  background: var(--panel);
  box-shadow: var(--shadow);
}
.field {
  margin: 12px 0;
}
.field-label {
  display: block;
  font-size: 12px;
  color: var(--muted);
  margin-bottom: 4px;
}
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.error {
  color: var(--warn-text);
  margin-bottom: 12px;
}
</style>
