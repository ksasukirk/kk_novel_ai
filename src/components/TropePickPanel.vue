<!--
  写作页情节/喜好卡片：筛选 + 多选注入本章，可填入指令
  代码路径: kk_novel_ai/src/components/TropePickPanel.vue
-->
<script setup>
import { computed, ref } from "vue";
import { appState } from "../stores/appState.js";
import { aiPanelForm, insertInstructionText } from "../stores/aiPanelState.js";
import { isTropeKind, tropeKindLabelKey } from "../utils/tropeKinds.js";
import {
  TROPE_CATEGORY_IDS,
  categoryLabelKey,
  itemCategoryTags,
  itemIsUncategorized,
} from "../utils/tropeCategories.js";
import {
  TROPE_STYLE_IDS,
  itemHasNoStyles,
  itemStyleTags,
  styleLabelKey,
} from "../utils/tropeStyles.js";
import {
  clearTropeSelection,
  isTropeSelected,
  toggleTropeSelection,
} from "../services/tropeSelect.js";
import { t } from "../i18n/index.js";
import { displayTropeContent, displayTropeTitle } from "../utils/tropeI18n.js";

const emit = defineEmits(["hide"]);

/** all | trope | kink | style — style 为文风分区，列表按 kind=all */
const kindFilter = ref("all");
const searchQuery = ref("");
/** "" | "__uncat__" | 规范名 */
const categoryFilter = ref("");
/** "" | "__nostyle__" | 规范文风名 */
const styleFilter = ref("");
const expandedId = ref("");
const stylePartition = computed(() => kindFilter.value === "style");

const libraryItems = computed(() =>
  (appState.tropeList || []).filter((it) => it && isTropeKind(it.kind))
);

const kindItems = computed(() => {
  if (kindFilter.value === "all" || kindFilter.value === "style") {
    return libraryItems.value;
  }
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

const styleCounts = computed(() => {
  const counts = {};
  let nostyle = 0;
  for (const it of kindItems.value) {
    const styles = itemStyleTags(it);
    if (!styles.length) nostyle += 1;
    for (const s of styles) {
      counts[s] = (counts[s] || 0) + 1;
    }
  }
  return {
    chips: TROPE_STYLE_IDS.map((id) => ({
      id,
      n: counts[id] || 0,
    })),
    nostyle,
  };
});

const visibleItems = computed(() => {
  let list = kindItems.value;
  if (categoryFilter.value === "__uncat__") {
    list = list.filter((it) => itemIsUncategorized(it));
  } else if (categoryFilter.value) {
    list = list.filter((it) => itemCategoryTags(it).includes(categoryFilter.value));
  }
  if (styleFilter.value === "__nostyle__") {
    list = list.filter((it) => itemHasNoStyles(it));
  } else if (styleFilter.value) {
    list = list.filter((it) => itemStyleTags(it).includes(styleFilter.value));
  }
  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((it) => {
      const hay = [
        it.title || "",
        (it.attrs && it.attrs.title_en) || "",
        (it.keywords || []).join(" "),
        it.content || "",
        (it.attrs && it.attrs.content_en) || "",
        itemCategoryTags(it).join(" "),
        itemStyleTags(it).join(" "),
        (it.attrs && it.attrs.do) || "",
        (it.attrs && it.attrs.dont) || "",
      ]
        .join(" ")
        .toLowerCase();
      return hay.includes(q);
    });
  }
  return list;
});

const selectedCount = computed(() => (aiPanelForm.selectedTropeIds || []).length);

const visibleCountText = computed(() =>
  t("trope.visibleCount", { n: visibleItems.value.length, m: kindItems.value.length })
);

function setKindFilter(id) {
  kindFilter.value = id;
}

function enterStylePartition() {
  kindFilter.value = "style";
  categoryFilter.value = "";
}

function toggleCategory(id) {
  categoryFilter.value = categoryFilter.value === id ? "" : id;
}

function toggleStyleFilter(id) {
  styleFilter.value = styleFilter.value === id ? "" : id;
}

function toggleExpand(id, ev) {
  if (ev) ev.stopPropagation();
  expandedId.value = expandedId.value === id ? "" : id;
}

function attrsOf(item) {
  return (item && item.attrs && typeof item.attrs === "object") ? item.attrs : {};
}

function snippetOf(item) {
  const s = displayTropeContent(item).replace(/\s+/g, " ").trim();
  if (s.length <= 80) return s;
  return s.slice(0, 80) + "…";
}

function onToggle(item) {
  if (!item || !item.id) return;
  toggleTropeSelection(item.id);
}

function onInsert(item, ev) {
  if (ev) ev.stopPropagation();
  const title = displayTropeTitle(item);
  if (!title) return;
  insertInstructionText(`「${title}」`);
}

function onHide() {
  emit("hide");
}
</script>

<template>
  <aside class="trope-pick" :aria-label="$t('ai.localTropes')">
    <div class="pick-head">
      <strong class="pick-title">{{ $t("ai.localTropes") }}</strong>
      <span class="muted pick-count">{{ $t("trope.selectedCount", { n: selectedCount }) }}</span>
      <button
        type="button"
        class="pick-head-btn"
        :disabled="!selectedCount"
        @click="clearTropeSelection"
      >
        {{ $t("trope.clearSelected") }}
      </button>
      <button type="button" class="pick-head-btn" :title="$t('editor.hideTropesBar')" @click="onHide">
        {{ $t("editor.hideToc") }}
      </button>
    </div>
    <p class="muted pick-hint">{{ $t("trope.pickHint") }}</p>
    <div class="kind-row">
      <button
        type="button"
        class="chip"
        :class="kindFilter === 'all' ? 'chip-active' : ''"
        @click="setKindFilter('all')"
      >
        {{ $t("common.all") }}
      </button>
      <button
        type="button"
        class="chip"
        :class="kindFilter === 'trope' ? 'chip-active' : ''"
        @click="setKindFilter('trope')"
      >
        {{ $t("common.trope") }}
      </button>
      <button
        type="button"
        class="chip"
        :class="kindFilter === 'kink' ? 'chip-active' : ''"
        @click="setKindFilter('kink')"
      >
        {{ $t("common.kink") }}
      </button>
      <button
        type="button"
        class="chip"
        :class="stylePartition ? 'chip-active' : ''"
        @click="enterStylePartition()"
      >
        {{ $t("trope.style") }}
      </button>
    </div>
    <input
      v-model="searchQuery"
      type="search"
      class="pick-search"
      :placeholder="$t('trope.searchPh')"
    />
    <div v-show="!stylePartition" class="filter-partition">
      <span class="partition-label">{{ $t("trope.fieldTags") }}</span>
      <div class="cat-row">
        <button
          v-if="categoryCounts.uncat"
          type="button"
          class="chip cat-chip"
          :class="categoryFilter === '__uncat__' ? 'chip-active' : ''"
          @click="toggleCategory('__uncat__')"
        >
          {{ $t("trope.uncategorized") }} {{ categoryCounts.uncat }}
        </button>
        <button
          v-for="chip in categoryCounts.chips"
          :key="chip.id"
          type="button"
          class="chip cat-chip"
          :class="categoryFilter === chip.id ? 'chip-active' : ''"
          @click="toggleCategory(chip.id)"
        >
          {{ $t(categoryLabelKey(chip.id)) }} {{ chip.n }}
        </button>
      </div>
    </div>
    <div class="filter-partition" :class="{ 'is-emphasis': stylePartition }">
      <span class="partition-label">{{ $t("trope.style") }}</span>
      <div class="cat-row">
        <button
          v-if="styleCounts.nostyle"
          type="button"
          class="chip cat-chip style-chip"
          :class="styleFilter === '__nostyle__' ? 'chip-active' : ''"
          @click="toggleStyleFilter('__nostyle__')"
        >
          {{ $t("trope.noStyle") }} {{ styleCounts.nostyle }}
        </button>
        <button
          v-for="chip in styleCounts.chips"
          :key="'style-' + chip.id"
          type="button"
          class="chip cat-chip style-chip"
          :class="[
            styleFilter === chip.id ? 'chip-active' : '',
            chip.n === 0 ? 'chip-empty' : '',
          ]"
          @click="toggleStyleFilter(chip.id)"
        >
          {{ $t(styleLabelKey(chip.id)) }} {{ chip.n }}
        </button>
      </div>
    </div>
    <p class="muted pick-count-line">{{ visibleCountText }}</p>
    <div class="pick-list">
      <article
        v-for="item in visibleItems"
        :key="item.id"
        class="pick-card"
        :class="{
          'is-on': isTropeSelected(item.id),
          'is-kink': item.kind === 'kink',
          expanded: expandedId === item.id,
        }"
        @click="onToggle(item)"
      >
        <div class="card-top">
          <strong class="card-title">{{ displayTropeTitle(item) }}</strong>
          <div class="card-ops">
            <button
              type="button"
              class="card-op"
              :title="$t('trope.insertInstrHint')"
              @click="onInsert(item, $event)"
            >
              {{ $t("trope.insertInstr") }}
            </button>
            <button type="button" class="card-op" @click="toggleExpand(item.id, $event)">
              {{ expandedId === item.id ? $t("trope.collapse") : $t("trope.expand") }}
            </button>
          </div>
        </div>
        <div class="tag-row">
          <span class="chip kind-tag">{{ $t(tropeKindLabelKey(item.kind)) }}</span>
          <span v-if="attrsOf(item).intensity" class="chip kind-tag">{{
            $t("trope.intensityShort", { n: attrsOf(item).intensity })
          }}</span>
          <span v-for="tag in itemCategoryTags(item)" :key="tag" class="chip kind-tag cat-chip">{{
            $t(categoryLabelKey(tag))
          }}</span>
          <span
            v-for="st in itemStyleTags(item)"
            :key="'st-' + st"
            class="chip kind-tag style-chip"
          >{{ $t(styleLabelKey(st)) }}</span>
        </div>
        <p v-if="displayTropeContent(item) && expandedId !== item.id" class="snippet">{{ snippetOf(item) }}</p>
        <p v-if="expandedId === item.id && displayTropeContent(item)" class="full-content">{{ displayTropeContent(item) }}</p>
        <div v-if="expandedId === item.id && (attrsOf(item).do || attrsOf(item).dont)" class="do-dont">
          <p v-if="attrsOf(item).do">
            <span class="muted">{{ $t("trope.do") }}</span> {{ attrsOf(item).do }}
          </p>
          <p v-if="attrsOf(item).dont">
            <span class="muted">{{ $t("trope.dont") }}</span> {{ attrsOf(item).dont }}
          </p>
        </div>
      </article>
      <p v-if="!visibleItems.length" class="muted empty-hint">{{ $t("trope.empty") }}</p>
    </div>
  </aside>
</template>

<style scoped>
.trope-pick {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 12px 10px;
  background: var(--panel);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow);
  overflow: hidden;
}
.pick-head {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.pick-title {
  font-size: 13px;
}
.pick-count {
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.pick-head-btn {
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 4px;
  margin-left: auto;
}
.pick-head-btn + .pick-head-btn {
  margin-left: 0;
}
.pick-head-btn:hover:not(:disabled) {
  color: var(--text);
  background: var(--hover, rgba(0, 0, 0, 0.04));
}
.pick-head-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.pick-hint {
  font-size: 11px;
  line-height: 1.35;
  margin: 0;
}
.kind-row,
.cat-row,
.tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  align-items: center;
}
.filter-partition {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: 4px 6px;
}
.filter-partition .cat-row {
  flex: 1 1 120px;
}
.partition-label {
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 600;
  color: var(--muted, #888);
  line-height: 20px;
  min-width: 2.2em;
}
.filter-partition.is-emphasis {
  padding: 3px 4px;
  border-radius: 6px;
  background: color-mix(in srgb, var(--accent-soft, #fce7f3) 55%, transparent);
}
.filter-partition.is-emphasis .partition-label {
  color: var(--accent, #be185d);
}
.cat-chip,
.style-chip {
  font-size: 11px;
  padding: 1px 7px;
  min-height: 20px;
}
.style-chip {
  border-style: dashed;
}
.chip-empty {
  opacity: 0.55;
}
.pick-search {
  width: 100%;
  min-height: 28px;
  font-size: 12px;
}
.pick-count-line {
  font-size: 11px;
  margin: 0;
}
.pick-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
  align-content: start;
  padding-right: 2px;
}
.pick-card {
  border-radius: 8px;
  padding: 6px 7px;
  background: var(--surface, rgba(0, 0, 0, 0.03));
  border: 1px solid color-mix(in srgb, var(--muted) 22%, transparent);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}
.pick-card.expanded {
  grid-column: 1 / -1;
}
.pick-card:hover {
  border-color: color-mix(in srgb, var(--accent) 40%, transparent);
}
.pick-card.is-on {
  border-color: var(--accent, #f472b6);
  background: color-mix(in srgb, var(--accent-soft, #fce7f3) 70%, transparent);
}
.pick-card.is-kink:not(.is-on) {
  border-color: color-mix(in srgb, var(--accent) 28%, var(--muted));
}
.card-top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 4px;
}
.card-title {
  font-size: 12px;
  line-height: 1.25;
  font-weight: 650;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
}
.card-ops {
  display: flex;
  flex-shrink: 0;
  flex-direction: column;
  align-items: flex-end;
  gap: 1px;
}
.card-op {
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 10px;
  cursor: pointer;
  padding: 0;
  line-height: 1.2;
}
.card-op:hover {
  color: var(--accent);
}
.kind-tag {
  pointer-events: none;
  font-size: 10px;
  padding: 0 5px;
  min-height: 16px;
}
.tag-row {
  gap: 3px;
}
.snippet,
.full-content,
.do-dont {
  margin: 0;
  font-size: 11px;
  line-height: 1.35;
  color: var(--text);
  white-space: pre-wrap;
}
.snippet {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  color: var(--muted);
}
.empty-hint {
  grid-column: 1 / -1;
  margin: 8px 4px;
  font-size: 12px;
}
@media (max-width: 360px) {
  .pick-list {
    grid-template-columns: 1fr;
  }
}
</style>
