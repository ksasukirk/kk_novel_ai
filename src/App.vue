<script setup>
// 代码路径: kk_novel_ai/src/App.vue
import { computed, onMounted, onUnmounted, ref } from "vue";
import { appState } from "./stores/appState.js";
import { loadSettings, refreshHealth } from "./services/llmClient.js";
import { scheduleStartupUpdateCheck } from "./services/updateFlow.js";
import { startGuiBridge, resolveExternalConflict } from "./services/guiBridge.js";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import UpdateDialog from "./components/UpdateDialog.vue";
import IllustrationPromptDialog from "./components/IllustrationPromptDialog.vue";
import ToastHost from "./components/ToastHost.vue";
import AppSidebar from "./components/shell/AppSidebar.vue";
import PageHeader from "./components/shell/PageHeader.vue";
import PageBackground from "./components/shell/PageBackground.vue";
import ProjectHome from "./views/ProjectHome.vue";
import KnowledgeHome from "./views/KnowledgeHome.vue";
import EditorView from "./views/EditorView.vue";
import OutlineView from "./views/OutlineView.vue";
import LoreView from "./views/LoreView.vue";
import CharacterRosterView from "./views/CharacterRosterView.vue";
import SettingsView from "./views/SettingsView.vue";
import GenLogView from "./views/GenLogView.vue";
import UsageAnalyticsView from "./views/UsageAnalyticsView.vue";
import StoryView from "./views/StoryView.vue";
import { isKbProject, restoreWritingSnapshot } from "./stores/appState.js";
import { isMobileUx, isTauriMobile, watchMobileViewport } from "./utils/platform.js";
import {
  cycleSidebarMode,
  readSidebarMode,
  saveSidebarMode,
  sidebarToggleTitle,
} from "./utils/layoutPrefs.js";

const THEME_KEY = "kk_novel_ai_theme";
const isWindowMaximized = ref(false);
const theme = ref("light");
const mobileUx = ref(isMobileUx());
const tauriMobile = ref(isTauriMobile());
/** 桌面：expanded | compact | closed；移动抽屉打开时为 expanded */
const sidebarMode = ref(readSidebarMode());
const sidebarDrawerOpen = ref(false);

/** 全局侧栏固定，不因知识库切换而替换 */
const sidebarTabs = [
  { id: "project", label: "作品" },
  { id: "knowledge", label: "知识库" },
  { id: "characters", label: "角色定义" },
  { id: "story", label: "总谱" },
  { id: "outline", label: "大纲" },
  { id: "editor", label: "写作" },
  { id: "lore", label: "设定" },
  { id: "analytics", label: "分析" },
  { id: "log", label: "日志" },
  { id: "settings", label: "设置" },
];

const bottomPrimary = [
  { id: "project", label: "作品" },
  { id: "editor", label: "写作" },
  { id: "outline", label: "大纲" },
  { id: "story", label: "总谱" },
];

const moreTabIds = new Set(["knowledge", "characters", "lore", "analytics", "log", "settings"]);

const titleSuffix = computed(() => {
  if (appState.activeNav === "knowledge" && appState.project && isKbProject(appState.project)) {
    if (appState.project.kind === "universal") return "通用知识库";
    return `知识库 · ${appState.project.title}`;
  }
  if (appState.project && appState.project.title && !isKbProject(appState.project)) {
    return appState.project.title;
  }
  if (appState.writingSnapshot && appState.writingSnapshot.project) {
    return appState.writingSnapshot.project.title || "小说创作台";
  }
  return "小说创作台";
});

const pageTitle = computed(() => {
  const tab = sidebarTabs.find((t) => t.id === appState.activeNav);
  return tab ? tab.label : "作品";
});

const moreActive = computed(() => moreTabIds.has(appState.activeNav));

const effectiveSidebarMode = computed(() => {
  if (mobileUx.value) return sidebarDrawerOpen.value ? "expanded" : "closed";
  return sidebarMode.value;
});

const sidebarMenuTitle = computed(() => sidebarToggleTitle(effectiveSidebarMode.value));

function setActiveNav(tab) {
  // 角色定义 / 设定：全局仓，不强制先开写作工程
  if (tab.id === "lore" || tab.id === "characters") {
    appState.activeNav = tab.id;
    if (mobileUx.value) sidebarDrawerOpen.value = false;
    return;
  }
  // 离开知识库、进入写作相关页时，恢复写作作品；没有写作快照则留在知识库页
  if (tab.id !== "knowledge" && isKbProject(appState.project)) {
    if (["project", "editor", "outline", "story"].includes(tab.id)) {
      const ok = restoreWritingSnapshot();
      if (!ok && tab.id !== "project") {
        appState.statusMessage = "知识库内容请在「知识库」页内查看；请先在「作品」打开写作工程";
        appState.activeNav = "knowledge";
        if (mobileUx.value) sidebarDrawerOpen.value = false;
        return;
      }
    }
  }
  appState.activeNav = tab.id;
  if (mobileUx.value) sidebarDrawerOpen.value = false;
}

function openMoreDrawer() {
  sidebarDrawerOpen.value = true;
}

const themeLabel = computed(() => (theme.value === "dark" ? "浅色主题" : "深色主题"));

function applyTheme(next) {
  theme.value = next === "dark" ? "dark" : "light";
  document.documentElement.setAttribute("data-theme", theme.value);
  try {
    localStorage.setItem(THEME_KEY, theme.value);
  } catch {
    /* ignore */
  }
}

function toggleTheme() {
  applyTheme(theme.value === "dark" ? "light" : "dark");
}

function toggleSidebar() {
  if (mobileUx.value) {
    sidebarDrawerOpen.value = !sidebarDrawerOpen.value;
    return;
  }
  sidebarMode.value = cycleSidebarMode(sidebarMode.value);
  saveSidebarMode(sidebarMode.value);
}

function getCurrentWindow() {
  const t = globalThis.__TAURI__;
  if (!t || !t.window) return null;
  if (typeof t.window.getCurrentWindow === "function") return t.window.getCurrentWindow();
  if (t.window.appWindow) return t.window.appWindow;
  return null;
}

async function minimizeWindow() {
  const win = getCurrentWindow();
  if (win && typeof win.minimize === "function") await win.minimize();
}

async function toggleMaximizeWindow() {
  const win = getCurrentWindow();
  if (!win) return;
  if (typeof win.toggleMaximize === "function") {
    await win.toggleMaximize();
  } else if (
    typeof win.isMaximized === "function" &&
    typeof win.maximize === "function" &&
    typeof win.unmaximize === "function"
  ) {
    const isMax = await win.isMaximized();
    if (isMax) await win.unmaximize();
    else await win.maximize();
  }
  await syncWindowMaximizedState();
}

async function closeWindow() {
  const win = getCurrentWindow();
  if (win && typeof win.close === "function") await win.close();
}

async function startWindowDrag(event) {
  if (mobileUx.value || tauriMobile.value) return;
  if (event && event.button !== 0) return;
  const win = getCurrentWindow();
  if (win && typeof win.startDragging === "function") await win.startDragging();
}

async function syncWindowMaximizedState() {
  try {
    const win = getCurrentWindow();
    if (win && typeof win.isMaximized === "function") {
      isWindowMaximized.value = await win.isMaximized();
    }
  } catch {
    isWindowMaximized.value = false;
  }
}

let unwatchMobile = () => {};

onMounted(async () => {
  try {
    const saved = localStorage.getItem(THEME_KEY);
    applyTheme(saved === "dark" ? "dark" : "light");
  } catch {
    applyTheme("light");
  }
  mobileUx.value = isMobileUx();
  tauriMobile.value = isTauriMobile();
  if (mobileUx.value) sidebarDrawerOpen.value = false;
  unwatchMobile = watchMobileViewport((m) => {
    mobileUx.value = m || isTauriMobile();
    if (mobileUx.value) sidebarDrawerOpen.value = false;
  });
  void syncWindowMaximizedState();
  try {
    await startGuiBridge();
  } catch {
    /* 非 Tauri 预览时可忽略 */
  }
  try {
    await loadSettings();
    await refreshHealth();
  } catch {
    /* 设置页可再试 */
  }
  scheduleStartupUpdateCheck();
});

onUnmounted(() => {
  unwatchMobile();
});
</script>

<template>
  <div class="app-shell" :class="{ 'is-mobile': mobileUx }">
    <header class="titlebar" :class="{ 'titlebar-mobile': mobileUx || tauriMobile }">
      <div class="titlebar-left" @mousedown="startWindowDrag">
        <div class="titlebar-logo" aria-hidden="true">K</div>
        <div class="titlebar-brand">Kk Novel Ai</div>
        <div class="titlebar-title">{{ titleSuffix }}</div>
      </div>
      <div v-if="!mobileUx && !tauriMobile" class="titlebar-controls">
        <button type="button" class="titlebar-btn" @click="minimizeWindow" title="最小化">
          <span class="icon icon-minimize"></span>
        </button>
        <button type="button" class="titlebar-btn" @click="toggleMaximizeWindow" :title="isWindowMaximized ? '还原' : '最大化'">
          <span class="icon icon-maximize" :class="isWindowMaximized ? 'restore' : ''"></span>
        </button>
        <button type="button" class="titlebar-btn titlebar-btn-close" @click="closeWindow" title="关闭">
          <span class="icon icon-close"></span>
        </button>
      </div>
    </header>

    <div class="layout-body">
      <PageBackground />
      <div
        v-if="mobileUx && sidebarDrawerOpen"
        class="sidebar-backdrop"
        @click="sidebarDrawerOpen = false"
      />
      <AppSidebar
        :mode="effectiveSidebarMode"
        :tabs="sidebarTabs"
        :active-id="appState.activeNav"
        :theme="theme"
        :theme-label="themeLabel"
        :overlay="mobileUx"
        @select="setActiveNav"
        @toggle-theme="toggleTheme"
      />
      <main
        class="content-surface main-panel"
        :class="{
          'sidebar-collapsed': effectiveSidebarMode === 'closed',
          'sidebar-compact': effectiveSidebarMode === 'compact',
        }"
      >
        <PageHeader
          :sidebar-mode="effectiveSidebarMode"
          :sidebar-menu-title="sidebarMenuTitle"
          :title="pageTitle"
          :llm-online="appState.llmOnline"
          :llm-model="appState.llmModel || ''"
          :has-project="!!appState.projectRoot"
          :status-message="appState.statusMessage || ''"
          :show-gen-progress="appState.generating || appState.genProgressPct > 0"
          @toggle-sidebar="toggleSidebar"
        />
        <div
          class="main-scroll"
          :class="{
            'main-scroll-lock':
              appState.activeNav === 'characters' ||
              appState.activeNav === 'story' ||
              appState.activeNav === 'outline' ||
              appState.activeNav === 'knowledge',
          }"
        >
          <ProjectHome v-if="appState.activeNav === 'project'" />
          <KnowledgeHome v-else-if="appState.activeNav === 'knowledge'" />
          <StoryView v-else-if="appState.activeNav === 'story'" />
          <OutlineView v-else-if="appState.activeNav === 'outline'" />
          <EditorView v-else-if="appState.activeNav === 'editor'" />
          <CharacterRosterView v-else-if="appState.activeNav === 'characters'" />
          <LoreView v-else-if="appState.activeNav === 'lore'" />
          <UsageAnalyticsView v-else-if="appState.activeNav === 'analytics'" />
          <GenLogView v-else-if="appState.activeNav === 'log'" />
          <SettingsView v-else-if="appState.activeNav === 'settings'" />
        </div>
      </main>
    </div>

    <nav v-if="mobileUx" class="bottom-nav" aria-label="主导航">
      <button
        v-for="tab in bottomPrimary"
        :key="tab.id"
        type="button"
        class="bottom-nav-item"
        :class="{ active: appState.activeNav === tab.id }"
        @click="setActiveNav(tab)"
      >
        {{ tab.label }}
      </button>
      <button
        type="button"
        class="bottom-nav-item"
        :class="{ active: moreActive || sidebarDrawerOpen }"
        @click="openMoreDrawer"
      >
        更多
      </button>
    </nav>

    <div v-if="appState.externalConflict" class="conflict-mask">
      <div class="conflict-card">
        <h2>外部写入冲突</h2>
        <p>当前章节有未保存编辑，CLI / 外部写入想覆盖。请选择：</p>
        <div class="conflict-actions">
          <button type="button" class="app-btn app-btn-primary" @click="resolveExternalConflict(true)">
            保留本地
          </button>
          <button type="button" class="app-btn" @click="resolveExternalConflict(false)">
            接受外部覆盖
          </button>
        </div>
      </div>
    </div>

    <ConfirmDialog />
    <UpdateDialog />
    <IllustrationPromptDialog />
    <ToastHost />
  </div>
</template>

<style scoped>
.app-shell {
  height: 100vh;
  height: 100dvh;
  display: flex;
  flex-direction: column;
  background: var(--bg);
  color: var(--text);
  font-family: var(--font-ui);
  overflow: hidden;
}

.app-shell.is-mobile {
  padding-bottom: calc(56px + env(safe-area-inset-bottom, 0px));
}

.titlebar-mobile {
  padding-top: env(safe-area-inset-top, 0px);
  min-height: calc(38px + env(safe-area-inset-top, 0px));
}

.sidebar-backdrop {
  position: absolute;
  inset: 0;
  z-index: 28;
  background: rgba(20, 16, 24, 0.35);
}

.bottom-nav {
  position: fixed;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 45;
  display: flex;
  align-items: stretch;
  gap: 2px;
  min-height: calc(56px + env(safe-area-inset-bottom, 0px));
  padding: 4px 6px calc(4px + env(safe-area-inset-bottom, 0px));
  background: var(--panel);
  border-top: 1px solid var(--divider);
  box-shadow: 0 -4px 18px rgba(0, 0, 0, 0.08);
}

.bottom-nav-item {
  flex: 1;
  min-height: 44px;
  border: none;
  border-radius: 12px;
  background: transparent;
  color: var(--muted);
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
}

.bottom-nav-item.active {
  color: var(--accent-hover);
  background: var(--accent-soft);
}

.titlebar {
  height: 38px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: none;
  background: var(--titlebar-bg);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  user-select: none;
  flex-shrink: 0;
  z-index: 40;
}

.titlebar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 100%;
  flex: 1;
  min-width: 0;
  padding: 0 14px;
}

.titlebar-logo {
  width: 20px;
  height: 20px;
  border-radius: 8px;
  border: none;
  background: linear-gradient(135deg, #f472b6, #c084fc);
  color: #fff;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 700;
  line-height: 1;
  flex-shrink: 0;
  box-shadow: var(--shadow-sm);
}

.titlebar-brand {
  font-size: 12px;
  color: var(--muted);
  font-weight: 600;
}

.titlebar-title {
  font-size: 12px;
  color: var(--text);
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.titlebar-controls {
  display: flex;
  align-items: center;
  height: 100%;
}

.titlebar-btn {
  width: 44px;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  padding: 0;
}

.titlebar-btn:hover {
  background: var(--accent-soft);
  color: var(--text);
}

.titlebar-btn-close:hover {
  background: rgba(239, 68, 68, 0.18);
  color: var(--warn-text);
}

.icon {
  position: relative;
  display: block;
  width: 12px;
  height: 12px;
  margin: 0 auto;
  color: currentColor;
}

.icon-minimize::before {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  bottom: 1px;
  height: 1.5px;
  border-radius: 1px;
  background: currentColor;
}

.icon-maximize::before {
  content: "";
  position: absolute;
  inset: 1px;
  border: 1.5px solid currentColor;
  border-radius: 2px;
}

.icon-maximize.restore::before {
  inset: 3px 0 0 3px;
}

.icon-maximize.restore::after {
  content: "";
  position: absolute;
  inset: 0 3px 3px 0;
  border: 1.5px solid currentColor;
  border-radius: 2px;
}

.icon-close::before,
.icon-close::after {
  content: "";
  position: absolute;
  left: 5px;
  top: 0;
  width: 1.5px;
  height: 12px;
  border-radius: 1px;
  background: currentColor;
}

.icon-close::before {
  transform: rotate(45deg);
}

.icon-close::after {
  transform: rotate(-45deg);
}

.layout-body {
  flex: 1;
  min-height: 0;
  display: flex;
  position: relative;
  background: var(--bg-grad);
  padding: 4px;
  gap: 0;
}

.main-panel {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  margin: 4px;
  overflow: hidden;
  position: relative;
  z-index: 1;
  box-shadow: var(--shadow);
}

.main-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 4px 12px 16px;
}

/* 角色定义：左右栏各自滚动，禁止外层整页同步滚 */
.main-scroll-lock {
  overflow: hidden;
  display: flex;
  flex-direction: column;
  padding-bottom: 8px;
}

.main-scroll-lock > * {
  flex: 1;
  min-height: 0;
}

.conflict-mask {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(20, 16, 24, 0.45);
  backdrop-filter: blur(4px);
}

.conflict-card {
  max-width: 420px;
  padding: 22px 24px;
  border-radius: var(--radius-lg);
  background: var(--panel);
  box-shadow: var(--shadow);
}

.conflict-card h2 {
  margin: 0 0 10px;
  font-size: 16px;
}

.conflict-card p {
  margin: 0 0 16px;
  color: var(--muted);
  line-height: 1.5;
}

.conflict-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}
</style>
