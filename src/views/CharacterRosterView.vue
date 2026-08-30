<!--
  角色定义：全局人物 + 背景/世界观（不依赖作品）
  代码路径: kk_novel_ai/src/views/CharacterRosterView.vue
-->
<script setup>
import { computed, onMounted, ref } from "vue";
import * as project from "../services/projectClient.js";
import CapsuleSwitch from "../components/CapsuleSwitch.vue";
import { appConfirmDelete } from "../services/confirmDialog.js";

const items = ref([]);
const rosterPath = ref("");
const error = ref("");
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
    .filter(([k]) => k !== "unique" && !k.startsWith("_"))
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
    if (!rosterPath.value) throw new Error("无法打开全局角色仓");
    const r = await project.listLoreAt(rosterPath.value);
    items.value = r.items || [];
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
    if (!rosterPath.value) throw new Error("无角色仓路径");
    if (!form.value.title.trim()) throw new Error("请填写标题");
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
      attrs: parseAttrs(form.value.attrsText),
      unique: isChar ? !!form.value.unique : false,
      sources: [],
      updated_at: "",
    });
    status.value =
      form.value.kind === "world" ? "背景/世界观已保存到全局仓" : "角色已保存到全局仓";
    resetForm();
    await refresh();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function remove(item) {
  if (!rosterPath.value) return;
  if (
    !(await appConfirmDelete(`删除角色「${item.title || item.id}」？`, {
      title: "删除角色",
    }))
  ) {
    return;
  }
  await project.deleteLoreAt(rosterPath.value, item.id);
  if (form.value.id === item.id) resetForm();
  await refresh();
}

onMounted(refresh);
</script>

<template>
  <section class="panel roster-panel">
    <div class="roster-head">
      <h1 class="panel-heading">角色定义</h1>
      <p class="muted">
        全局人物与背景设定，<strong>不依赖任何作品</strong>。新建小说默认挂接本仓（@characters）。路径：
        <code v-if="rosterPath">{{ rosterPath }}</code>
        <span v-else>加载中…</span>
      </p>

      <div class="tabs">
        <button
          type="button"
          class="chip"
          :class="kindFilter === 'character' ? 'chip-active' : ''"
          @click="kindFilter = 'character'; resetForm()"
        >
          角色
        </button>
        <button
          type="button"
          class="chip"
          :class="kindFilter === 'world' ? 'chip-active' : ''"
          @click="kindFilter = 'world'; resetForm()"
        >
          背景 / 世界观
        </button>
        <button
          type="button"
          class="chip"
          :class="kindFilter === 'all' ? 'chip-active' : ''"
          @click="kindFilter = 'all'"
        >
          全部
        </button>
        <button type="button" class="app-btn app-btn-light refresh-btn" @click="refresh">刷新</button>
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
                item.kind === "world" ? "背景" : "角色"
              }}</span>
              <span
                v-if="item.kind === 'character' && entryIsUnique(item)"
                class="chip kind-tag unique"
                >唯一</span
              >
            </div>
            <p class="snippet">{{ (item.content || "").slice(0, 72) }}{{ (item.content || "").length > 72 ? "…" : "" }}</p>
          </div>
          <button type="button" class="app-btn app-btn-danger" @click.stop="remove(item)">删除</button>
        </div>
        <p v-if="!visibleItems.length" class="muted">暂无条目。右侧新建即可，无需先开作品。</p>
      </div>

      <div class="editor editor-pane">
        <div class="field">
          <label class="field-label">类型</label>
          <select
            v-model="form.kind"
            @change="form.unique = form.kind === 'character'"
          >
            <option value="character">角色</option>
            <option value="world">背景 / 世界观</option>
          </select>
        </div>
        <div v-if="form.kind === 'character'" class="field capsule-switch-row">
          <CapsuleSwitch v-model="form.unique" label="唯一角色（同名跨作品只保留一条；可改）" />
        </div>
        <div class="field">
          <label class="field-label">名称</label>
          <input
            v-model="form.title"
            type="text"
            :placeholder="form.kind === 'world' ? '如：暑门乡镇' : '如：娜娜'"
          />
        </div>
        <div class="field">
          <label class="field-label">关键词（逗号分隔）</label>
          <input v-model="form.keywords" type="text" placeholder="娜娜, 清理, 女体" />
        </div>
        <div class="field">
          <label class="field-label">属性（每行 key=value）</label>
          <textarea
            v-model="form.attrsText"
            rows="3"
            :placeholder="
              form.kind === 'world'
                ? '时代=当代\n地点=乡镇'
                : '解剖=女体无阴茎\n自称=娜娜/人家'
            "
          />
        </div>
        <div class="field">
          <label class="field-label">关联（每行 target_id|关系）</label>
          <textarea v-model="form.linksText" rows="2" placeholder="uuid|恋人" />
        </div>
        <div class="field">
          <label class="field-label">设定正文</label>
          <textarea
            v-model="form.content"
            rows="12"
            placeholder="身份、性格、对白习惯、解剖硬约束、禁止项…"
          />
        </div>
        <div class="actions">
          <button type="button" class="app-btn app-btn-primary" @click="save">保存到全局仓</button>
          <button type="button" class="app-btn" @click="resetForm">新建</button>
        </div>
        <pre v-if="status" class="out">{{ status }}</pre>
        <pre v-if="error" class="out error">{{ error }}</pre>
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
