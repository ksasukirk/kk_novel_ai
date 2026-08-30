<!--
  主区 sticky 胶囊顶栏（折叠侧栏 + 页面标题 + 状态徽章）
  代码路径: kk_novel_ai/src/components/shell/PageHeader.vue
-->
<script setup>
import GenProgressBar from "../GenProgressBar.vue";

defineProps({
  /** expanded | compact | closed */
  sidebarMode: { type: String, default: "expanded" },
  sidebarMenuTitle: { type: String, default: "切换导航" },
  title: { type: String, required: true },
  llmOnline: { type: Boolean, default: false },
  llmModel: { type: String, default: "" },
  hasProject: { type: Boolean, default: false },
  statusMessage: { type: String, default: "" },
  showGenProgress: { type: Boolean, default: false },
});

const emit = defineEmits(["toggle-sidebar"]);
</script>

<template>
  <div class="page-header">
    <button
      type="button"
      class="menu-btn"
      :title="sidebarMenuTitle"
      @click="emit('toggle-sidebar')"
    >
      <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor" aria-hidden="true">
        <path d="M3 6h18v2H3V6zm0 5h14v2H3v-2zm0 5h18v2H3v-2z" />
      </svg>
    </button>
    <nav class="crumbs" aria-label="面包屑">
      <span class="crumb">{{ title }}</span>
    </nav>
    <div v-if="showGenProgress" class="header-progress">
      <GenProgressBar variant="compact" />
    </div>
    <div class="header-badges">
      <span class="app-badge" :class="llmOnline ? 'app-badge-success' : 'app-badge-warn'">
        {{ llmOnline ? "LM Studio 在线" : "LM Studio 离线" }}
      </span>
      <span class="app-badge">{{ llmModel || "未选模型" }}</span>
      <span v-if="hasProject" class="app-badge">已打开作品</span>
      <span v-if="statusMessage && !showGenProgress" class="status-msg">{{ statusMessage }}</span>
    </div>
  </div>
</template>

<style scoped>
.page-header {
  height: 40px;
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 8px;
  padding: 0 10px 0 4px;
  border-radius: var(--radius-pill);
  background: var(--header-pill-bg);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--shadow-sm);
  position: sticky;
  top: 8px;
  z-index: 20;
}

.menu-btn {
  width: 36px;
  height: 36px;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.menu-btn:hover {
  background: var(--hover-fill);
  color: var(--text);
}

.crumbs {
  display: flex;
  align-items: center;
  min-width: 0;
}

.crumb {
  font-size: 16px;
  font-weight: 700;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.header-progress {
  flex: 0 0 168px;
  width: 168px;
  height: 22px;
  max-height: 22px;
  overflow: hidden;
  display: flex;
  align-items: center;
}

.header-badges {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  justify-content: flex-end;
  min-width: 0;
}

.status-msg {
  color: var(--muted);
  font-size: 12px;
  max-width: 220px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

@media (max-width: 900px) {
  .status-msg {
    display: none;
  }
}
</style>
