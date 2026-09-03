<!--
  数据分析：余额 + KPI 仪表盘 + 折线/柱状 + 列表详情
  代码路径: kk_novel_ai/src/views/UsageAnalyticsView.vue
-->
<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import {
  loadGenLogs,
  loadProjectGenLogs,
  loadProviderBalance,
  loadUsageSummary,
} from "../services/projectClient.js";
import CapsuleSwitch from "../components/CapsuleSwitch.vue";
import UsageTrendChart from "../components/analytics/UsageTrendChart.vue";
import UsageBarChart from "../components/analytics/UsageBarChart.vue";
import { useToastError } from "../services/toast.js";
import {
  bucketCacheRate,
  bucketTotalTokens,
  cacheHitRate,
  formatCacheHit,
  formatCost,
  formatMessages,
  formatTokens,
  usageTotalTokens,
} from "../utils/usageFormat.js";
import {
  aggregateByModel,
  aggregateByTask,
  buildDailySeries,
  summarizeLogs,
} from "../utils/usageSeries.js";
import { estimateFromSettings } from "../utils/usageEstimate.js";

const error = useToastError();
const loading = ref(false);
const showAllProjects = ref(false);
const filterTask = ref("");
const filterModel = ref("");
const selectedId = ref("");
const chartMetric = ref("cost");

const dataSourceLabel = computed(() =>
  showAllProjects.value ? "全局日志" : "本作品目录（gen_activity / .genlog）"
);

const sourceLogs = computed(() => {
  if (showAllProjects.value) {
    return Array.isArray(appState.genLogs) ? appState.genLogs : [];
  }
  const local = Array.isArray(appState.projectGenLogs) ? appState.projectGenLogs : [];
  if (local.length) return local;
  const root = String(appState.projectRoot || "");
  const global = Array.isArray(appState.genLogs) ? appState.genLogs : [];
  return root ? global.filter((item) => String(item.project_root || "") === root) : [];
});

const selected = computed(() => {
  const id = selectedId.value;
  if (!id) return null;
  return sourceLogs.value.find((x) => String(x.id) === String(id)) || null;
});

const taskOptions = computed(() => {
  const set = new Set();
  for (const item of sourceLogs.value) {
    if (item.task) set.add(String(item.task));
  }
  return [...set].sort();
});

const modelOptions = computed(() => {
  const set = new Set();
  for (const item of sourceLogs.value) {
    if (item.model_used) set.add(String(item.model_used));
  }
  return [...set].sort();
});

const visibleLogs = computed(() => {
  let logs = [...sourceLogs.value];
  const task = filterTask.value.trim();
  if (task) logs = logs.filter((item) => String(item.task || "") === task);
  const model = filterModel.value.trim();
  if (model) logs = logs.filter((item) => String(item.model_used || "") === model);
  logs.sort((a, b) => String(b.ts || "").localeCompare(String(a.ts || "")));
  return logs;
});

const hasRealData = computed(() => visibleLogs.value.length > 0);

const estimate = computed(() => estimateFromSettings(appState.settings));

const kpi = computed(() => {
  if (hasRealData.value) {
    return { ...summarizeLogs(visibleLogs.value), mode: "real" };
  }
  const p = estimate.value.perCall;
  return {
    mode: "estimate",
    calls: 0,
    cost: p.cost,
    tokens: p.tokens,
    hitRate: null,
    hit: 0,
    miss: 0,
  };
});

const dailySeries = computed(() => {
  if (hasRealData.value) return buildDailySeries(visibleLogs.value, 14);
  return estimate.value.scenarioDaily;
});

const modelBars = computed(() =>
  hasRealData.value ? aggregateByModel(visibleLogs.value) : []
);

const taskBars = computed(() =>
  hasRealData.value ? aggregateByTask(visibleLogs.value) : []
);

const byModelRows = computed(() => {
  const map = (appState.usageSummary && appState.usageSummary.by_model) || {};
  return Object.entries(map)
    .map(([name, b]) => ({
      name,
      tokens: bucketTotalTokens(b),
      cost: b.cost_cny || 0,
      calls: b.calls || 0,
      hitRate: bucketCacheRate(b),
      hit: b.prompt_cache_hit_tokens || 0,
      miss: b.prompt_cache_miss_tokens || 0,
    }))
    .sort((a, b) => b.cost - a.cost || b.tokens - a.tokens);
});

const globalSummary = computed(() => {
  const g = appState.usageSummary && appState.usageSummary.global;
  if (!g) return null;
  return {
    tokens: bucketTotalTokens(g),
    cost: g.cost_cny || 0,
    calls: g.calls || 0,
    hit: g.prompt_cache_hit_tokens || 0,
    miss: g.prompt_cache_miss_tokens || 0,
    hitRate: bucketCacheRate(g),
  };
});

const projectSummary = computed(() => {
  const p = appState.usageSummary && appState.usageSummary.project;
  if (!p) return null;
  return {
    tokens: bucketTotalTokens(p),
    cost: p.cost_cny || 0,
    calls: p.calls || 0,
    hitRate: bucketCacheRate(p),
    hit: p.prompt_cache_hit_tokens || 0,
    miss: p.prompt_cache_miss_tokens || 0,
  };
});

const balance = computed(() => appState.providerBalance || null);

const contextItems = computed(() => {
  const cs = selected.value && selected.value.context_sources;
  if (!cs) return [];
  const items = Array.isArray(cs) ? cs : cs.items;
  return Array.isArray(items) ? items : [];
});

async function refreshAll() {
  error.value = "";
  loading.value = true;
  try {
    const jobs = [
      loadGenLogs(500),
      loadUsageSummary(appState.projectRoot || null),
      loadProviderBalance().catch((e) => {
        appState.providerBalance = {
          ok: false,
          reason: String(e.message || e),
        };
      }),
    ];
    if (appState.projectRoot) {
      jobs.push(loadProjectGenLogs(appState.projectRoot, 500));
    } else {
      appState.projectGenLogs = [];
    }
    await Promise.all(jobs);
    if (selectedId.value && !selected.value) {
      selectedId.value = "";
    }
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    loading.value = false;
  }
}

function openDetail(item) {
  if (!item || !item.id) return;
  selectedId.value = String(item.id);
}

function backToList() {
  selectedId.value = "";
}

function rowTokens(item) {
  const u = item.usage;
  if (!u) return "—";
  const p = u.prompt_tokens || 0;
  const c = u.completion_tokens || 0;
  return `${p} / ${c}`;
}

function rowCache(item) {
  const rate = cacheHitRate(item.usage);
  if (rate == null) return "—";
  return `${rate}%`;
}

function shortTs(ts) {
  const s = String(ts || "");
  if (s.length >= 19) return s.slice(0, 19).replace("T", " ");
  return s;
}

function formatBalanceNum(v) {
  const n = Number(v);
  if (Number.isFinite(n)) return n.toFixed(2);
  return String(v ?? "—");
}

onMounted(() => {
  void refreshAll();
});

watch(
  () => appState.projectRoot,
  () => {
    selectedId.value = "";
    showAllProjects.value = false;
    filterTask.value = "";
    filterModel.value = "";
    void refreshAll();
  }
);
</script>

<template>
  <section class="panel">
    <template v-if="selected">
      <div class="detail-top">
        <button type="button" class="app-btn back-btn" @click="backToList">返回列表</button>
        <h1 class="panel-heading detail-heading">生成详情</h1>
      </div>

      <div class="meta-card">
        <div class="meta-row">
          <span class="meta-label">时间</span>
          <span>{{ shortTs(selected.ts) }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">事件</span>
          <span>{{ selected.event || selected.task || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">任务</span>
          <span>{{ selected.task || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">模型</span>
          <span>{{ selected.model_used || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">来源</span>
          <span>{{ selected.source || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">章节</span>
          <span class="mono">{{ selected.chapter_id || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">作品</span>
          <span class="mono path">{{ selected.project_root || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">字数</span>
          <span>
            定稿 {{ selected.chars_final ?? "—" }}
            <template v-if="selected.chars_raw != null"> · 原始 {{ selected.chars_raw }}</template>
            <span v-if="selected.truncated" class="warn"> · 已截断</span>
          </span>
        </div>
        <div class="meta-row">
          <span class="meta-label">花费</span>
          <span>{{ formatCost(selected.cost_cny) || "—" }}</span>
        </div>
      </div>

      <h2 class="section-title">Token</h2>
      <div class="meta-card" v-if="selected.usage">
        <div class="meta-row">
          <span class="meta-label">prompt</span>
          <span>{{ selected.usage.prompt_tokens ?? 0 }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">completion</span>
          <span>{{ selected.usage.completion_tokens ?? 0 }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">total</span>
          <span>{{ usageTotalTokens(selected.usage) }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">来源</span>
          <span>{{ selected.usage.source === "api" ? "api" : "估算" }}</span>
        </div>
        <div class="meta-row" v-if="formatCacheHit(selected.usage)">
          <span class="meta-label">缓存</span>
          <span>{{ formatCacheHit(selected.usage) }}</span>
        </div>
      </div>
      <p v-else class="muted">无 token 用量</p>

      <h2 class="section-title">预览</h2>
      <pre class="preview">{{ selected.preview || selected.final_text || "（无）" }}</pre>

      <details v-if="selected.raw_text" class="raw-details">
        <summary>原始全文</summary>
        <pre class="preview">{{ selected.raw_text }}</pre>
      </details>

      <details v-if="formatMessages(selected)" class="raw-details">
        <summary>指令 / 消息</summary>
        <pre class="preview">{{ formatMessages(selected) }}</pre>
      </details>

      <template v-if="contextItems.length">
        <h2 class="section-title">上下文来源</h2>
        <ul class="ctx-list">
          <li v-for="(it, i) in contextItems" :key="i">
            <strong>{{ it.kind || "item" }}</strong>
            <span v-if="it.title"> · {{ it.title }}</span>
            <span v-if="it.detail" class="muted"> — {{ it.detail }}</span>
          </li>
        </ul>
      </template>
    </template>

    <template v-else>
      <h1 class="panel-heading">分析</h1>
      <p class="muted intro">
        AI 生成履历、花费与余额。当前数据源：{{ dataSourceLabel }}。
      </p>

      <!-- 账户余额 -->
      <div class="balance-bar">
        <div class="balance-main">
          <template v-if="balance && balance.ok">
            <span class="sum-label">DeepSeek 余额</span>
            <div class="sum-main">
              {{ balance.currency || "CNY" }} {{ formatBalanceNum(balance.total) }}
            </div>
            <div class="muted">
              赠送 {{ formatBalanceNum(balance.granted) }} · 充值
              {{ formatBalanceNum(balance.topped_up) }}
              <template v-if="balance.is_available === false"> · 余额不可用</template>
            </div>
          </template>
          <template v-else>
            <span class="sum-label">账户余额</span>
            <div class="muted">
              {{ (balance && balance.reason) || "加载中或暂不可用" }}
            </div>
          </template>
        </div>
        <button
          type="button"
          class="app-btn app-btn-info"
          :disabled="loading"
          @click="refreshAll"
        >
          {{ loading ? "刷新中…" : "刷新" }}
        </button>
      </div>

      <!-- KPI -->
      <div class="summary-grid kpi-grid">
        <div class="sum-card" :class="{ estimate: kpi.mode === 'estimate' }">
          <div class="sum-label">
            {{ kpi.mode === "estimate" ? "单次续写约算" : "当前列表" }}
            <span v-if="kpi.mode === 'estimate'" class="badge-est">估算 · 非实测</span>
          </div>
          <div class="sum-main">
            {{
              kpi.mode === "estimate"
                ? formatCost(kpi.cost)
                : formatCost(kpi.cost) || "¥0"
            }}
          </div>
          <div class="muted">
            <template v-if="kpi.mode === 'estimate'">
              ≈ {{ kpi.tokens }} tok（prompt {{ estimate.perCall.prompt }} / out
              {{ estimate.perCall.completion }}）
            </template>
            <template v-else>
              {{ kpi.tokens }} tok · {{ kpi.calls }} 次
              <template v-if="kpi.hitRate != null"> · 缓存 {{ kpi.hitRate }}%</template>
            </template>
          </div>
        </div>
        <div class="sum-card" v-if="globalSummary">
          <div class="sum-label">本应用全局累计</div>
          <div class="sum-main">{{ formatCost(globalSummary.cost) || "¥0" }}</div>
          <div class="muted">
            {{ globalSummary.tokens }} tok · {{ globalSummary.calls }} 次
            <template v-if="globalSummary.hitRate != null">
              · 缓存 {{ globalSummary.hitRate }}%
            </template>
          </div>
        </div>
        <div class="sum-card" v-if="projectSummary">
          <div class="sum-label">本作品累计（账本）</div>
          <div class="sum-main">{{ formatCost(projectSummary.cost) || "¥0" }}</div>
          <div class="muted">
            {{ projectSummary.tokens }} tok · {{ projectSummary.calls }} 次
          </div>
        </div>
      </div>

      <p v-if="kpi.mode === 'estimate'" class="muted estimate-note">{{ estimate.note }}</p>

      <div v-if="kpi.mode === 'estimate'" class="scenario-row">
        <div class="sum-card estimate">
          <div class="sum-label">约 10 次续写</div>
          <div class="sum-main">{{ formatCost(estimate.scenarios.x10.cost) }}</div>
        </div>
        <div class="sum-card estimate">
          <div class="sum-label">约 50 次续写</div>
          <div class="sum-main">{{ formatCost(estimate.scenarios.x50.cost) }}</div>
        </div>
      </div>

      <div class="metric-toggle">
        <span class="muted">图表指标</span>
        <button
          type="button"
          class="chip"
          :class="{ active: chartMetric === 'cost' }"
          @click="chartMetric = 'cost'"
        >
          花费
        </button>
        <button
          type="button"
          class="chip"
          :class="{ active: chartMetric === 'tokens' }"
          @click="chartMetric = 'tokens'"
        >
          Token
        </button>
      </div>

      <UsageTrendChart
        :series="dailySeries"
        :metric="chartMetric"
        :estimate="!hasRealData"
        :title="hasRealData ? '近 14 天趋势' : '配置情景示意'"
        :subtitle="
          hasRealData
            ? '按已加载履历按日汇总'
            : '假设每天 3 次续写的累计花费/token（非实测）'
        "
      />

      <div v-if="hasRealData" class="charts-row">
        <UsageBarChart
          :rows="modelBars"
          :metric="chartMetric === 'tokens' ? 'tokens' : 'cost'"
          title="按模型"
        />
        <UsageBarChart :rows="taskBars" metric="calls" title="按任务（次数）" />
      </div>

      <div v-if="byModelRows.length && hasRealData" class="model-table-wrap">
        <h2 class="section-title">账本 · 按模型</h2>
        <table class="model-table">
          <thead>
            <tr>
              <th>模型</th>
              <th>调用</th>
              <th>token</th>
              <th>缓存</th>
              <th>花费</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in byModelRows" :key="row.name">
              <td class="mono">{{ row.name }}</td>
              <td>{{ row.calls }}</td>
              <td>{{ row.tokens }}</td>
              <td>{{ row.hitRate != null ? row.hitRate + "%" : "—" }}</td>
              <td>{{ formatCost(row.cost) }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="toolbar">
        <CapsuleSwitch v-model="showAllProjects" label="包含其他作品" />
        <label class="filter-field">
          <span class="muted">任务</span>
          <select v-model="filterTask">
            <option value="">全部</option>
            <option v-for="t in taskOptions" :key="t" :value="t">{{ t }}</option>
          </select>
        </label>
        <label class="filter-field">
          <span class="muted">模型</span>
          <select v-model="filterModel">
            <option value="">全部</option>
            <option v-for="m in modelOptions" :key="m" :value="m">{{ m }}</option>
          </select>
        </label>
      </div>

      <p class="muted list-count">共 {{ visibleLogs.length }} 条</p>

      <button
        v-for="item in visibleLogs"
        :key="item.id"
        type="button"
        class="log-row"
        @click="openDetail(item)"
      >
        <div class="log-row-top">
          <span class="ts">{{ shortTs(item.ts) }}</span>
          <span class="task">
            {{ item.event && item.event !== item.task ? item.event + " · " : ""
            }}{{ item.task }}
          </span>
          <span v-if="item.model_used" class="model mono">{{ item.model_used }}</span>
          <span v-if="item.truncated" class="warn">截断</span>
        </div>
        <div class="log-row-stats muted">
          字数 {{ item.chars_final ?? "—" }}
          · prompt/comp {{ rowTokens(item) }}
          · 命中 {{ rowCache(item) }}
          <template v-if="item.cost_cny != null"> · {{ formatCost(item.cost_cny) }}</template>
          <template v-if="item.usage"> · {{ formatTokens(item.usage) }}</template>
        </div>
        <div class="log-row-preview">{{ item.preview || "（无预览）" }}</div>
      </button>

      <p v-if="!loading && !visibleLogs.length" class="muted empty">
        {{
          showAllProjects
            ? "暂无生成记录；上方为按当前配置的约算。"
            : appState.projectRoot
              ? "当前作品暂无履历；上方为配置约算。新生成后会出现在 gen_activity.jsonl。"
              : "请先打开作品，或勾选「包含其他作品」。上方为配置约算。"
        }}
      </p>
    </template>
  </section>
</template>

<style scoped>
.panel {
  min-height: calc(100% - 8px);
}
.intro {
  margin: 0 0 12px;
  font-size: 13px;
  line-height: 1.5;
}
.balance-bar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px;
  margin-bottom: 12px;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
}
.balance-main {
  min-width: 0;
}
.estimate-note {
  font-size: 12px;
  margin: 0 0 10px;
  line-height: 1.45;
}
.scenario-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  margin-bottom: 12px;
}
.kpi-grid {
  margin-bottom: 8px;
}
.sum-card.estimate {
  border: 1px dashed var(--warning, #c27803);
}
.badge-est {
  margin-left: 6px;
  font-size: 11px;
  font-weight: 600;
  color: var(--warning, #c27803);
}
.metric-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 4px 0 0;
  flex-wrap: wrap;
}
.chip {
  border: 1px solid var(--divider);
  background: var(--surface-solid);
  color: inherit;
  border-radius: 999px;
  padding: 4px 12px;
  font-size: 12px;
  cursor: pointer;
}
.chip.active {
  border-color: var(--accent-hover);
  color: var(--accent-hover);
  font-weight: 600;
}
.charts-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
@media (max-width: 900px) {
  .charts-row,
  .scenario-row {
    grid-template-columns: 1fr;
  }
}
.detail-top {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
  flex-wrap: wrap;
}
.back-btn {
  flex-shrink: 0;
}
.detail-heading {
  margin: 0;
}
.section-title {
  margin: 18px 0 8px;
  font-size: 14px;
  font-weight: 700;
  color: var(--text);
}
.summary-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  margin-bottom: 12px;
}
@media (max-width: 720px) {
  .summary-grid {
    grid-template-columns: 1fr;
  }
}
.sum-card {
  padding: 14px 16px;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
}
.sum-label {
  font-size: 12px;
  font-weight: 700;
  color: var(--muted, #888);
  margin-bottom: 4px;
}
.sum-main {
  font-size: 18px;
  font-weight: 700;
  color: var(--accent-hover);
}
.model-table-wrap {
  margin-bottom: 12px;
  overflow-x: auto;
}
.model-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
  background: var(--surface-solid);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-sm);
}
.model-table th,
.model-table td {
  text-align: left;
  padding: 8px 12px;
  border-bottom: 1px solid var(--divider);
}
.model-table th {
  font-size: 12px;
  color: var(--muted, #888);
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 10px 16px;
  flex-wrap: wrap;
  margin: 12px 0 8px;
  padding-top: 12px;
  border-top: 1px solid var(--divider);
}
.filter-field {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}
.filter-field select {
  min-width: 120px;
  max-width: 200px;
}
.list-count {
  margin: 0 0 8px;
  font-size: 12px;
}
.log-row {
  display: block;
  width: 100%;
  text-align: left;
  margin: 0 0 10px;
  padding: 14px 16px;
  border: none;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
  cursor: pointer;
  color: inherit;
  font: inherit;
  transition: box-shadow 0.15s ease, background 0.15s ease;
}
.log-row:hover {
  box-shadow: var(--shadow);
  background: var(--accent-soft, var(--panel-2));
}
.log-row-top {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 10px;
  align-items: baseline;
  font-weight: 700;
  color: var(--accent-hover);
  margin-bottom: 4px;
  font-size: 13px;
}
.log-row-stats {
  font-size: 12px;
  margin-bottom: 6px;
  line-height: 1.45;
}
.log-row-preview {
  font-size: 13px;
  line-height: 1.5;
  color: var(--text);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.meta-card {
  padding: 14px 16px;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
}
.meta-row {
  display: grid;
  grid-template-columns: 96px 1fr;
  gap: 8px;
  padding: 4px 0;
  font-size: 13px;
  line-height: 1.45;
}
.meta-label {
  color: var(--muted, #888);
  font-weight: 600;
}
.preview {
  white-space: pre-wrap;
  font-size: 12px;
  margin: 0;
  padding: 12px 14px;
  border-radius: var(--radius-md);
  background: var(--panel-2);
  color: var(--text);
  font-family: var(--font-mono);
  box-shadow: var(--shadow-sm);
  max-height: 480px;
  overflow: auto;
}
.raw-details {
  margin-top: 10px;
}
.ctx-list {
  margin: 0;
  padding-left: 1.2em;
  font-size: 13px;
  line-height: 1.55;
}
.mono {
  font-family: var(--font-mono);
  font-size: 12px;
}
.path {
  word-break: break-all;
}
.warn {
  color: var(--warning, #c27803);
  font-weight: 600;
}
.empty {
  margin-top: 16px;
}
</style>
