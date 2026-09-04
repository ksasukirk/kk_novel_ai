<!--
  数据分析：列表=作品整体统计；详情=按章节；条目详情=单次生成
  代码路径: kk_novel_ai/src/views/UsageAnalyticsView.vue
-->
<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import {
  backfillUsageCosts,
  loadGenLogs,
  loadProjectGenLogs,
  loadProviderBalance,
  loadUsageSummary,
  listNovelsProjects,
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
  aggregateByChapter,
  aggregateByModel,
  aggregateByProject,
  aggregateByTask,
  buildDailySeries,
  summarizeLogs,
} from "../utils/usageSeries.js";
import { estimateFromSettings } from "../utils/usageEstimate.js";

const error = useToastError();
const loading = ref(false);
const showAllProjects = ref(true);
const filterTask = ref("");
const filterModel = ref("");
/** 作品详情：选中的 project_root */
const selectedRoot = ref("");
/** 章节详情：选中的 chapter_id（含 _project） */
const selectedChapterId = ref("");
/** 单次生成详情 id */
const selectedLogId = ref("");
const chartMetric = ref("cost");
/** 各层列表当前页（1-based） */
const listPage = ref(1);
const chapterPage = ref(1);
const logPage = ref(1);

const pageSize = computed(() => {
  const n = Number(appState.settings && appState.settings.analytics_page_size);
  if (!Number.isFinite(n) || n < 1) return 10;
  return Math.min(200, Math.max(1, Math.floor(n)));
});

function paginate(rows, page) {
  const size = pageSize.value;
  const total = Array.isArray(rows) ? rows.length : 0;
  const pages = Math.max(1, Math.ceil(total / size) || 1);
  const p = Math.min(Math.max(1, page), pages);
  const start = (p - 1) * size;
  return {
    total,
    pages,
    page: p,
    items: (rows || []).slice(start, start + size),
  };
}

const dataSourceLabel = computed(() =>
  showAllProjects.value ? "全部作品（全局履历聚合）" : "仅当前作品"
);

const sourceLogs = computed(() => {
  const global = Array.isArray(appState.genLogs) ? appState.genLogs : [];
  if (showAllProjects.value) return global;
  const root = String(appState.projectRoot || "");
  if (!root) return [];
  const local = Array.isArray(appState.projectGenLogs) ? appState.projectGenLogs : [];
  if (local.length) return local;
  return global.filter((item) => String(item.project_root || "") === root);
});

const filteredLogs = computed(() => {
  let logs = [...sourceLogs.value];
  const task = filterTask.value.trim();
  if (task) logs = logs.filter((item) => String(item.task || "") === task);
  const model = filterModel.value.trim();
  if (model) logs = logs.filter((item) => String(item.model_used || "") === model);
  return logs;
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

function normPath(p) {
  return String(p || "")
    .replace(/\\/g, "/")
    .replace(/\/+$/, "")
    .toLowerCase();
}

function resolveProjectTitle(root) {
  const r = String(root || "");
  if (!r || r === "(none)") return "未关联作品";
  const catalog = Array.isArray(appState.analyticsProjects) ? appState.analyticsProjects : [];
  const cat = catalog.find((p) => normPath(p.root) === normPath(r));
  if (cat && cat.title) return cat.title;
  const recent = (appState.settings && appState.settings.recent_projects) || [];
  const hit = recent.find((p) => normPath(p.path) === normPath(r));
  if (hit && hit.title) return hit.title;
  if (r === appState.projectRoot && appState.project && appState.project.title) {
    return appState.project.title;
  }
  const parts = r.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] || r;
}

function emptyProjectRow(root, title) {
  return {
    root,
    title: title || resolveProjectTitle(root),
    cost: 0,
    tokens: 0,
    prompt: 0,
    completion: 0,
    calls: 0,
    hit: 0,
    miss: 0,
    hitRate: null,
    chapterCount: 0,
    lastTs: "",
    firstTs: "",
  };
}

/** 列表：作品目录清单 + 履历/账本整体统计（无履历也显示，便于分页） */
const projectRows = computed(() => {
  const byKey = new Map();
  const ensure = (root, title) => {
    const key = normPath(root) || "(none)";
    let row = byKey.get(key);
    if (!row) {
      row = emptyProjectRow(root || "(none)", title);
      byKey.set(key, row);
    } else if (title && (!row.title || row.title === row.root)) {
      row.title = title;
    }
    return row;
  };

  const catalog = Array.isArray(appState.analyticsProjects) ? appState.analyticsProjects : [];
  if (showAllProjects.value) {
    for (const p of catalog) {
      if (p && p.root) ensure(p.root, p.title);
    }
  } else {
    const root = String(appState.projectRoot || "");
    if (root) {
      const hit = catalog.find((p) => normPath(p.root) === normPath(root));
      ensure(root, (hit && hit.title) || resolveProjectTitle(root));
    }
  }

  const fromLogs = aggregateByProject(filteredLogs.value);
  for (const agg of fromLogs) {
    const row = ensure(agg.root, resolveProjectTitle(agg.root));
    row.cost = agg.cost;
    row.tokens = agg.tokens;
    row.prompt = agg.prompt;
    row.completion = agg.completion;
    row.calls = agg.calls;
    row.hit = agg.hit;
    row.miss = agg.miss;
    row.hitRate = agg.hitRate;
    row.chapterCount = agg.chapterCount;
    row.lastTs = agg.lastTs;
    row.firstTs = agg.firstTs;
  }

  const ledgerMap = (appState.usageSummary && appState.usageSummary.by_project) || {};
  for (const [root, b] of Object.entries(ledgerMap)) {
    if (!root || root === "(none)") continue;
    if (!showAllProjects.value) {
      const cur = String(appState.projectRoot || "");
      if (normPath(root) !== normPath(cur)) continue;
    }
    const row = ensure(root, resolveProjectTitle(root));
    const ledgerCalls = b.calls || 0;
    const ledgerCost = b.cost_cny || 0;
    const ledgerTokens = bucketTotalTokens(b);
    // 账本覆盖全量履历；列表 KPI 优先用账本（避免只加载最近 N 条导致旧作花费仍为 0）
    if (ledgerCalls > 0 && (row.calls <= 0 || ledgerCalls >= row.calls)) {
      row.calls = ledgerCalls;
      row.cost = ledgerCost;
      row.tokens = ledgerTokens;
      row.hit = b.prompt_cache_hit_tokens || 0;
      row.miss = b.prompt_cache_miss_tokens || 0;
      row.hitRate = bucketCacheRate(b);
    } else if (row.cost <= 0 && ledgerCost > 0) {
      row.cost = ledgerCost;
    }
  }

  const rows = [...byKey.values()];
  rows.sort((a, b) => {
    const ts = String(b.lastTs || "").localeCompare(String(a.lastTs || ""));
    if (ts) return ts;
    return String(a.title || "").localeCompare(String(b.title || ""), "zh");
  });
  return rows;
});

const projectPageInfo = computed(() => paginate(projectRows.value, listPage.value));
const pagedProjectRows = computed(() => projectPageInfo.value.items);

const projectLogs = computed(() => {
  const root = selectedRoot.value;
  if (!root) return [];
  return filteredLogs.value.filter((item) => {
    const r = String(item.project_root || "").trim() || "(none)";
    return r === root;
  });
});

const chapterRows = computed(() => aggregateByChapter(projectLogs.value));

const chapterPageInfo = computed(() => paginate(chapterRows.value, chapterPage.value));
const pagedChapterRows = computed(() => chapterPageInfo.value.items);

const chapterLogs = computed(() => {
  const cid = selectedChapterId.value;
  if (!cid) return [];
  return projectLogs.value
    .filter((item) => {
      const raw = String(item.chapter_id || "").trim() || "_project";
      return raw === cid;
    })
    .sort((a, b) => String(b.ts || "").localeCompare(String(a.ts || "")));
});

const logPageInfo = computed(() => paginate(chapterLogs.value, logPage.value));
const pagedChapterLogs = computed(() => logPageInfo.value.items);

const selectedLog = computed(() => {
  const id = selectedLogId.value;
  if (!id) return null;
  return (
    filteredLogs.value.find((x) => String(x.id) === String(id)) ||
    sourceLogs.value.find((x) => String(x.id) === String(id)) ||
    null
  );
});

const selectedChapter = computed(() => {
  const id = selectedChapterId.value;
  if (!id) return null;
  return chapterRows.value.find((c) => c.chapterId === id) || null;
});

const selectedProject = computed(() => {
  const root = selectedRoot.value;
  if (!root) return null;
  return projectRows.value.find((p) => p.root === root) || null;
});

/** 当前层级用于图表的日志 */
const chartLogs = computed(() => {
  if (selectedChapterId.value) return chapterLogs.value;
  if (selectedRoot.value) return projectLogs.value;
  return filteredLogs.value;
});

const hasRealData = computed(() => chartLogs.value.length > 0);

const estimate = computed(() => estimateFromSettings(appState.settings));

const kpi = computed(() => {
  if (hasRealData.value) {
    return { ...summarizeLogs(chartLogs.value), mode: "real" };
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
  if (hasRealData.value) return buildDailySeries(chartLogs.value, 14);
  return estimate.value.scenarioDaily;
});

const modelBars = computed(() =>
  hasRealData.value ? aggregateByModel(chartLogs.value) : []
);

const taskBars = computed(() =>
  hasRealData.value ? aggregateByTask(chartLogs.value) : []
);

const chapterBars = computed(() =>
  chapterRows.value.map((c) => ({
    name: c.label,
    cost: c.cost,
    tokens: c.tokens,
    calls: c.calls,
  }))
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
  };
});

const balance = computed(() => appState.providerBalance || null);

const contextItems = computed(() => {
  const cs = selectedLog.value && selectedLog.value.context_sources;
  if (!cs) return [];
  const items = Array.isArray(cs) ? cs : cs.items;
  return Array.isArray(items) ? items : [];
});

/** list | project | chapter | log */
const viewMode = computed(() => {
  if (selectedLogId.value && selectedLog.value) return "log";
  if (selectedChapterId.value) return "chapter";
  if (selectedRoot.value) return "project";
  return "list";
});

async function refreshAll() {
  error.value = "";
  loading.value = true;
  try {
    // 先按当前单价补齐历史花费并写回作品目录，再拉列表
    await backfillUsageCosts().catch(() => null);
    const jobs = [
      loadGenLogs(2000),
      loadUsageSummary(appState.projectRoot || null),
      listNovelsProjects()
        .then((r) => {
          appState.analyticsProjects = (r && r.items) || [];
        })
        .catch(() => {
          appState.analyticsProjects = [];
        }),
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
    if (selectedRoot.value && !projectRows.value.some((p) => p.root === selectedRoot.value)) {
      selectedRoot.value = "";
      selectedChapterId.value = "";
      selectedLogId.value = "";
    }
    if (selectedLogId.value && !selectedLog.value) {
      selectedLogId.value = "";
    }
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    loading.value = false;
  }
}

function openProject(row) {
  if (!row || !row.root) return;
  selectedRoot.value = row.root;
  selectedChapterId.value = "";
  selectedLogId.value = "";
  chapterPage.value = 1;
  logPage.value = 1;
}

function openChapter(row) {
  if (!row || !row.chapterId) return;
  selectedChapterId.value = row.chapterId;
  selectedLogId.value = "";
  logPage.value = 1;
}

function openLog(item) {
  if (!item || !item.id) return;
  selectedLogId.value = String(item.id);
}

function backOne() {
  if (viewMode.value === "log") {
    selectedLogId.value = "";
    return;
  }
  if (viewMode.value === "chapter") {
    selectedChapterId.value = "";
    logPage.value = 1;
    return;
  }
  if (viewMode.value === "project") {
    selectedRoot.value = "";
    selectedChapterId.value = "";
    selectedLogId.value = "";
    chapterPage.value = 1;
    logPage.value = 1;
  }
}

function rowTokens(item) {
  const u = item.usage;
  if (!u) return "—";
  return `${u.prompt_tokens || 0} / ${u.completion_tokens || 0}`;
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
    selectedRoot.value = "";
    selectedChapterId.value = "";
    selectedLogId.value = "";
    filterTask.value = "";
    filterModel.value = "";
    listPage.value = 1;
    chapterPage.value = 1;
    logPage.value = 1;
    void refreshAll();
  }
);

watch([showAllProjects, filterTask, filterModel, pageSize], () => {
  listPage.value = 1;
});

watch(projectPageInfo, (info) => {
  if (listPage.value > info.pages) listPage.value = info.pages;
});
watch(chapterPageInfo, (info) => {
  if (chapterPage.value > info.pages) chapterPage.value = info.pages;
});
watch(logPageInfo, (info) => {
  if (logPage.value > info.pages) logPage.value = info.pages;
});
</script>

<template>
  <section class="panel">
    <!-- 单次生成详情 -->
    <template v-if="viewMode === 'log' && selectedLog">
      <div class="detail-top">
        <button type="button" class="app-btn back-btn" @click="backOne">返回章节</button>
        <h1 class="panel-heading detail-heading">生成详情</h1>
      </div>

      <div class="meta-card">
        <div class="meta-row">
          <span class="meta-label">时间</span>
          <span>{{ shortTs(selectedLog.ts) }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">任务</span>
          <span>{{ selectedLog.task || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">模型</span>
          <span>{{ selectedLog.model_used || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">章节</span>
          <span class="mono">{{ selectedLog.chapter_id || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">作品</span>
          <span class="mono path">{{ selectedLog.project_root || "—" }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">花费</span>
          <span>{{ formatCost(selectedLog.cost_cny) || "—" }}</span>
        </div>
      </div>

      <h2 class="section-title">Token</h2>
      <div class="meta-card" v-if="selectedLog.usage">
        <div class="meta-row">
          <span class="meta-label">prompt</span>
          <span>{{ selectedLog.usage.prompt_tokens ?? 0 }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">completion</span>
          <span>{{ selectedLog.usage.completion_tokens ?? 0 }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">total</span>
          <span>{{ usageTotalTokens(selectedLog.usage) }}</span>
        </div>
        <div class="meta-row">
          <span class="meta-label">来源</span>
          <span>{{ selectedLog.usage.source === "api" ? "api" : "估算" }}</span>
        </div>
        <div class="meta-row" v-if="formatCacheHit(selectedLog.usage)">
          <span class="meta-label">缓存</span>
          <span>{{ formatCacheHit(selectedLog.usage) }}</span>
        </div>
      </div>

      <h2 class="section-title">预览</h2>
      <pre class="preview">{{ selectedLog.preview || selectedLog.final_text || "（无）" }}</pre>

      <details v-if="selectedLog.raw_text" class="raw-details">
        <summary>原始全文</summary>
        <pre class="preview">{{ selectedLog.raw_text }}</pre>
      </details>

      <details v-if="formatMessages(selectedLog)" class="raw-details">
        <summary>指令 / 消息</summary>
        <pre class="preview">{{ formatMessages(selectedLog) }}</pre>
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

    <!-- 章节详情：该章各次生成 -->
    <template v-else-if="viewMode === 'chapter'">
      <div class="detail-top">
        <button type="button" class="app-btn back-btn" @click="backOne">返回作品</button>
        <h1 class="panel-heading detail-heading">
          章节 · {{ selectedChapter ? selectedChapter.label : selectedChapterId }}
        </h1>
      </div>
      <p class="muted intro">
        {{ resolveProjectTitle(selectedRoot) }} · 本章整体
        {{ selectedChapter ? formatCost(selectedChapter.cost) : "" }}
        · {{ selectedChapter ? selectedChapter.calls : 0 }} 次
      </p>

      <div class="summary-grid kpi-grid" v-if="selectedChapter">
        <div class="sum-card">
          <div class="sum-label">本章花费</div>
          <div class="sum-main">{{ formatCost(selectedChapter.cost) || "¥0" }}</div>
          <div class="muted">{{ selectedChapter.tokens }} tok · {{ selectedChapter.calls }} 次</div>
        </div>
        <div class="sum-card">
          <div class="sum-label">缓存命中</div>
          <div class="sum-main">
            {{ selectedChapter.hitRate != null ? selectedChapter.hitRate + "%" : "—" }}
          </div>
        </div>
      </div>

      <p class="muted list-count">
        共 {{ chapterLogs.length }} 次生成 · 第 {{ logPageInfo.page }}/{{ logPageInfo.pages }} 页（每页
        {{ pageSize }}）
      </p>
      <button
        v-for="item in pagedChapterLogs"
        :key="item.id"
        type="button"
        class="log-row"
        @click="openLog(item)"
      >
        <div class="log-row-top">
          <span class="task">{{ item.task }}</span>
          <span v-if="item.model_used" class="model mono">{{ item.model_used }}</span>
          <span class="ts">{{ shortTs(item.ts) }}</span>
        </div>
        <div class="log-row-stats muted">
          prompt/comp {{ rowTokens(item) }}
          · 命中 {{ rowCache(item) }}
          <template v-if="item.cost_cny != null"> · {{ formatCost(item.cost_cny) }}</template>
          <template v-if="item.usage"> · {{ formatTokens(item.usage) }}</template>
        </div>
        <div class="log-row-preview">{{ item.preview || "（无预览）" }}</div>
      </button>
      <div v-if="chapterLogs.length" class="pager">
        <button
          type="button"
          class="app-btn"
          :disabled="logPageInfo.page <= 1"
          @click="logPage = logPageInfo.page - 1"
        >
          上一页
        </button>
        <span class="muted">{{ logPageInfo.page }} / {{ logPageInfo.pages }}</span>
        <button
          type="button"
          class="app-btn"
          :disabled="logPageInfo.page >= logPageInfo.pages"
          @click="logPage = logPageInfo.page + 1"
        >
          下一页
        </button>
      </div>
      <p v-if="!chapterLogs.length" class="muted empty">本章暂无履历（或被筛选过滤）。</p>
    </template>

    <!-- 作品详情：按章节整体 -->
    <template v-else-if="viewMode === 'project'">
      <div class="detail-top">
        <button type="button" class="app-btn back-btn" @click="backOne">返回作品列表</button>
        <h1 class="panel-heading detail-heading">
          {{ selectedProject ? selectedProject.title : resolveProjectTitle(selectedRoot) }}
        </h1>
      </div>
      <p class="muted intro path">{{ selectedRoot }}</p>

      <div class="summary-grid kpi-grid" v-if="selectedProject">
        <div class="sum-card">
          <div class="sum-label">作品整体花费</div>
          <div class="sum-main">{{ formatCost(selectedProject.cost) || "¥0" }}</div>
          <div class="muted">
            {{ selectedProject.tokens }} tok · {{ selectedProject.calls }} 次 ·
            {{ selectedProject.chapterCount }} 章有记录
          </div>
        </div>
        <div class="sum-card">
          <div class="sum-label">缓存命中</div>
          <div class="sum-main">
            {{ selectedProject.hitRate != null ? selectedProject.hitRate + "%" : "—" }}
          </div>
          <div class="muted">最近 {{ shortTs(selectedProject.lastTs) || "—" }}</div>
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
        :estimate="false"
        title="本作品近 14 天"
        subtitle="按该作品履历按日汇总"
      />

      <div class="charts-row" v-if="chapterBars.length">
        <UsageBarChart
          :rows="chapterBars"
          :metric="chartMetric === 'tokens' ? 'tokens' : 'cost'"
          title="按章节"
        />
        <UsageBarChart :rows="taskBars" metric="calls" title="按任务（次数）" />
      </div>

      <h2 class="section-title">章节（点击进入该章记录）</h2>
      <p class="muted list-count">
        共 {{ chapterRows.length }} 个章节维度 · 第 {{ chapterPageInfo.page }}/{{ chapterPageInfo.pages }} 页（每页
        {{ pageSize }}）
      </p>
      <button
        v-for="ch in pagedChapterRows"
        :key="ch.chapterId"
        type="button"
        class="log-row"
        @click="openChapter(ch)"
      >
        <div class="log-row-top">
          <span class="task">{{ ch.label }}</span>
          <span class="ts">{{ shortTs(ch.lastTs) }}</span>
        </div>
        <div class="log-row-stats muted">
          {{ ch.calls }} 次 · {{ ch.tokens }} tok
          <template v-if="ch.hitRate != null"> · 缓存 {{ ch.hitRate }}%</template>
          · {{ formatCost(ch.cost) || "¥0" }}
        </div>
      </button>
      <div v-if="chapterRows.length" class="pager">
        <button
          type="button"
          class="app-btn"
          :disabled="chapterPageInfo.page <= 1"
          @click="chapterPage = chapterPageInfo.page - 1"
        >
          上一页
        </button>
        <span class="muted">{{ chapterPageInfo.page }} / {{ chapterPageInfo.pages }}</span>
        <button
          type="button"
          class="app-btn"
          :disabled="chapterPageInfo.page >= chapterPageInfo.pages"
          @click="chapterPage = chapterPageInfo.page + 1"
        >
          下一页
        </button>
      </div>
      <p v-if="!chapterRows.length" class="muted empty">该作品暂无章节履历。</p>
    </template>

    <!-- 列表：作品整体 -->
    <template v-else>
      <h1 class="panel-heading">分析</h1>
      <p class="muted intro">
        列表按<strong>作品整体</strong>汇总；进入作品后再看各章节。数据源：{{ dataSourceLabel }}。
      </p>

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
            </div>
          </template>
          <template v-else>
            <span class="sum-label">账户余额</span>
            <div class="muted">{{ (balance && balance.reason) || "加载中或暂不可用" }}</div>
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

      <div class="summary-grid kpi-grid">
        <div class="sum-card" :class="{ estimate: kpi.mode === 'estimate' }">
          <div class="sum-label">
            {{ kpi.mode === "estimate" ? "单次续写约算" : "当前范围合计" }}
            <span v-if="kpi.mode === 'estimate'" class="badge-est">估算 · 非实测</span>
          </div>
          <div class="sum-main">{{ formatCost(kpi.cost) || "¥0" }}</div>
          <div class="muted">
            <template v-if="kpi.mode === 'estimate'">
              ≈ {{ kpi.tokens }} tok（prompt {{ estimate.perCall.prompt }} / out
              {{ estimate.perCall.completion }}）
            </template>
            <template v-else>
              {{ kpi.tokens }} tok · {{ kpi.calls }} 次 · {{ projectRows.length }} 部作品
              <template v-if="kpi.hitRate != null"> · 缓存 {{ kpi.hitRate }}%</template>
            </template>
          </div>
        </div>
        <div class="sum-card" v-if="globalSummary">
          <div class="sum-label">本应用全局累计</div>
          <div class="sum-main">{{ formatCost(globalSummary.cost) || "¥0" }}</div>
          <div class="muted">{{ globalSummary.tokens }} tok · {{ globalSummary.calls }} 次</div>
        </div>
        <div class="sum-card" v-if="projectSummary">
          <div class="sum-label">当前打开作品（账本）</div>
          <div class="sum-main">{{ formatCost(projectSummary.cost) || "¥0" }}</div>
          <div class="muted">{{ projectSummary.tokens }} tok · {{ projectSummary.calls }} 次</div>
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

      <h2 class="section-title">作品（整体统计）</h2>
      <p class="muted list-count">
        共 {{ projectRows.length }} 部 · 第 {{ projectPageInfo.page }}/{{ projectPageInfo.pages }} 页（每页
        {{ pageSize }}）
      </p>

      <button
        v-for="row in pagedProjectRows"
        :key="row.root"
        type="button"
        class="log-row"
        @click="openProject(row)"
      >
        <div class="log-row-top">
          <span class="task">{{ row.title }}</span>
          <span class="ts">{{ shortTs(row.lastTs) }}</span>
        </div>
        <div class="log-row-stats muted">
          {{ row.calls }} 次 · {{ row.tokens }} tok · {{ row.chapterCount }} 章
          <template v-if="row.hitRate != null"> · 缓存 {{ row.hitRate }}%</template>
          · {{ formatCost(row.cost) || "¥0" }}
        </div>
        <div class="log-row-preview mono path">{{ row.root }}</div>
      </button>

      <div v-if="projectRows.length" class="pager">
        <button
          type="button"
          class="app-btn"
          :disabled="projectPageInfo.page <= 1"
          @click="listPage = projectPageInfo.page - 1"
        >
          上一页
        </button>
        <span class="muted">{{ projectPageInfo.page }} / {{ projectPageInfo.pages }}</span>
        <button
          type="button"
          class="app-btn"
          :disabled="projectPageInfo.page >= projectPageInfo.pages"
          @click="listPage = projectPageInfo.page + 1"
        >
          下一页
        </button>
      </div>

      <p v-if="!loading && !projectRows.length" class="muted empty">
        {{
          showAllProjects
            ? "未在 novels 目录 / 最近列表发现作品；上方为按当前配置的约算。"
            : appState.projectRoot
              ? "当前作品暂无条目；上方为配置约算。"
              : "请先打开作品，或勾选「包含其他作品」。"
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
.pager {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  margin: 12px 0 20px;
  padding: 10px 0;
  flex-wrap: wrap;
  border-top: 1px solid var(--divider);
}
.pager .app-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
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
  flex-wrap: nowrap;
  align-items: baseline;
  gap: 10px;
  font-weight: 700;
  color: var(--accent-hover);
  margin-bottom: 4px;
  font-size: 13px;
  width: 100%;
}
.log-row-top .task {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.log-row-top .ts {
  flex: 0 0 auto;
  margin-left: auto;
  font-weight: 600;
  font-size: 12px;
  opacity: 0.85;
  white-space: nowrap;
}
.log-row-top .model {
  flex: 0 0 auto;
  font-weight: 500;
  opacity: 0.75;
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
