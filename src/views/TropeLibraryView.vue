<!--
  情节库：全局情节套路 / 性癖（不依赖作品）
  代码路径: kk_novel_ai/src/views/TropeLibraryView.vue
-->
<script setup>
import { computed, onActivated, onMounted, ref } from "vue";
import { bumpTropeRevision } from "../stores/appState.js";
import * as project from "../services/projectClient.js";
import { appConfirmDelete } from "../services/confirmDialog.js";
import { useToastError } from "../services/toast.js";
import { t } from "../i18n/index.js";
import { isTropeKind, tropeKindLabelKey } from "../utils/tropeKinds.js";

const items = ref([]);
const rosterPath = ref("");
const error = useToastError();
const status = ref("");
/** all | trope | kink */
const kindFilter = ref("all");
const form = ref(emptyForm("trope"));

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

const visibleItems = computed(() => {
  if (kindFilter.value === "all") return libraryItems.value;
  return libraryItems.value.filter((it) => it.kind === kindFilter.value);
});

function attrsOf(item) {
  return (item && item.attrs && typeof item.attrs === "object") ? item.attrs : {};
}

async function refresh() {
  error.value = "";
  try {
    const ens = await project.ensureCharacterRoster();
    rosterPath.value = ens.root || "";
    if (!rosterPath.value) throw new Error(t("trope.noRoster"));
    const r = await project.listLoreAt(rosterPath.value);
    items.value = r.items || [];
  } catch (e) {
    error.value = String(e.message || e);
    items.value = [];
  }
}

function edit(item) {
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
    tags: attrs.tags || "",
  };
  kindFilter.value = form.value.kind;
}

function resetForm() {
  const kind = kindFilter.value === "kink" ? "kink" : "trope";
  form.value = emptyForm(kind);
}

async function save() {
  error.value = "";
  status.value = "";
  try {
    if (!rosterPath.value) await refresh();
    if (!rosterPath.value) throw new Error(t("trope.noPath"));
    if (!form.value.title.trim()) throw new Error(t("trope.needTitle"));
    const kind = form.value.kind === "kink" ? "kink" : "trope";
    const attrs = {
      intensity: String(form.value.intensity || "3"),
    };
    if (form.value.doText.trim()) attrs.do = form.value.doText.trim();
    if (form.value.dontText.trim()) attrs.dont = form.value.dontText.trim();
    if (form.value.tags.trim()) attrs.tags = form.value.tags.trim();
    await project.upsertLoreAt(rosterPath.value, {
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
    });
    status.value = kind === "kink" ? t("trope.savedKink") : t("trope.savedTrope");
    resetForm();
    await refresh();
    bumpTropeRevision();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

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
  if (form.value.id === item.id) resetForm();
  await refresh();
  bumpTropeRevision();
}

onMounted(refresh);
onActivated(refresh);
</script>

<template>
  <section class="panel roster-panel">
    <div class="roster-head">
      <h1 class="panel-heading">{{ $t("trope.title") }}</h1>
      <p class="muted">
        {{ $t("trope.introBefore") }}<strong>{{ $t("trope.introStrong") }}</strong>{{ $t("trope.introAfter") }}
        <code v-if="rosterPath">{{ rosterPath }}</code>
        <span v-else>{{ $t("common.loading") }}</span>
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
        <button type="button" class="app-btn app-btn-light refresh-btn" @click="refresh">{{ $t("common.refresh") }}</button>
      </div>
    </div>

    <div class="lore-grid">
      <div class="list-pane">
        <div
          v-for="item in visibleItems"
          :key="item.id"
          class="lore-item"
          :class="{ active: form.id === item.id }"
          @click="edit(item)"
        >
          <div class="lore-meta">
            <strong>{{ item.title }}</strong>
            <div class="tag-row">
              <span class="chip chip-active kind-tag">{{ $t(tropeKindLabelKey(item.kind)) }}</span>
              <span v-if="attrsOf(item).intensity" class="chip kind-tag">{{ $t("trope.intensityShort", { n: attrsOf(item).intensity }) }}</span>
            </div>
            <p class="snippet">{{ (item.content || "").slice(0, 72) }}{{ (item.content || "").length > 72 ? "…" : "" }}</p>
          </div>
          <button type="button" class="app-btn app-btn-danger" @click.stop="remove(item)">{{ $t("common.delete") }}</button>
        </div>
        <p v-if="!visibleItems.length" class="muted">{{ $t("trope.empty") }}</p>
      </div>

      <div class="editor editor-pane">
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
          <input v-model="form.tags" type="text" :placeholder="$t('trope.tagsPh')" />
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
          <button type="button" class="app-btn app-btn-primary" @click="save">{{ $t("trope.save") }}</button>
          <button type="button" class="app-btn" @click="resetForm">{{ $t("trope.new") }}</button>
        </div>
        <pre v-if="status" class="out">{{ status }}</pre>
      </div>
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
.tabs {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}
.refresh-btn {
  margin-left: auto;
}
.lore-grid {
  display: grid;
  grid-template-columns: 1fr 1.35fr;
  gap: 14px;
  margin-top: 12px;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.list-pane,
.editor-pane {
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  padding-right: 4px;
}
.lore-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 14px 16px;
  border: none;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  margin-bottom: 10px;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: background 0.15s ease, box-shadow 0.15s ease;
}
.lore-item:hover,
.lore-item.active {
  background: var(--accent-soft);
  box-shadow: var(--shadow);
}
.lore-meta {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
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
.snippet {
  margin: 0;
  font-size: 0.85rem;
  color: var(--muted, #888);
  line-height: 1.4;
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}
@media (max-width: 900px) {
  .lore-grid {
    grid-template-columns: 1fr;
    overflow-y: auto;
  }
  .list-pane,
  .editor-pane {
    max-height: none;
    overflow: visible;
  }
  .refresh-btn {
    margin-left: 0;
  }
}
</style>
