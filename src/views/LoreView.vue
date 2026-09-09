<!--
  设定库：无作品也可编辑全局角色/世界观；有小说时另加本篇补充
  代码路径: kk_novel_ai/src/views/LoreView.vue
-->
<script setup>
import { computed, onActivated, onMounted, ref, watch } from "vue";
import { appState, bumpCastRevision, isKbProject } from "../stores/appState.js";
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

defineProps({
  embedded: { type: Boolean, default: false },
});

const items = ref([]);
const error = useToastError();
const status = ref("");
/** local | global —— 仅小说工程需要本篇 */
const tab = ref("global");
/** all | character | world */
const kindFilter = ref("all");
const rosterPath = ref("");
const form = ref({
  id: "",
  kind: "character",
  title: "",
  content: "",
  keywords: "",
  linksText: "",
  attrsText: "",
  unique: true,
  scope: "global",
  visual: emptyVisualSheet(),
});

const isNovel = computed(() => {
  const p = appState.project;
  return !!(p && appState.projectRoot && !isKbProject(p));
});

/** 无写作工程时仍可维护全局设定 */
const globalOnly = computed(() => !isNovel.value);

const visibleItems = computed(() => {
  let list = items.value.filter((it) => !isTropeKind(it.kind));
  if (isNovel.value) {
    list = list.filter((it) => it.scope === tab.value);
  }
  if (kindFilter.value !== "all") {
    list = list.filter((it) => it.kind === kindFilter.value);
  }
  return list;
});

async function loadGlobalRoster() {
  const ens = await project.ensureCharacterRoster();
  rosterPath.value = ens.root || "";
  if (!rosterPath.value) throw new Error(t("lore.noRoster"));
  const r = await project.listLoreAt(rosterPath.value);
  items.value = (r.items || []).map((e) => ({
    ...e,
    scope: "global",
    _root: rosterPath.value,
  }));
}

async function refresh() {
  error.value = "";
  status.value = "";
  try {
    if (isNovel.value) {
      await project.ensureCharactersLink();
      const r = await project.listLoreScoped();
      rosterPath.value = (r.character_roster && r.character_roster.path) || "";
      const local = (r.local || []).map((row) => ({
        ...row.entry,
        scope: "local",
        _root: row.root,
      }));
      const global = (r.global || []).map((row) => ({
        ...row.entry,
        scope: "global",
        _root: row.root,
      }));
      items.value = [...local, ...global];
      if (appState.project) {
        appState.project = {
          ...appState.project,
          linked_kb_roots: appState.project.linked_kb_roots || [],
        };
      }
    } else {
      tab.value = "global";
      await loadGlobalRoster();
    }
  } catch (e) {
    error.value = String(e.message || e);
    items.value = [];
  }
}

watch(() => appState.projectRoot, refresh);
watch(() => appState.project && appState.project.kind, refresh);
onMounted(refresh);
onActivated(refresh);

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
      return { target_id: (target_id || "").trim(), relation: rest.join("|").trim() || "related" };
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

function entryIsUnique(item) {
  if (item.unique) return true;
  const u = item.attrs && item.attrs.unique;
  return u === "true" || u === "1" || u === "yes";
}

function edit(item) {
  form.value = {
    id: item.id,
    kind: item.kind,
    title: item.title,
    content: item.content,
    keywords: (item.keywords || []).join(", "),
    linksText: linksToText(item.links),
    attrsText: attrsToText(item.attrs),
    unique: item.kind === "character" ? entryIsUnique(item) : false,
    scope: item.scope || "global",
    visual: visualFromAttrs(item.attrs),
  };
  if (isNovel.value) tab.value = item.scope || tab.value;
}

function resetForm() {
  const preferWorld = kindFilter.value === "world";
  form.value = {
    id: "",
    kind: preferWorld ? "world" : "character",
    title: "",
    content: "",
    keywords: "",
    linksText: "",
    attrsText: "",
    unique: !preferWorld,
    scope: globalOnly.value ? "global" : tab.value === "local" ? "local" : "global",
    visual: emptyVisualSheet(),
  };
}

function saveRoot() {
  if (globalOnly.value || form.value.scope === "global") {
    return rosterPath.value;
  }
  return appState.projectRoot;
}

async function save() {
  error.value = "";
  status.value = "";
  try {
    if (!rosterPath.value && (globalOnly.value || form.value.scope === "global")) {
      await loadGlobalRoster();
    }
    const root = saveRoot();
    if (!root) throw new Error(t("lore.noSavePath"));
    const isChar = form.value.kind === "character";
    await project.upsertLoreAt(root, {
      id: form.value.id || "",
      kind: form.value.kind,
      title: form.value.title,
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
    if (globalOnly.value || form.value.scope === "global") {
      status.value =
        form.value.kind === "world" ? t("lore.savedWorld") : t("lore.savedChar");
    } else {
      status.value = t("lore.savedLocal");
    }
    resetForm();
    await refresh();
    bumpCastRevision();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function remove(item) {
  const root = item._root || rosterPath.value || appState.projectRoot;
  if (!root) return;
  if (
    !(await appConfirmDelete(t("lore.deleteQ", { title: item.title || item.id }), {
      title: t("lore.deleteTitle"),
    }))
  ) {
    return;
  }
  await project.deleteLoreAt(root, item.id);
  await refresh();
  bumpCastRevision();
}
</script>

<template>
  <section class="panel" :class="{ 'lore-embed': embedded }">
    <h1 v-if="!embedded" class="panel-heading">{{ $t("lore.title") }}</h1>
    <p v-if="!embedded" class="muted">
      <template v-if="globalOnly">
        {{ $t("lore.introGlobal") }}
      </template>
      <template v-else>
        {{ $t("lore.introNovel") }}
      </template>
      <code v-if="rosterPath">{{ rosterPath }}</code>
      <span v-else>{{ $t("common.loading") }}</span>
    </p>

    <div class="tabs">
      <template v-if="isNovel">
        <button
          type="button"
          class="chip"
          :class="tab === 'global' ? 'chip-active' : ''"
          @click="tab = 'global'"
        >
          {{ $t("common.global") }}
        </button>
        <button
          type="button"
          class="chip"
          :class="tab === 'local' ? 'chip-active' : ''"
          @click="tab = 'local'"
        >
          {{ $t("lore.localExtra") }}
        </button>
      </template>
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
    </div>

    <div class="lore-grid">
      <div>
        <div
          v-for="item in visibleItems"
          :key="item.scope + ':' + item.id"
          class="lore-item"
          @click="edit(item)"
        >
          <div class="lore-meta">
            <strong>{{ item.title }}</strong>
            <div class="tag-row">
              <span class="chip chip-active kind-tag">{{
                item.kind === "world" ? $t("common.worldShort") : $t("common.character")
              }}</span>
              <span v-if="item.kind === 'character' && entryIsUnique(item)" class="chip kind-tag unique"
                >{{ $t("common.unique") }}</span
              >
              <span v-if="isNovel" class="chip kind-tag">{{
                item.scope === "global" ? $t("common.global") : $t("common.local")
              }}</span>
            </div>
          </div>
          <button type="button" class="app-btn app-btn-danger" @click.stop="remove(item)">{{ $t("common.delete") }}</button>
        </div>
        <p v-if="!visibleItems.length" class="muted">
          {{ $t("lore.empty") }}
        </p>
      </div>
      <div>
        <div class="field" v-if="isNovel">
          <label class="field-label">{{ $t("lore.saveLocation") }}</label>
          <select v-model="form.scope">
            <option value="global">{{ $t("lore.globalStore") }}</option>
            <option value="local">{{ $t("lore.localOnly") }}</option>
          </select>
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.type") }}</label>
          <select v-model="form.kind" @change="form.unique = form.kind === 'character'">
            <option value="character">{{ $t("common.character") }}</option>
            <option value="world">{{ $t("common.world") }}</option>
          </select>
        </div>
        <div class="field capsule-switch-row" v-if="form.kind === 'character'">
          <CapsuleSwitch v-model="form.unique" :label="$t('lore.uniqueSwitch')" />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.entryTitle") }}</label>
          <input v-model="form.title" type="text" :placeholder="form.kind === 'world' ? $t('lore.titlePhWorld') : $t('lore.titlePhChar')" />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.keywords") }}</label>
          <input v-model="form.keywords" type="text" />
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
            :placeholder="form.kind === 'world' ? $t('lore.attrsPhWorld') : $t('lore.attrsPhChar')"
          />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.links") }}</label>
          <textarea v-model="form.linksText" rows="3" :placeholder="$t('lore.linksPh')" />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("lore.content") }}</label>
          <textarea v-model="form.content" rows="8" />
        </div>
        <div class="actions">
          <button type="button" class="app-btn app-btn-primary" @click="save">{{ $t("lore.save") }}</button>
          <button type="button" class="app-btn" @click="resetForm">{{ $t("lore.new") }}</button>
        </div>
      </div>
    </div>
    <pre v-if="status" class="out">{{ status }}</pre>
  </section>
</template>

<style scoped>
.panel {
  min-height: calc(100% - 8px);
}
.lore-embed {
  min-height: 0;
  height: 100%;
  padding: 0;
  box-shadow: none;
  background: transparent;
  overflow: auto;
}
.lore-embed .tabs {
  margin-top: 0;
}
.tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
}
.lore-grid {
  display: grid;
  grid-template-columns: 1fr 1.2fr;
  gap: 14px;
  margin-top: 12px;
}
.lore-item {
  display: flex;
  align-items: center;
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
.lore-item:hover {
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
.unique {
  opacity: 0.95;
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}
.check label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.95rem;
}
.error {
  color: var(--error);
}
@media (max-width: 900px) {
  .lore-grid {
    grid-template-columns: 1fr;
  }
}
</style>
