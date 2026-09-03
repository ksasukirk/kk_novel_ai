<!--
  大纲视图 + 卷弧 + 思维导图 + 右侧角色栏
  代码路径: kk_novel_ai/src/views/OutlineView.vue
-->
<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import * as project from "../services/projectClient.js";
import * as story from "../services/storyClient.js";
import MindMapBoard from "../components/MindMapBoard.vue";
import CastSidePanel from "../components/CastSidePanel.vue";
import { buildNovelMindTree } from "../utils/mindmapLayout.js";
import { useToastError } from "../services/toast.js";

const error = useToastError();
const drafts = ref({});
const volumeDrafts = ref({});
const bookOutlineDraft = ref("");
const plot = ref({ arcs: [], promises: [] });
const timeline = ref({ events: [] });
const canon = ref({ facts: [] });
const relations = ref({ edges: [] });
const loreItems = ref([]);
const showMap = ref(true);

const chapters = computed(() => (appState.project && appState.project.chapters) || []);
const volumes = computed(() => (appState.project && appState.project.volumes) || []);

watch(
  () => appState.project && appState.project.book_outline,
  (v) => {
    bookOutlineDraft.value = String(v || "");
  },
  { immediate: true }
);
const outlineMindTree = computed(() => {
  const full = buildNovelMindTree({
    title: (appState.project && appState.project.title) || "作品",
    volumes: volumes.value,
    chapters: chapters.value,
    plot: plot.value,
    timeline: timeline.value,
    canon: canon.value,
    relations: relations.value,
    loreItems: loreItems.value,
  });
  // 大纲页：大纲 + 角色 + 故事线缩略
  return {
    id: full.id,
    label: full.label,
    kind: "root",
    meta: "大纲导图",
    children: (full.children || []).filter((c) =>
      ["branch:outline", "branch:characters", "branch:plot"].includes(c.id)
    ),
  };
});

watch(
  chapters,
  (list) => {
    const next = {};
    for (const ch of list) {
      next[ch.id] = {
        title: ch.title,
        summary: ch.summary || "",
        must_do: ch.must_do || "",
        must_not: ch.must_not || "",
      };
    }
    drafts.value = next;
  },
  { immediate: true }
);

watch(
  volumes,
  (list) => {
    const next = {};
    for (const v of list) {
      next[v.id] = {
        title: v.title,
        arc_goal: v.arc_goal || "",
        arc_summary: v.arc_summary || "",
      };
    }
    volumeDrafts.value = next;
  },
  { immediate: true }
);

function flattenScopedLore(scoped) {
  const local = (scoped.local || []).map((row) => row.entry);
  const global = (scoped.global || []).map((row) => row.entry);
  const byTitle = new Map();
  for (const e of global) {
    if (!e) continue;
    byTitle.set((e.title || "").trim() || e.id, e);
  }
  for (const e of local) {
    if (!e) continue;
    byTitle.set((e.title || "").trim() || e.id, e);
  }
  return [...byTitle.values()];
}

async function loadStoryLite() {
  if (!appState.projectRoot) return;
  try {
    await project.ensureCharactersLink();
    const [p, t, c, r, lore] = await Promise.all([
      story.getPlot(),
      story.getTimeline(),
      story.getCanon(),
      story.getRelations(),
      project.listLoreScoped(),
    ]);
    plot.value = p.plot || { arcs: [], promises: [] };
    timeline.value = t.timeline || { events: [] };
    canon.value = c.canon || { facts: [] };
    relations.value = r.relations || { edges: [] };
    loreItems.value = flattenScopedLore(lore);
  } catch {
    /* 无总谱也可只看大纲树 */
  }
}

watch(() => appState.projectRoot, loadStoryLite);
onMounted(loadStoryLite);

async function saveOne(id) {
  error.value = "";
  const d = drafts.value[id];
  if (!d) return;
  try {
    await project.updateChapterMeta(id, {
      title: d.title,
      summary: d.summary,
      patch: { must_do: d.must_do, must_not: d.must_not },
    });
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function saveStyle() {
  if (!appState.project) return;
  try {
    await project.saveProjectMeta({ ...appState.project });
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function saveBookOutlineField() {
  if (!appState.project) return;
  error.value = "";
  try {
    await project.saveProjectMeta({
      ...appState.project,
      book_outline: String(bookOutlineDraft.value || ""),
    });
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function saveVolume(id) {
  if (!appState.project) return;
  const d = volumeDrafts.value[id];
  if (!d) return;
  try {
    const vols = (appState.project.volumes || []).map((v) => {
      if (v.id !== id) return v;
      return { ...v, title: d.title, arc_goal: d.arc_goal, arc_summary: d.arc_summary };
    });
    await project.saveProjectMeta({ ...appState.project, volumes: vols });
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function onMapSelect(n) {
  if (n.id && n.id.startsWith("ch:")) {
    appState.chapterId = n.id.slice(3);
    appState.activeNav = "editor";
  }
}

async function onCastChanged() {
  await loadStoryLite();
}
</script>

<template>
  <section class="panel outline-panel">
    <h1 class="panel-heading">大纲</h1>
    <p v-if="!appState.projectRoot" class="muted">请先打开作品。完整总谱导图见侧栏「总谱」。</p>
    <template v-else>
      <div class="outline-layout">
        <div class="outline-main">
          <div class="outline-scroll">
            <div class="map-head">
              <h2 class="sub">结构导图</h2>
              <button type="button" class="app-btn" @click="showMap = !showMap">
                {{ showMap ? "收起导图" : "展开导图" }}
              </button>
              <button type="button" class="app-btn" @click="loadStoryLite">刷新</button>
            </div>
            <MindMapBoard v-if="showMap" :tree="outlineMindTree" :height="360" @select="onMapSelect" />

            <div class="field" style="margin-top: 16px">
              <label class="field-label">全书大纲</label>
              <textarea
                v-model="bookOutlineDraft"
                rows="6"
                placeholder="全书主线冲突 / 分章钩子 / 人物关系（与写作页「按纲生成」共用）"
              />
              <button type="button" class="app-btn" style="margin-top: 8px" @click="saveBookOutlineField">
                保存全书大纲
              </button>
            </div>

            <div class="field" style="margin-top: 16px">
              <label class="field-label">文风</label>
              <textarea v-model="appState.project.style" rows="3" @change="saveStyle" />
            </div>

            <h2 class="sub">卷弧</h2>
            <div v-for="vol in volumes" :key="vol.id" class="outline-card">
              <input v-model="volumeDrafts[vol.id].title" type="text" placeholder="卷名" />
              <input v-model="volumeDrafts[vol.id].arc_goal" type="text" placeholder="本卷目标" />
              <textarea v-model="volumeDrafts[vol.id].arc_summary" rows="2" placeholder="本卷弧线摘要" />
              <button type="button" class="app-btn" @click="saveVolume(vol.id)">保存卷弧</button>
            </div>

            <h2 class="sub">章纲</h2>
            <div v-for="ch in chapters" :key="ch.id" class="outline-card">
              <input v-model="drafts[ch.id].title" type="text" />
              <textarea v-model="drafts[ch.id].summary" rows="3" placeholder="本章冲突 / 推进 / 钩子" />
              <textarea v-model="drafts[ch.id].must_do" rows="2" placeholder="本章必达（焦点）" />
              <textarea v-model="drafts[ch.id].must_not" rows="2" placeholder="本章禁止" />
              <button type="button" class="app-btn" @click="saveOne(ch.id)">保存本章纲</button>
            </div>
          </div>
        </div>
        <CastSidePanel class="outline-cast" @changed="onCastChanged" />
      </div>
    </template>
  </section>
</template>

<style scoped>
.outline-panel {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.outline-layout {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
  gap: 12px;
  overflow: hidden;
}
.outline-main {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.outline-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 4px;
}
.outline-cast {
  min-height: 0;
}
.sub {
  font-size: 14px;
  margin: 18px 0 8px;
}
.map-head {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  margin-bottom: 8px;
}
.map-head .sub {
  margin: 0;
  flex: 1;
}
.outline-card {
  margin-top: 14px;
  padding: 16px 18px;
  border: none;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow);
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.outline-card .app-btn {
  align-self: flex-start;
}
.error {
  color: var(--error);
}
@media (max-width: 960px) {
  .outline-layout {
    grid-template-columns: 1fr;
    overflow-y: auto;
  }
  .outline-cast {
    min-height: 280px;
  }
}
</style>
