<!--
  情节库：全局情节套路 / 性癖（不依赖作品）
  代码路径: kk_novel_ai/src/views/TropeLibraryView.vue
-->
<script setup>
import { computed, nextTick, onActivated, onMounted, onUnmounted, ref, watch } from "vue";
import { appState, bumpTropeRevision } from "../stores/appState.js";
import * as project from "../services/projectClient.js";
import * as kb from "../services/kbClient.js";
import { appConfirm, appConfirmDelete } from "../services/confirmDialog.js";
import { useToastError } from "../services/toast.js";
import { t } from "../i18n/index.js";
import { isTropeKind, tropeKindLabelKey } from "../utils/tropeKinds.js";
import { cancelTropeScan, formatScanUsage, scanTropesFromRoot, tropeScanState } from "../services/tropeScan.js";
import {
  TROPE_CARD_FIELD_IDS,
  readTropeCardFields,
  saveTropeCardFields,
  readTropeCardDensity,
  saveTropeCardDensity,
} from "../utils/layoutPrefs.js";
import {
  TROPE_CATEGORY_IDS,
  categoryLabelKey,
  formatTropeTags,
  itemCategoryTags,
  itemIsUncategorized,
  parseTropeTags,
} from "../utils/tropeCategories.js";
import { tropesScanConfirmText } from "../utils/usageEstimate.js";

const items = ref([]);
const rosterPath = ref("");
const error = useToastError();
const status = ref("");
/** all | trope | kink */
const kindFilter = ref("all");
const searchQuery = ref("");
/** "" | "__uncat__" | 规范名 */
const categoryFilter = ref("");
const form = ref(emptyForm("trope"));
const importTitle = ref("");
const editorOpen = ref(false);
const expandedId = ref("");
const showFieldPanel = ref(false);
const cardFields = ref(readTropeCardFields());
const cardDensity = ref(readTropeCardDensity());
const scan = tropeScanState;
let applyingRemote = false;
let saveTimer = null;
let scanRefreshTimer = null;

const chapterCount = computed(() => {
  const ch = (appState.project && appState.project.chapters) || [];
  return ch.length;
});

const canScanCurrent = computed(
  () => !!appState.projectRoot && chapterCount.value > 0 && !scan.running
);

function emptyForm(kind) {
  return {
    id: "",
    kind: kind === "kink" ? "kink" : "trope",
    title: "",
    content: "",
    keywords: "",
    intensity: "3",
    doText: "",
    dontText: "",
    tags: "",
  };
}

const libraryItems = computed(() =>
  (items.value || []).filter((it) => isTropeKind(it.kind))
);

const kindItems = computed(() => {
  if (kindFilter.value === "all") return libraryItems.value;
  return libraryItems.value.filter((it) => it.kind === kindFilter.value);
});

const categoryCounts = computed(() => {
  const counts = {};
  let uncat = 0;
  for (const it of kindItems.value) {
    const tags = itemCategoryTags(it);
    if (!tags.length) uncat += 1;
    for (const tag of tags) {
      counts[tag] = (counts[tag] || 0) + 1;
    }
  }
  return {
    chips: TROPE_CATEGORY_IDS.filter((id) => counts[id] > 0).map((id) => ({
      id,
      n: counts[id],
    })),
    uncat,
  };
});

const visibleItems = computed(() => {
  let list = kindItems.value;
  if (categoryFilter.value === "__uncat__") {
    list = list.filter((it) => itemIsUncategorized(it));
  } else if (categoryFilter.value) {
    list = list.filter((it) => itemCategoryTags(it).includes(categoryFilter.value));
  }
  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((it) => {
      const hay = [
        it.title || "",
        (it.keywords || []).join(" "),
        it.content || "",
        itemCategoryTags(it).join(" "),
        String((it.attrs && it.attrs.tags) || ""),
      ]
        .join(" ")
        .toLowerCase();
      return hay.includes(q);
    });
  }
  return list;
});

const visibleCountText = computed(() =>
  t("trope.visibleCount", { n: visibleItems.value.length, m: kindItems.value.length })
);

function attrsOf(item) {
  return item && item.attrs && typeof item.attrs === "object" ? item.attrs : {};
}

function keywordChips(item) {
  return (item.keywords || []).filter(Boolean).slice(0, 8);
}

function formTagSelected(id) {
  return parseTropeTags(form.value.tags).includes(id);
}

function toggleFormTag(id) {
  const cur = parseTropeTags(form.value.tags);
  const next = cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id];
  form.value.tags = formatTropeTags(next);
}

function toggleCategory(id) {
  categoryFilter.value = categoryFilter.value === id ? "" : id;
}

function toggleField(id) {
  cardFields.value = { ...cardFields.value, [id]: !cardFields.value[id] };
  saveTropeCardFields(cardFields.value);
}

function setDensity(d) {
  cardDensity.value = d === "compact" ? "compact" : "standard";
  saveTropeCardDensity(cardDensity.value);
}

function toggleExpand(id) {
  expandedId.value = expandedId.value === id ? "" : id;
}

function closeEditor() {
  editorOpen.value = false;
}

function onListPaneClick() {
  if (editorOpen.value) editorOpen.value = false;
}

function fieldOn(id) {
  return !!cardFields.value[id];
}

function showDoDont(item) {
  if (expandedId.value === item.id) return true;
  return fieldOn("doDont") && !!(attrsOf(item).do || attrsOf(item).dont);
}

function showEvidence(item) {
  if (expandedId.value === item.id) return !!attrsOf(item).evidence;
  return fieldOn("evidence") && !!attrsOf(item).evidence;
}

async function refresh() {
  error.value = "";
  try {
    const ens = await project.ensureTropeLibrary();
    rosterPath.value = ens.root || "";
    if (!rosterPath.value) throw new Error(t("trope.noRoster"));
    const r = await project.listLoreAt(rosterPath.value);
    items.value = r.items || [];
  } catch (e) {
    error.value = String(e.message || e);
    items.value = [];
  }
}

async function edit(item) {
  if (expandedId.value && expandedId.value !== item.id) expandedId.value = "";
  applyingRemote = true;
  const attrs = attrsOf(item);
  form.value = {
    id: item.id,
    kind: item.kind === "kink" ? "kink" : "trope",
    title: item.title || "",
    content: item.content || "",
    keywords: (item.keywords || []).join(", "),
    intensity: String(attrs.intensity || "3"),
    doText: attrs.do || "",
    dontText: attrs.dont || "",
    tags: formatTropeTags(attrs.tags || ""),
  };
  editorOpen.value = true;
  await nextTick();
  applyingRemote = false;
}

async function resetForm() {
  applyingRemote = true;
  const kind = kindFilter.value === "kink" ? "kink" : "trope";
  form.value = emptyForm(kind);
  status.value = "";
  editorOpen.value = true;
  await nextTick();
  applyingRemote = false;
}

function buildPayload() {
  const kind = form.value.kind === "kink" ? "kink" : "trope";
  const attrs = {
    intensity: String(form.value.intensity || "3"),
  };
  if (form.value.doText.trim()) attrs.do = form.value.doText.trim();
  if (form.value.dontText.trim()) attrs.dont = form.value.dontText.trim();
  const tags = formatTropeTags(form.value.tags);
  attrs.tags = tags;
  return {
    id: form.value.id || "",
    kind,
    title: form.value.title.trim(),
    content: form.value.content,
    keywords: form.value.keywords
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean),
    links: [],
    attrs,
    unique: true,
    sources: [],
    updated_at: "",
  };
}

async function save(opts = {}) {
  const silent = !!opts.silent;
  error.value = "";
  if (!silent) status.value = "";
  try {
    if (!rosterPath.value) await refresh();
    if (!rosterPath.value) throw new Error(t("trope.noPath"));
    if (!form.value.title.trim()) {
      if (silent) return;
      throw new Error(t("trope.needTitle"));
    }
    const kind = form.value.kind === "kink" ? "kink" : "trope";
    const r = await project.upsertLoreAt(rosterPath.value, buildPayload());
    const saved = r && r.item;
    if (saved && saved.id) form.value.id = saved.id;
    status.value = silent
      ? t("trope.savedAuto")
      : kind === "kink"
        ? t("trope.savedKink")
        : t("trope.savedTrope");
    await refresh();
    bumpTropeRevision();
  } catch (e) {
    if (!silent) error.value = String(e.message || e);
  }
}

function scheduleAutoSave() {
  if (applyingRemote || scan.running) return;
  if (!form.value.title.trim()) return;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    void save({ silent: true });
  }, 700);
}

async function runScan(root) {
  error.value = "";
  try {
    await scanTropesFromRoot(root);
    await refresh();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onScanCurrent() {
  if (!canScanCurrent.value) {
    error.value = t("trope.scanNeedProject");
    return;
  }
  const n = chapterCount.value;
  let msg = t("trope.scanConfirm", { n });
  try {
    const r = await project.listTropeSummaryStatus([appState.projectRoot]);
    const row = ((r && r.items) || [])[0] || {};
    const chars = Number(row.prose_chars) || 0;
    const priced = tropesScanConfirmText({
      settings: appState.settings,
      books: [{ chars }],
      n,
      variant: "scan",
    });
    if (priced) msg = priced;
  } catch {
    /* 约算失败仍用模糊句 */
  }
  const ok = await appConfirm(msg, {
    title: t("trope.scanCurrent"),
  });
  if (!ok) return;
  await runScan(appState.projectRoot);
}

async function onScanImport() {
  error.value = "";
  try {
    const filePicked = await project.pickFile(t("knowledge.pickTxt"), ["txt", "md"]);
    const dirPicked = await project.pickDirectory();
    const filePath = String((filePicked && filePicked.path) || "");
    const base = filePath.replace(/^.*[\\/]/, "").replace(/\.(txt|md)$/i, "");
    const name = importTitle.value.trim() || base.trim() || t("knowledge.untitledKb");
    appState.statusMessage = t("knowledge.importingStatus");
    const opened = await kb.importIntoKb(dirPicked.path, filePicked.path, name);
    appState.activeNav = "tropes";
    const n = ((opened.project && opened.project.chapters) || []).length;
    const ok = await appConfirm(t("trope.scanConfirmImportUnknown", { n }), {
      title: t("trope.scanImport"),
    });
    if (!ok) {
      await refresh();
      return;
    }
    await runScan(opened.root || dirPicked.path);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

const scanPct = computed(() => Math.max(0, Math.min(100, Number(scan.pct) || 0)));
const scanIndeterminate = computed(
  () => scan.running && (scan.steps || 0) <= 0 && (scan.total || 0) <= 0
);
const scanUsageText = computed(() => formatScanUsage(scan));
const scanMeterLabel = computed(() => {
  if (scanIndeterminate.value) return t("progress.connecting");
  if (scan.chunk && scan.chunks) {
    return t("trope.scanMeterChunk", {
      pct: scanPct.value,
      chunk: scan.chunk,
      chunks: scan.chunks,
    });
  }
  return t("trope.scanMeter", { pct: scanPct.value, current: scan.current, total: scan.total });
});

watch(
  () => [
    scan.current,
    scan.total,
    scan.added,
    scan.updated,
    scan.running,
    scan.tokens,
    scan.calls,
    scan.costCny,
    scan.chunk,
    scan.chunks,
  ],
  () => {
    if (!scan.running) return;
    const base = t("trope.scanProgress", {
      current: scan.current,
      total: scan.total,
      added: scan.added,
      updated: scan.updated,
    });
    const usage = formatScanUsage(scan);
    appState.statusMessage = usage ? `${base} · ${usage}` : base;
  }
);

watch(
  form,
  () => {
    scheduleAutoSave();
  },
  { deep: true }
);

watch(
  () => [scan.added, scan.updated, scan.running],
  () => {
    if (!scan.running) {
      if (scanRefreshTimer) {
        clearTimeout(scanRefreshTimer);
        scanRefreshTimer = null;
      }
      return;
    }
    if (scanRefreshTimer) clearTimeout(scanRefreshTimer);
    scanRefreshTimer = setTimeout(() => {
      scanRefreshTimer = null;
      void refresh();
    }, 800);
  }
);

async function remove(item) {
  if (!rosterPath.value) return;
  if (
    !(await appConfirmDelete(t("trope.deleteQ", { title: item.title || item.id }), {
      title: t("trope.deleteTitle"),
    }))
  ) {
    return;
  }
  await project.deleteLoreAt(rosterPath.value, item.id);
  if (form.value.id === item.id) {
    applyingRemote = true;
    form.value = emptyForm(kindFilter.value === "kink" ? "kink" : "trope");
    editorOpen.value = false;
    await nextTick();
    applyingRemote = false;
  }
  if (expandedId.value === item.id) expandedId.value = "";
  await refresh();
  bumpTropeRevision();
}

onMounted(refresh);
onActivated(refresh);
onUnmounted(() => {
  if (saveTimer) clearTimeout(saveTimer);
  if (scanRefreshTimer) clearTimeout(scanRefreshTimer);
});
</script>

<template>
  <section class="panel roster-panel">
    <div class="roster-head">
      <h1 class="panel-heading">{{ $t("trope.title") }}</h1>
      <p class="muted">
        {{ $t("trope.introBefore") }}<strong>{{ $t("trope.introStrong") }}</strong>{{ $t("trope.introAfter") }}
        <code v-if="rosterPath">{{ rosterPath }}</code>
        <span v-else>{{ $t("common.loading") }}</span>
        {{ $t("trope.autoSave") }}
      </p>

      <div class="tabs">
        <button
          type="button"
          class="chip"
          :class="kindFilter === 'all' ? 'chip-active' : ''"
          @click="kindFilter = 'all'"
        >
          {{ $t("common.all") }}
        </button>
        <button
          type="button"
          class="chip"
          :class="kindFilter === 'trope' ? 'chip-active' : ''"
          @click="kindFilter = 'trope'"
        >
          {{ $t("common.trope") }}
        </button>
        <button
          type="button"
          class="chip"
          :class="kindFilter === 'kink' ? 'chip-active' : ''"
          @click="kindFilter = 'kink'"
        >
          {{ $t("common.kink") }}
        </button>
        <button type="button" class="app-btn app-btn-light refresh-btn" :disabled="scan.running" @click="refresh">{{ $t("common.refresh") }}</button>
        <button type="button" class="app-btn app-btn-primary" :disabled="scan.running" @click="resetForm">{{ $t("trope.new") }}</button>
        <button type="button" class="app-btn" :disabled="!canScanCurrent" @click="onScanCurrent">{{ $t("trope.scanCurrent") }}</button>
        <button type="button" class="app-btn" :disabled="scan.running" @click="onScanImport">{{ $t("trope.scanImport") }}</button>
        <button v-if="scan.running" type="button" class="app-btn" @click="cancelTropeScan">{{ $t("common.cancel") }}</button>
      </div>

      <div class="filter-row">
        <input
          v-model="searchQuery"
          type="search"
          class="search-input"
          :placeholder="$t('trope.searchPh')"
        />
        <span class="muted count-label">{{ visibleCountText }}</span>
        <div class="field-panel-wrap">
          <button type="button" class="app-btn app-btn-light" @click="showFieldPanel = !showFieldPanel">
            {{ $t("trope.cardDisplay") }}
          </button>
          <div v-if="showFieldPanel" class="field-panel">
            <label v-for="id in TROPE_CARD_FIELD_IDS" :key="id" class="field-check">
              <input type="checkbox" :checked="fieldOn(id)" @change="toggleField(id)" />
              {{ $t("trope.field" + id.charAt(0).toUpperCase() + id.slice(1)) }}
            </label>
            <div class="density-row">
              <span class="muted">{{ $t("trope.density") }}</span>
              <button
                type="button"
                class="chip"
                :class="cardDensity === 'standard' ? 'chip-active' : ''"
                @click="setDensity('standard')"
              >
                {{ $t("trope.densityStandard") }}
              </button>
              <button
                type="button"
                class="chip"
                :class="cardDensity === 'compact' ? 'chip-active' : ''"
                @click="setDensity('compact')"
              >
                {{ $t("trope.densityCompact") }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <div class="cat-row">
        <button
          v-if="categoryCounts.uncat"
          type="button"
          class="chip"
          :class="categoryFilter === '__uncat__' ? 'chip-active' : ''"
          @click="toggleCategory('__uncat__')"
        >
          {{ $t("trope.uncategorized") }} {{ categoryCounts.uncat }}
        </button>
        <button
          v-for="chip in categoryCounts.chips"
          :key="chip.id"
          type="button"
          class="chip"
          :class="categoryFilter === chip.id ? 'chip-active' : ''"
          @click="toggleCategory(chip.id)"
        >
          {{ $t(categoryLabelKey(chip.id)) }} {{ chip.n }}
        </button>
      </div>

      <div v-if="scan.running" class="scan-meter-wrap">
        <div
          class="scan-meter"
          :class="{ indeterminate: scanIndeterminate }"
          role="progressbar"
          :aria-valuenow="scanIndeterminate ? undefined : scanPct"
          aria-valuemin="0"
          aria-valuemax="100"
          aria-busy="true"
        >
          <div class="scan-track">
            <div
              class="scan-fill"
              :style="scanIndeterminate ? undefined : { width: scanPct + '%' }"
            />
          </div>
          <span class="scan-meter-label">{{ scanMeterLabel }}</span>
        </div>
        <p class="muted scan-progress">
          {{ $t("trope.scanProgress", { current: scan.current, total: scan.total, added: scan.added, updated: scan.updated }) }}
          <span v-if="scan.title"> · {{ scan.title }}</span>
          <span v-if="scan.chunk && scan.chunks">
            · {{ $t("trope.scanChunk", { chunk: scan.chunk, chunks: scan.chunks }) }}
          </span>
        </p>
        <p v-if="scanUsageText" class="muted scan-usage">{{ scanUsageText }}</p>
      </div>
      <div class="field scan-title-field">
        <label class="field-label">{{ $t("trope.importTitle") }}</label>
        <input v-model="importTitle" type="text" :disabled="scan.running" :placeholder="$t('knowledge.untitledKb')" />
      </div>
    </div>

    <div class="lore-grid" :class="{ 'has-drawer': editorOpen }">
      <div class="list-pane" @click="onListPaneClick">
        <div class="card-grid">
          <article
            v-for="item in visibleItems"
            :key="item.id"
            class="lore-item"
            :class="{
              active: form.id === item.id && editorOpen,
              compact: cardDensity === 'compact',
              expanded: expandedId === item.id,
            }"
            @click.stop="edit(item)"
          >
            <button
              type="button"
              class="card-del"
              :disabled="scan.running"
              :title="$t('common.delete')"
              @click.stop="remove(item)"
            >
              {{ $t("common.delete") }}
            </button>
            <div class="lore-meta">
              <div class="card-head">
                <strong>{{ item.title }}</strong>
                <button
                  type="button"
                  class="expand-btn"
                  @click.stop="toggleExpand(item.id)"
                >
                  {{ expandedId === item.id ? $t("trope.collapse") : $t("trope.expand") }}
                </button>
              </div>
              <div class="tag-row">
                <span class="chip chip-active kind-tag">{{ $t(tropeKindLabelKey(item.kind)) }}</span>
                <span
                  v-if="fieldOn('intensity') && attrsOf(item).intensity"
                  class="chip kind-tag"
                >{{ $t("trope.intensityShort", { n: attrsOf(item).intensity }) }}</span>
                <span
                  v-if="fieldOn('tags')"
                  v-for="tag in itemCategoryTags(item)"
                  :key="tag"
                  class="chip kind-tag cat-chip"
                >{{ $t(categoryLabelKey(tag)) }}</span>
              </div>
              <div v-if="fieldOn('keywords') && keywordChips(item).length" class="tag-row">
                <span v-for="kw in keywordChips(item)" :key="kw" class="chip kind-tag kw-chip">{{ kw }}</span>
              </div>
              <p
                v-if="fieldOn('snippet') && item.content && expandedId !== item.id"
                class="snippet"
              >{{ item.content }}</p>
              <p v-if="expandedId === item.id && item.content" class="full-content">{{ item.content }}</p>
              <div v-if="showDoDont(item)" class="do-dont">
                <p v-if="attrsOf(item).do"><span class="muted">{{ $t("trope.do") }}</span> {{ attrsOf(item).do }}</p>
                <p v-if="attrsOf(item).dont"><span class="muted">{{ $t("trope.dont") }}</span> {{ attrsOf(item).dont }}</p>
              </div>
              <p v-if="showEvidence(item)" class="evidence">
                <span class="muted">{{ $t("trope.evidence") }}</span> {{ attrsOf(item).evidence }}
              </p>
            </div>
          </article>
        </div>
        <p v-if="!visibleItems.length" class="muted empty-hint">{{ $t("trope.empty") }}</p>
      </div>

      <aside v-if="editorOpen" class="editor editor-pane" @click.stop>
        <div class="drawer-head">
          <strong>{{ form.id ? $t("trope.save") : $t("trope.new") }}</strong>
          <button type="button" class="app-btn app-btn-light" @click="closeEditor">{{ $t("trope.closeDrawer") }}</button>
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.type") }}</label>
          <select v-model="form.kind">
            <option value="trope">{{ $t("common.trope") }}</option>
            <option value="kink">{{ $t("common.kink") }}</option>
          </select>
        </div>
        <div class="field">
          <label class="field-label">{{ $t("trope.name") }}</label>
          <input
            v-model="form.title"
            type="text"
            :placeholder="form.kind === 'kink' ? $t('trope.titlePhKink') : $t('trope.titlePhTrope')"
          />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.keywords") }}</label>
          <input v-model="form.keywords" type="text" :placeholder="$t('trope.keywordsPh')" />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("trope.intensity") }}</label>
          <select v-model="form.intensity">
            <option v-for="n in 5" :key="n" :value="String(n)">{{ n }}</option>
          </select>
        </div>
        <div class="field">
          <label class="field-label">{{ $t("trope.tags") }}</label>
          <div class="tag-row editor-tags">
            <button
              v-for="id in TROPE_CATEGORY_IDS"
              :key="id"
              type="button"
              class="chip"
              :class="formTagSelected(id) ? 'chip-active' : ''"
              @click="toggleFormTag(id)"
            >
              {{ $t(categoryLabelKey(id)) }}
            </button>
          </div>
        </div>
        <div class="field">
          <label class="field-label">{{ $t("trope.do") }}</label>
          <textarea v-model="form.doText" rows="2" :placeholder="$t('trope.doPh')" />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("trope.dont") }}</label>
          <textarea v-model="form.dontText" rows="2" :placeholder="$t('trope.dontPh')" />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("trope.content") }}</label>
          <textarea v-model="form.content" rows="10" :placeholder="$t('trope.contentPh')" />
        </div>
        <div class="actions">
          <button type="button" class="app-btn app-btn-primary" :disabled="scan.running" @click="save()">{{ $t("trope.save") }}</button>
          <button type="button" class="app-btn" :disabled="scan.running" @click="resetForm">{{ $t("trope.new") }}</button>
        </div>
        <pre v-if="status" class="out">{{ status }}</pre>
      </aside>
    </div>
  </section>
</template>

<style scoped>
.roster-panel {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.roster-head {
  flex-shrink: 0;
}
.tabs,
.filter-row,
.cat-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}
.refresh-btn {
  margin-left: auto;
}
.search-input {
  flex: 1 1 220px;
  min-width: 160px;
  max-width: 420px;
}
.count-label {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.field-panel-wrap {
  position: relative;
}
.field-panel {
  position: absolute;
  right: 0;
  top: calc(100% + 6px);
  z-index: 8;
  min-width: 200px;
  padding: 10px 12px;
  border-radius: var(--radius-lg, 10px);
  background: var(--surface-solid, #1c1c1c);
  box-shadow: var(--shadow, 0 8px 24px rgba(0, 0, 0, 0.25));
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.field-check {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.85rem;
}
.density-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
}
.scan-meter-wrap {
  margin-top: 10px;
  max-width: 720px;
}
.scan-meter {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 8px;
  height: 24px;
}
.scan-track {
  flex: 1 1 auto;
  height: 8px;
  border-radius: 999px;
  background: var(--chip-bg, rgba(0, 0, 0, 0.08));
  overflow: hidden;
  min-width: 80px;
}
.scan-fill {
  height: 100%;
  width: 0;
  border-radius: inherit;
  background: linear-gradient(90deg, var(--accent, #f472b6), var(--accent-hover, #ec4899));
  transition: width 0.2s ease-out;
}
.scan-meter.indeterminate .scan-fill {
  width: 36%;
  animation: scan-indet 1.1s ease-in-out infinite;
}
.scan-meter-label {
  flex-shrink: 0;
  font-size: 12px;
  font-weight: 600;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.scan-progress,
.scan-usage {
  margin: 6px 0 0;
  font-variant-numeric: tabular-nums;
}
@keyframes scan-indet {
  0% {
    transform: translateX(-120%);
  }
  100% {
    transform: translateX(320%);
  }
}
.scan-title-field {
  margin-top: 8px;
  max-width: 360px;
}
.lore-grid {
  display: flex;
  gap: 14px;
  margin-top: 12px;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.list-pane {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  padding-right: 4px;
}
.editor-pane {
  flex: 0 0 400px;
  width: 400px;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 4px 8px 16px 12px;
  border-left: 1px solid var(--border, rgba(0, 0, 0, 0.08));
}
.drawer-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}
.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 12px;
  align-items: start;
}
.lore-item {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 14px 16px;
  border: none;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: background 0.15s ease, box-shadow 0.15s ease;
}
.lore-item.compact {
  padding: 10px 12px;
}
.lore-item:hover,
.lore-item.active {
  background: var(--accent-soft);
  box-shadow: var(--shadow);
}
.card-del {
  position: absolute;
  top: 8px;
  right: 8px;
  opacity: 0;
  font-size: 12px;
  padding: 2px 8px;
  border: none;
  border-radius: 6px;
  background: var(--danger, #c44);
  color: #fff;
  cursor: pointer;
}
.lore-item:hover .card-del,
.lore-item.active .card-del {
  opacity: 1;
}
.lore-meta {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  width: 100%;
  padding-right: 8px;
}
.card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
  padding-right: 36px;
}
.expand-btn {
  flex-shrink: 0;
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 12px;
  cursor: pointer;
  padding: 0;
}
.tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.kind-tag {
  align-self: flex-start;
  pointer-events: none;
  box-shadow: none;
}
.kw-chip,
.cat-chip {
  font-size: 0.78rem;
}
.snippet {
  margin: 0;
  font-size: 0.85rem;
  color: var(--muted, #888);
  line-height: 1.4;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 4;
  overflow: hidden;
}
.lore-item.compact .snippet {
  -webkit-line-clamp: 2;
}
.full-content,
.do-dont p,
.evidence {
  margin: 0;
  font-size: 0.85rem;
  line-height: 1.45;
  white-space: pre-wrap;
}
.do-dont {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.empty-hint {
  margin: 12px 4px;
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}
.editor-tags .chip {
  pointer-events: auto;
}
@media (max-width: 900px) {
  .lore-grid.has-drawer {
    position: relative;
  }
  .editor-pane {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(400px, 100%);
    flex: none;
    background: var(--bg, var(--surface-solid));
    z-index: 6;
    box-shadow: var(--shadow);
  }
  .refresh-btn {
    margin-left: 0;
  }
}
</style>
