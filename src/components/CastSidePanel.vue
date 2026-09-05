<!--
  本篇角色侧栏：右侧列表添加 / 删除
  代码路径: kk_novel_ai/src/components/CastSidePanel.vue
-->
<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import * as project from "../services/projectClient.js";
import CapsuleSwitch from "./CapsuleSwitch.vue";
import { appConfirmDelete } from "../services/confirmDialog.js";
import { useToastError } from "../services/toast.js";

const emit = defineEmits(["changed", "select"]);

const items = ref([]);
const rosterPath = ref("");
const error = useToastError();
const busy = ref(false);
const newName = ref("");
/** 新建写入全局仓（否则写本篇 lore） */
const writeGlobal = ref(false);

const characters = computed(() => {
  const byTitle = new Map();
  for (const it of items.value) {
    if (it.kind !== "character") continue;
    const key = (it.title || "").trim() || it.id;
    // 本篇覆盖同名全局
    if (it.scope === "local" || !byTitle.has(key)) {
      byTitle.set(key, it);
    }
  }
  return [...byTitle.values()].sort((a, b) =>
    String(a.title).localeCompare(String(b.title), "zh")
  );
});

async function refresh() {
  error.value = "";
  if (!appState.projectRoot) {
    items.value = [];
    return;
  }
  try {
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
  } catch (e) {
    error.value = String(e.message || e);
    items.value = [];
  }
}

watch(() => appState.projectRoot, refresh);
watch(() => appState.castRevision, () => {
  if (appState.projectRoot) void refresh();
});
onMounted(refresh);

defineExpose({ refresh });

async function addCharacter() {
  const title = newName.value.trim();
  if (!title) {
    error.value = "先填角色名";
    return;
  }
  if (!appState.projectRoot) {
    error.value = "请先打开作品";
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    let root = appState.projectRoot;
    if (writeGlobal.value) {
      const ens = await project.ensureCharacterRoster();
      root = ens.root;
      rosterPath.value = root;
    }
    await project.upsertLoreAt(root, {
      id: "",
      kind: "character",
      title,
      content: `${title}（待补设定）`,
      keywords: [title],
      links: [],
      attrs: {},
      unique: true,
      sources: [],
      updated_at: "",
    });
    newName.value = "";
    await refresh();
    emit("changed", characters.value);
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    busy.value = false;
  }
}

async function removeCharacter(item) {
  if (!item || !item._root || !item.id) return;
  const where = item.scope === "global" ? "全局角色仓" : "本篇设定";
  if (
    !(await appConfirmDelete(`从${where}删除「${item.title}」？`, {
      title: "删除角色",
    }))
  ) {
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    await project.deleteLoreAt(item._root, item.id);
    await refresh();
    emit("changed", characters.value);
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    busy.value = false;
  }
}

function onSelect(item) {
  emit("select", item);
}
</script>

<template>
  <aside class="cast-panel">
    <div class="cast-head">
      <h2 class="cast-title">角色</h2>
    </div>

    <div class="cast-add">
      <input
        v-model="newName"
        type="text"
        placeholder="角色名"
        @keydown.enter.prevent="addCharacter"
      />
      <CapsuleSwitch v-model="writeGlobal" label="全局仓" />
      <button
        type="button"
        class="app-btn app-btn-primary"
        :disabled="busy"
        @click="addCharacter"
      >
        添加
      </button>
    </div>

    <div class="cast-list">
      <div
        v-for="item in characters"
        :key="item.id"
        class="cast-item"
        @click="onSelect(item)"
      >
        <div class="cast-meta">
          <strong>{{ item.title }}</strong>
          <span class="chip kind-tag" :class="item.scope === 'global' ? 'chip-global' : 'chip-active'">
            {{ item.scope === "global" ? "全局" : "本篇" }}
          </span>
          <p class="snippet">
            {{ (item.content || "").slice(0, 48)
            }}{{ (item.content || "").length > 48 ? "…" : "" }}
          </p>
        </div>
        <button
          type="button"
          class="app-btn app-btn-danger"
          :disabled="busy"
          @click.stop="removeCharacter(item)"
        >
          删除
        </button>
      </div>
      <p v-if="!characters.length" class="muted empty">还没有角色，上面填名字添加。</p>
    </div>

  </aside>
</template>

<style scoped>
.cast-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  background: var(--surface-solid);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  padding: 8px;
  overflow: hidden;
}
.cast-head {
  flex-shrink: 0;
}
.cast-title {
  margin: 0 0 6px;
  font-size: 14px;
  font-weight: 700;
}
.cast-add {
  flex-shrink: 0;
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--accent-soft, rgba(0, 0, 0, 0.06));
}
.cast-add input {
  flex: 1;
  min-width: 72px;
}
.cast-add .app-btn {
  padding: 4px 12px;
  font-size: 12px;
}
.cast-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 2px;
}
.cast-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
  padding: 10px 12px;
  margin-bottom: 8px;
  border-radius: var(--radius-md);
  background: var(--panel, var(--bg));
  cursor: pointer;
  transition: background 0.15s ease;
}
.cast-item:hover {
  background: var(--accent-soft);
}
.cast-meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.kind-tag {
  align-self: flex-start;
  pointer-events: none;
  font-size: 11px;
  padding: 2px 8px;
}
.chip-global {
  background: var(--chip-bg);
  color: var(--muted);
}
.snippet {
  margin: 0;
  font-size: 12px;
  color: var(--muted);
  line-height: 1.35;
}
.empty {
  font-size: 12px;
  padding: 8px 0;
}
.cast-error {
  flex-shrink: 0;
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--error);
}
</style>
