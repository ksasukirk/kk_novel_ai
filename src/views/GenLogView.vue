<!--
  生成日志 + token/费用累计
  代码路径: kk_novel_ai/src/views/GenLogView.vue
-->
<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import { loadGenLogs, loadUsageSummary, exportTxt, exportPdf, exportEpub, pickDirectory } from "../services/projectClient.js";
import CapsuleSwitch from "../components/CapsuleSwitch.vue";

const error = ref("");
const exportMsg = ref("");
const exporting = ref(false);
/** 历史对话默认隐藏；用户明确打开后才加载并展示 */
const showHistory = ref(false);
/** 默认只看当前作品；需要排查时可手动查看全部作品 */
const showAllProjects = ref(false);
const historyLoaded = ref(false);

const visibleLogs = computed(() => {
  const logs = Array.isArray(appState.genLogs) ? appState.genLogs : [];
  if (showAllProjects.value) return logs;
  const root = String(appState.projectRoot || "");
  if (!root) return [];
  return logs.filter((item) => String(item.project_root || "") === root);
});

const usageLine = computed(() => {
  const g = appState.usageSummary && appState.usageSummary.global;
  if (!g) return "";
  const p = appState.usageSummary.project;
  const parts = [
    `全局 ${g.total_tokens || (g.prompt_tokens || 0) + (g.completion_tokens || 0)} tok`,
    `¥${Number(g.cost_cny || 0).toFixed(4)}`,
    `${g.calls || 0} 次`,
  ];
  if (p) {
    parts.push(
      `本作品 ${(p.prompt_tokens || 0) + (p.completion_tokens || 0)} tok · ¥${Number(p.cost_cny || 0).toFixed(4)}`
    );
  }
  return parts.join(" · ");
});

function safeFileStem(name) {
  return String(name || "novel")
    .replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 80) || "novel";
}

async function exportToDir(ext, runner, label) {
  error.value = "";
  exportMsg.value = "";
  if (!appState.projectRoot) {
    error.value = "请先打开作品";
    return;
  }
  exporting.value = true;
  try {
    const picked = await pickDirectory();
    const stem = safeFileStem(appState.project && appState.project.title);
    const out = `${picked.path}\\${stem}.${ext}`;
    await runner(out);
    exportMsg.value = `已导出 ${label}：${out}`;
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    exporting.value = false;
  }
}

async function refreshUsage() {
  error.value = "";
  try {
    await loadUsageSummary(appState.projectRoot || null);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function refreshHistory() {
  error.value = "";
  try {
    await loadGenLogs(80);
    historyLoaded.value = true;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onHistoryToggle(enabled) {
  showHistory.value = !!enabled;
  if (showHistory.value && !historyLoaded.value) {
    await refreshHistory();
  }
}

onMounted(refreshUsage);

watch(
  () => appState.projectRoot,
  () => {
    // 切换 / 新建作品后不沿用旧作品的展开状态。
    showHistory.value = false;
    showAllProjects.value = false;
    historyLoaded.value = false;
    appState.genLogs = [];
    void refreshUsage();
  }
);

function usageLabel(item) {
  const u = item.usage;
  if (!u) return "";
  const total = u.total_tokens || (u.prompt_tokens || 0) + (u.completion_tokens || 0);
  const src = u.source === "api" ? "api" : "估";
  return `${total} tok (${src})`;
}

function formatMessages(item) {
  const msgs = item.messages || [];
  if (!msgs.length) return item.instruction || "";
  return msgs.map((m) => `【${m.role}】\n${m.content || ""}`).join("\n\n---\n\n");
}

async function onExport() {
  await exportToDir("txt", exportTxt, "TXT");
}

async function onExportPdf() {
  await exportToDir("pdf", exportPdf, "PDF");
}

async function onExportEpub() {
  await exportToDir("epub", exportEpub, "EPUB");
}
</script>

<template>
  <section class="panel">
    <h1 class="panel-heading">日志 / 导出</h1>
    <p v-if="usageLine" class="muted usage-sum">{{ usageLine }}</p>
    <div class="actions">
      <button type="button" class="app-btn app-btn-primary" :disabled="exporting" @click="onExport">
        导出 TXT
      </button>
      <button type="button" class="app-btn" :disabled="exporting" @click="onExportPdf">
        导出 PDF
      </button>
      <button type="button" class="app-btn app-btn-warning" :disabled="exporting" @click="onExportEpub">
        导出 EPUB
      </button>
    </div>
    <p v-if="exportMsg" class="muted">{{ exportMsg }}</p>

    <div class="history-controls">
      <CapsuleSwitch
        :model-value="showHistory"
        label="查看 AI 历史对话"
        @update:model-value="onHistoryToggle"
      />
      <template v-if="showHistory">
        <CapsuleSwitch
          v-model="showAllProjects"
          label="包含其他作品"
        />
        <button type="button" class="app-btn app-btn-info" @click="refreshHistory">
          刷新历史
        </button>
      </template>
    </div>
    <p v-if="!showHistory" class="muted history-hint">
      历史对话默认隐藏；打开开关后仅显示当前作品。
    </p>

    <template v-if="showHistory">
      <div v-for="(item, idx) in visibleLogs" :key="item.id || idx" class="log-card">
        <div class="log-meta">
          {{ item.ts }} · {{ item.source }} · {{ item.task }}
          <span v-if="item.model_used"> · {{ item.model_used }}</span>
          <span v-if="item.truncated"> · 已截断</span>
          <span v-if="usageLabel(item)"> · {{ usageLabel(item) }}</span>
          <span v-if="item.cost_cny"> · ¥{{ Number(item.cost_cny).toFixed(4) }}</span>
        </div>
        <div class="muted">{{ item.project_root }} / {{ item.chapter_id }}</div>
        <details v-if="item.final_text || item.preview">
          <summary>正文</summary>
          <pre class="preview">{{ item.final_text || item.preview }}</pre>
        </details>
        <details v-if="(item.messages && item.messages.length) || item.instruction">
          <summary>提示词</summary>
          <pre class="preview">{{ formatMessages(item) }}</pre>
        </details>
        <details v-if="item.raw_text && item.raw_text !== item.final_text">
          <summary>原始全文（截断前）</summary>
          <pre class="preview">{{ item.raw_text }}</pre>
        </details>
      </div>
      <p v-if="historyLoaded && !visibleLogs.length" class="muted">
        {{ showAllProjects ? "暂无生成记录。" : "当前作品暂无 AI 历史对话。" }}
      </p>
    </template>
    <pre v-if="error" class="out error">{{ error }}</pre>
  </section>
</template>

<style scoped>
.panel {
  min-height: calc(100% - 8px);
}
.usage-sum {
  margin: 8px 0 0;
  font-size: 13px;
}
.actions {
  display: flex;
  gap: 8px;
  margin: 10px 0;
  flex-wrap: wrap;
}
.history-controls {
  display: flex;
  align-items: center;
  gap: 10px 18px;
  flex-wrap: wrap;
  margin: 14px 0 8px;
  padding-top: 12px;
  border-top: 1px solid var(--divider);
}
.history-hint {
  margin: 6px 0 0;
}
.log-card {
  margin-top: 12px;
  padding: 16px 18px;
  border: none;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow);
}
.log-meta {
  font-weight: 700;
  color: var(--accent-hover);
  margin-bottom: 4px;
}
.preview {
  white-space: pre-wrap;
  font-size: 12px;
  margin: 10px 0 0;
  padding: 10px 12px;
  border-radius: var(--radius-md);
  background: var(--panel-2);
  color: var(--text);
  font-family: var(--font-mono);
  box-shadow: var(--shadow-sm);
}
.error {
  color: var(--error);
}
</style>
