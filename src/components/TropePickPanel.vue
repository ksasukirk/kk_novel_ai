<!--
  写作页情节/性癖卡片：筛选 + 多选注入本章，可填入指令
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
  clearTropeSelection,
  isTropeSelected,
  toggleTropeSelection,
} from "../services/tropeSelect.js";
import { t } from "../i18n/index.js";

const emit = defineEmits(["hide"]);

/** all | trope | kink */
const kindFilter = ref("all");
const searchQuery = ref("");
/** "" | "__uncat__" | 规范名 */
const categoryFilter = ref("");
const expandedId = ref("");

const libraryItems = computed(() =>
  (appState.tropeList || []).filter((it) => it && isTropeKind(it.kind))
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

function toggleCategory(id) {
  categoryFilter.value = categoryFilter.value === id ? "" : id;
}

function toggleExpand(id, ev) {
  if (ev) ev.stopPropagation();
  expandedId.value = expandedId.value === id ? "" : id;
}

function attrsOf(item) {
  return (item && item.attrs && typeof item.attrs === "object") ? item.attrs : {};
}

function snippetOf(item) {
  const s = String((item && item.content) || "")
    .replace(/\s+/g, " ")
    .trim();
  if (s.length <= 80) return s;
  return s.slice(0, 80) + "…";
}

function onToggle(item) {
  if (!item || !item.id) return;
  toggleTropeSelection(item.id);
}

function onInsert(item, ev) {
  if (ev) ev.stopPropagation();
  const title = String((item && item.title) || "").trim();
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
    </div>
    <input
      v-model="searchQuery"
      type="search"
      class="pick-search"
      :placeholder="$t('trope.searchPh')"
    />
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
          <strong class="card-title">{{ item.title }}</strong>
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
        </div>
        <p v-if="item.content && expandedId !== item.id" class="snippet">{{ snippetOf(item) }}</p>
        <p v-if="expandedId === item.id && item.content" class="full-content">{{ item.content }}</p>
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
.cat-chip {
  font-size: 11px;
  padding: 1px 7px;
  min-height: 20px;
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
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-right: 2px;
}
.pick-card {
  border-radius: 10px;
  padding: 8px 10px;
  background: var(--surface, rgba(0, 0, 0, 0.03));
  border: 1px solid color-mix(in srgb, var(--muted) 22%, transparent);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 5px;
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
  gap: 6px;
}
.card-title {
  font-size: 13px;
  line-height: 1.3;
}
.card-ops {
  display: flex;
  flex-shrink: 0;
  gap: 4px;
}
.card-op {
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 11px;
  cursor: pointer;
  padding: 0;
}
.card-op:hover {
  color: var(--accent);
}
.kind-tag {
  pointer-events: none;
  font-size: 11px;
}
.snippet,
.full-content,
.do-dont {
  margin: 0;
  font-size: 12px;
  line-height: 1.45;
  color: var(--text);
  white-space: pre-wrap;
}
.snippet {
  display: -webkit-box;
  -webkit-line-clamp: 3;
  line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  color: var(--muted);
}
.empty-hint {
  margin: 8px 4px;
  font-size: 12px;
}
</style>
