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
import { useToastError } from "../services/toast.js";
import { t } from "../i18n/index.js";

const error = useToastError();
const showImport = ref(false);
const importBackdrop = createBackdropDismiss(() => {
  showImport.value = false;
});
const importing = ref(false);
const importTitle = ref("问道红尘");
const distillBusy = ref(false);
const syncBusy = ref(false);
const registry = ref(null);
const corpusError = useToastError();

const subTabs = computed(() => [
  { id: "home", label: t("knowledge.list") },
  { id: "entities", label: t("knowledge.entities") },
  { id: "story", label: t("knowledge.graph") },
  { id: "corpus", label: t("knowledge.corpus") },
]);

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
  return ch ? ch.title : t("knowledge.noChapter");
});
const wordCount = computed(() => (appState.chapterContent || "").replace(/\s/g, "").length);
const isHomeTab = computed(() => subNav.value === "home");

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
    error.value = t("knowledge.needKb");
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
    appState.statusMessage = t("knowledge.openedUniversal");
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
    appState.statusMessage = t("knowledge.openedKb");
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onImportConfirm() {
  error.value = "";
  importing.value = true;
  try {
    const filePicked = await project.pickFile(t("knowledge.pickTxt"), ["txt", "md"]);
    const dirPicked = await project.pickDirectory();
    const name = importTitle.value.trim() || t("knowledge.untitledKb");
    appState.statusMessage = t("knowledge.importingStatus");
    await kb.importIntoKb(dirPicked.path, filePicked.path, name);
    await refresh();
    showImport.value = false;
    subNav.value = "home";
    appState.statusMessage = t("knowledge.ready", { name });
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    importing.value = false;
  }
}

async function onDistill() {
  error.value = "";
  if (!kb.kbIsSingleBook()) {
    error.value = t("knowledge.needSingleDistill");
    return;
  }
  distillBusy.value = true;
  try {
    appState.statusMessage = t("knowledge.distillingStatus");
    const r = await project.importDistill(appState.projectRoot, {
      from: 1,
      to: 20,
      apply: "auto",
      resume: true,
    });
    await refresh();
    appState.statusMessage = t("knowledge.distillDone", { n: r.entity_count ?? 0 });
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    distillBusy.value = false;
  }
}

async function onSyncCurrent() {
  error.value = "";
  if (!kbOpen.value || isUniversal.value) {
    error.value = t("knowledge.needSingleSync");
    return;
  }
  syncBusy.value = true;
  try {
    const r = await kb.syncKb(appState.projectRoot);
    appState.statusMessage = t("knowledge.synced", { n: r.lore_count ?? 0 });
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
    appState.statusMessage = t("knowledge.syncedAll");
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
    !(await appConfirmDelete(t("knowledge.forgetQ"), {
      title: t("knowledge.forgetTitle"),
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
  <section class="panel kb-shell" :class="{ 'kb-work': !isHomeTab }">
    <div v-if="isHomeTab" class="page-head">
      <div>
        <h1 class="panel-heading">{{ $t("knowledge.title") }}</h1>
        <p class="muted">
          {{ $t("knowledge.intro") }}
        </p>
      </div>
      <div class="head-actions">
        <button type="button" class="app-btn" :title="$t('knowledge.importTxtHint')" @click="showImport = true">{{ $t("knowledge.importToKb") }}</button>
        <button
          type="button"
          class="app-btn"
          :title="$t('knowledge.distillHint')"
          :disabled="!kb.kbIsSingleBook() || distillBusy"
          @click="onDistill"
        >
          {{ distillBusy ? $t("knowledge.distilling") : $t("knowledge.distill20") }}
        </button>
        <button
          type="button"
          class="app-btn"
          :title="$t('knowledge.syncCurrentHint')"
          :disabled="syncBusy || !kb.kbIsSingleBook()"
          @click="onSyncCurrent"
        >
          {{ $t("knowledge.syncCurrent") }}
        </button>
        <button type="button" class="app-btn" :title="$t('knowledge.syncAllHint')" :disabled="syncBusy" @click="onSyncAll">{{ $t("knowledge.syncAll") }}</button>
      </div>
    </div>

    <nav class="kb-subnav" :aria-label="$t('knowledge.subnav')">
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
        {{ $t("knowledge.currentPrefix", { name: isUniversal ? $t("knowledge.universal") : appState.project.title }) }}
      </span>
      <div v-if="!isHomeTab" class="head-actions kb-subnav-actions">
        <button type="button" class="app-btn" :title="$t('knowledge.importTxtHint')" @click="showImport = true">{{ $t("common.import") }}</button>
        <button
          type="button"
          class="app-btn"
          :title="$t('knowledge.distillHint')"
          :disabled="!kb.kbIsSingleBook() || distillBusy"
          @click="onDistill"
        >
          {{ distillBusy ? $t("knowledge.distilling") : $t("knowledge.distillShort") }}
        </button>
        <button
          type="button"
          class="app-btn"
          :title="$t('knowledge.syncCurrentHint')"
          :disabled="syncBusy || !kb.kbIsSingleBook()"
          @click="onSyncCurrent"
        >
          {{ $t("knowledge.syncCurrentShort") }}
        </button>
        <button type="button" class="app-btn" :title="$t('knowledge.syncAllHint')" :disabled="syncBusy" @click="onSyncAll">{{ $t("knowledge.syncAll") }}</button>
      </div>
    </nav>

    <!-- 库列表 -->
    <div v-if="subNav === 'home'" class="kb-pane">
      <div class="work-grid">
        <button type="button" class="work-card work-card-uni" @click="openUniversal">
          <div class="card-top">
            <span class="card-badge">{{ $t("knowledge.universalShort") }}</span>
          </div>
          <div class="card-title">{{ $t("knowledge.universal") }}</div>
          <div class="card-path muted">{{ $t("knowledge.universalDesc") }}</div>
          <div v-if="isUniversal" class="card-active-tag">{{ $t("knowledge.nowOpen") }}</div>
        </button>

        <button type="button" class="work-card work-card-add" @click="showImport = true">
          <span class="plus" aria-hidden="true">+</span>
          <span class="add-label">{{ $t("knowledge.importAsKb") }}</span>
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
            <span class="card-badge">{{ $t("knowledge.singleBook") }}</span>
            <span class="card-forget" :title="$t('knowledge.forgetRecent')" @click="onForget(item.path, $event)">×</span>
          </div>
          <div class="card-title">{{ item.title || $t("knowledge.untitledKb") }}</div>
          <div class="card-path muted">{{ shortPath(item.path) }}</div>
          <div
            v-if="appState.projectRoot === item.path && kbOpen && !isUniversal"
            class="card-active-tag"
          >
            {{ $t("knowledge.nowOpen") }}
          </div>
        </button>
      </div>
    </div>

    <!-- 实体 / 总谱：复用现有视图，仍读当前 projectRoot（此时应为 KB） -->
    <div v-else-if="subNav === 'entities'" class="kb-pane kb-embed">
      <LoreView embedded />
    </div>
    <div v-else-if="subNav === 'story'" class="kb-pane kb-embed">
      <StoryView embedded />
    </div>

    <!-- 语料只读 -->
    <div v-else-if="subNav === 'corpus'" class="kb-pane corpus-layout">
      <aside class="chapter-tree">
        <div class="tree-head">{{ $t("knowledge.corpusChapters") }}</div>
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
        <p v-if="!chapters.length" class="muted pad">{{ $t("knowledge.corpusEmpty") }}</p>
      </aside>
      <div class="corpus-main">
        <div class="corpus-toolbar">
          <strong>{{ currentChapterTitle }}</strong>
          <span class="muted">{{ $t("knowledge.readonlyChars", { n: wordCount }) }}</span>
        </div>
        <textarea
          class="corpus-area"
          :value="appState.chapterContent"
          readonly
          :placeholder="$t('knowledge.corpusPh')"
        />
      </div>
    </div>

    <div
      v-if="showImport"
      class="create-mask"
      @mousedown="importBackdrop.onMouseDown"
      @click="importBackdrop.onClick"
    >
      <div class="create-dialog">
        <h2>{{ $t("knowledge.importDialogTitle") }}</h2>
        <p class="muted">{{ $t("knowledge.importDialogHint") }}</p>
        <div class="field">
          <label class="field-label">{{ $t("knowledge.kbName") }}</label>
          <input
            v-model="importTitle"
            type="text"
            :placeholder="$t('knowledge.bookTitlePh')"
            @keydown.enter.prevent="onImportConfirm"
          />
        </div>
        <div class="dialog-actions">
          <button type="button" class="app-btn" @click="showImport = false">{{ $t("common.cancel") }}</button>
          <button
            type="button"
            class="app-btn app-btn-primary"
            :disabled="importing"
            @click="onImportConfirm"
          >
            {{ importing ? $t("knowledge.importing") : $t("knowledge.pickFiles") }}
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
.kb-work {
  padding-top: 10px;
  padding-bottom: 10px;
}
.kb-subnav {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px 8px;
  margin-bottom: 6px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}
.kb-subtab {
  border: 1px solid var(--border);
  background: var(--panel);
  color: inherit;
  border-radius: 999px;
  padding: 4px 12px;
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
.kb-subnav-actions {
  margin-left: 0;
}
.kb-pane {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.kb-embed {
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.kb-embed > * {
  flex: 1;
  min-height: 0;
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
  min-height: 0;
  height: 100%;
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
