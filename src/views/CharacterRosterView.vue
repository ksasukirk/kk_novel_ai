<!--
  角色定义：全局人物 + 背景/世界观（不依赖作品）
  代码路径: kk_novel_ai/src/views/CharacterRosterView.vue
-->
<script setup>
import { computed, onActivated, onMounted, ref } from "vue";
import { bumpCastRevision } from "../stores/appState.js";
import * as project from "../services/projectClient.js";
import CapsuleSwitch from "../components/CapsuleSwitch.vue";
import { appConfirmDelete } from "../services/confirmDialog.js";
import { useToastError } from "../services/toast.js";
import { t } from "../i18n/index.js";
import {
  emptyVisualSheet,
  isVisualAttrKey,
  mergeVisualIntoAttrs,
  visualFromAttrs,
} from "../utils/loreVisual.js";
import { isTropeKind } from "../utils/tropeKinds.js";

const items = ref([]);
const rosterPath = ref("");
const error = useToastError();
const status = ref("");
/** all | character | world */
const kindFilter = ref("character");
const form = ref(emptyForm("character"));

function emptyForm(kind) {
  return {
    id: "",
    kind: kind || "character",
    title: "",
    content: "",
    keywords: "",
    linksText: "",
    attrsText: "",
    unique: kind !== "world",
    visual: emptyVisualSheet(),
  };
}

const visibleItems = computed(() => {
  if (kindFilter.value === "all") return items.value;
  return items.value.filter((it) => it.kind === kindFilter.value);
});

function entryIsUnique(item) {
  if (item.unique) return true;
  const u = item.attrs && item.attrs.unique;
  return u === "true" || u === "1" || u === "yes";
}

function linksToText(links) {
  return (links || [])
    .map((l) => `${l.target_id}|${l.relation || ""}`)
    .join("\n");
}

function attrsToText(attrs) {
  if (!attrs || typeof attrs !== "object") return "";
  return Object.entries(attrs)
    .filter(([k]) => k !== "unique" && !k.startsWith("_") && !isVisualAttrKey(k))
    .map(([k, v]) => `${k}=${v}`)
    .join("\n");
}

function parseLinks(text) {
  return (text || "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [target_id, ...rest] = line.split("|");
      return {
        target_id: (target_id || "").trim(),
        relation: rest.join("|").trim() || "related",
      };
    })
    .filter((l) => l.target_id);
}

function parseAttrs(text) {
  const out = {};
  (text || "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .forEach((line) => {
      const i = line.indexOf("=");
      if (i > 0) out[line.slice(0, i).trim()] = line.slice(i + 1).trim();
    });
  return out;
}

async function refresh() {
  error.value = "";
  try {
    const ens = await project.ensureCharacterRoster();
    rosterPath.value = ens.root || "";
    if (!rosterPath.value) throw new Error(t("roster.noRoster"));
    const r = await project.listLoreAt(rosterPath.value);
    items.value = (r.items || []).filter((it) => !isTropeKind(it.kind));
  } catch (e) {
    error.value = String(e.message || e);
    items.value = [];
  }
}

function edit(item) {
  form.value = {
    id: item.id,
    kind: item.kind || "character",
    title: item.title || "",
    content: item.content || "",
    keywords: (item.keywords || []).join(", "),
    linksText: linksToText(item.links),
    attrsText: attrsToText(item.attrs),
    unique: item.kind === "character" ? entryIsUnique(item) : false,
    visual: visualFromAttrs(item.attrs),
  };
  if (item.kind === "world" || item.kind === "character") {
    kindFilter.value = item.kind;
  }
}

function resetForm() {
  const kind = kindFilter.value === "world" ? "world" : "character";
  form.value = emptyForm(kind);
}

async function save() {
  error.value = "";
  status.value = "";
  try {
    if (!rosterPath.value) await refresh();
    if (!rosterPath.value) throw new Error(t("roster.noPath"));
    if (!form.value.title.trim()) throw new Error(t("roster.needTitle"));
    const isChar = form.value.kind === "character";
    await project.upsertLoreAt(rosterPath.value, {
      id: form.value.id || "",
      kind: form.value.kind,
      title: form.value.title.trim(),
      content: form.value.content,
      keywords: form.value.keywords
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean),
      links: parseLinks(form.value.linksText),
      attrs: mergeVisualIntoAttrs(parseAttrs(form.value.attrsText), form.value.visual),
      unique: isChar ? !!form.value.unique : false,
      sources: [],
      updated_at: "",
    });
    status.value =
      form.value.kind === "world" ? t("roster.savedWorld") : t("roster.savedChar");
    resetForm();
    await refresh();
    bumpCastRevision();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function remove(item) {
  if (!rosterPath.value) return;
  if (
    !(await appConfirmDelete(t("roster.deleteQ", { title: item.title || item.id }), {
      title: t("roster.deleteTitle"),
    }))
  ) {
    return;
  }
  await project.deleteLoreAt(rosterPath.value, item.id);
  if (form.value.id === item.id) resetForm();
  await refresh();
  bumpCastRevision();
}

onMounted(refresh);
onActivated(refresh);
</script>

<template>
  <section class="panel roster-panel">
    <div class="roster-head">
      <h1 class="panel-heading">{{ $t("roster.title") }}</h1>
      <p class="muted">
        {{ $t("roster.introBefore") }}<strong>{{ $t("roster.introStrong") }}</strong>{{ $t("roster.introAfter") }}
        <code v-if="rosterPath">{{ rosterPath }}</code>
        <span v-else>{{ $t("common.loading") }}</span>
      </p>

      <div class="tabs">
        <button
          type="button"
          class="chip"
          :class="kindFilter === 'character' ? 'chip-active' : ''"
          @click="kindFilter = 'character'"
        >
          {{ $t("common.character") }}
        </button>
        <button
          type="button"
          class="chip"
          :class="kindFilter === 'world' ? 'chip-active' : ''"
          @click="kindFilter = 'world'"
        >
          {{ $t("common.world") }}
        </button>
        <button
          type="button"
          class="chip"
          :class="kindFilter === 'all' ? 'chip-active' : ''"
          @click="kindFilter = 'all'"
        >
          {{ $t("common.all") }}
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
              <span class="chip chip-active kind-tag">{{
                item.kind === "world" ? $t("common.worldShort") : $t("common.character")
              }}</span>
              <span
                v-if="item.kind === 'character' && entryIsUnique(item)"
                class="chip kind-tag unique"
                >{{ $t("common.unique") }}</span
              >
            </div>
            <p class="snippet">{{ (item.content || "").slice(0, 72) }}{{ (item.content || "").length > 72 ? "…" : "" }}</p>
          </div>
          <button type="button" class="app-btn app-btn-danger" @click.stop="remove(item)">{{ $t("common.delete") }}</button>
        </div>
        <p v-if="!visibleItems.length" class="muted">{{ $t("roster.empty") }}</p>
      </div>

      <div class="editor editor-pane">
        <div class="field">
          <label class="field-label">{{ $t("lore.type") }}</label>
          <select
            v-model="form.kind"
            @change="form.unique = form.kind === 'character'"
          >
            <option value="character">{{ $t("common.character") }}</option>
            <option value="world">{{ $t("common.world") }}</option>
          </select>
        </div>
        <div v-if="form.kind === 'character'" class="field capsule-switch-row">
          <CapsuleSwitch v-model="form.unique" :label="$t('roster.uniqueSwitch')" />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("roster.name") }}</label>
          <input
            v-model="form.title"
            type="text"
            :placeholder="form.kind === 'world' ? $t('lore.titlePhWorld') : $t('lore.titlePhChar')"
          />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.keywords") }}</label>
          <input v-model="form.keywords" type="text" :placeholder="$t('roster.keywordsPh')" />
        </div>
        <div class="field" v-if="form.kind === 'character'">
          <label class="field-label">{{ $t("lore.visualCard") }}</label>
          <input v-model="form.visual.外貌" type="text" :placeholder="$t('lore.look')" />
          <input v-model="form.visual.发型" type="text" :placeholder="$t('lore.hair')" />
          <input v-model="form.visual.瞳色" type="text" :placeholder="$t('lore.eyes')" />
          <input v-model="form.visual.体态" type="text" :placeholder="$t('lore.body')" />
          <input v-model="form.visual.常服" type="text" :placeholder="$t('lore.outfit')" />
          <input v-model="form.visual.画风锚" type="text" :placeholder="$t('lore.styleAnchor')" />
          <input v-model="form.visual.portrait_rel" type="text" :placeholder="$t('lore.portraitPath')" />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.attrs") }}</label>
          <textarea
            v-model="form.attrsText"
            rows="3"
            :placeholder="
              form.kind === 'world'
                ? $t('lore.attrsPhWorld')
                : $t('roster.attrsPhChar')
            "
          />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.links") }}</label>
          <textarea v-model="form.linksText" rows="2" :placeholder="$t('roster.linksPh')" />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("roster.content") }}</label>
          <textarea
            v-model="form.content"
            rows="12"
            :placeholder="$t('roster.contentPh')"
          />
        </div>
        <div class="actions">
          <button type="button" class="app-btn app-btn-primary" @click="save">{{ $t("roster.save") }}</button>
          <button type="button" class="app-btn" @click="resetForm">{{ $t("roster.new") }}</button>
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
.error {
  color: var(--error);
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
