<!--
  总谱：故事线 / 时间线 / Canon / 关系 / 节拍
  代码路径: kk_novel_ai/src/views/StoryView.vue
-->
<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import * as story from "../services/storyClient.js";
import * as project from "../services/projectClient.js";
import MindMapBoard from "../components/MindMapBoard.vue";
import CapsuleSwitch from "../components/CapsuleSwitch.vue";
import CastSidePanel from "../components/CastSidePanel.vue";
import { buildNovelMindTree } from "../utils/mindmapLayout.js";
import { appConfirmDelete } from "../services/confirmDialog.js";
import { useToastError } from "../services/toast.js";

const tab = ref("map");
const error = useToastError();
const message = ref("");
const selectedNode = ref(null);

const plot = ref({ arcs: [], promises: [] });
const timeline = ref({ calendar_note: "", events: [] });
const canon = ref({ facts: [] });
const relations = ref({ edges: [] });
const loreItems = ref([]);
const snapshots = ref({});

const chapters = computed(() => (appState.project && appState.project.chapters) || []);
const volumes = computed(() => (appState.project && appState.project.volumes) || []);
const currentChapter = computed(() =>
  chapters.value.find((c) => c.id === appState.chapterId) || null
);

const mindTree = computed(() =>
  buildNovelMindTree({
    title: (appState.project && appState.project.title) || "作品",
    volumes: volumes.value,
    chapters: chapters.value,
    plot: plot.value,
    timeline: timeline.value,
    canon: canon.value,
    relations: relations.value,
    loreItems: loreItems.value,
    snapshots: snapshots.value,
  })
);

const timelineSorted = computed(() =>
  [...(timeline.value.events || [])].sort((a, b) =>
    String(a.story_time).localeCompare(String(b.story_time))
  )
);

const focusDraft = ref({
  pov_lore_id: "",
  focus_arc_ids: "",
  must_do: "",
  must_not: "",
  reader_knows: "",
  character_knows: "",
  beatsText: "",
});

const beatProgress = ref({ current_beat_id: "", beats: {} });

const beatProgressRows = computed(() => {
  const ch = currentChapter.value;
  if (!ch || !(ch.beats || []).length) return [];
  const map = beatProgress.value.beats || {};
  return (ch.beats || []).map((b, i) => ({
    ...b,
    index: i + 1,
    status: map[b.id] || "pending",
  }));
});

async function loadBeatProgress() {
  if (!appState.projectRoot || !appState.chapterId) {
    beatProgress.value = { current_beat_id: "", beats: {} };
    return;
  }
  try {
    beatProgress.value = await project.getBeatProgress(appState.chapterId);
  } catch {
    beatProgress.value = { current_beat_id: "", beats: {} };
  }
}

async function onResetBeatProgress() {
  if (!appState.chapterId) return;
  try {
    await project.resetBeatProgress(appState.chapterId);
    await loadBeatProgress();
    message.value = "节拍进度已重置";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onSkipCurrentBeat() {
  const id = beatProgress.value.current_beat_id;
  if (!id || !appState.chapterId) return;
  try {
    beatProgress.value = await project.skipBeatProgress(appState.chapterId, id);
    message.value = "已跳过当前节拍";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function beatStatusLabel(st) {
  if (st === "in_progress") return "进行中";
  if (st === "completed") return "已完成";
  if (st === "skipped") return "已跳过";
  return "待写";
}

const graphNodes = computed(() => {
  const ids = new Set();
  for (const e of relations.value.edges || []) {
    ids.add(e.from_id);
    ids.add(e.to_id);
  }
  const list = [...ids];
  const n = Math.max(list.length, 1);
  const cx = 160;
  const cy = 140;
  const r = 110;
  return list.map((id, i) => {
    const ang = (Math.PI * 2 * i) / n - Math.PI / 2;
    const lore = loreItems.value.find((x) => x.id === id);
    return {
      id,
      label: (lore && lore.title) || id.slice(0, 8),
      x: cx + r * Math.cos(ang),
      y: cy + r * Math.sin(ang),
    };
  });
});

const graphEdges = computed(() => {
  const byId = Object.fromEntries(graphNodes.value.map((n) => [n.id, n]));
  return (relations.value.edges || [])
    .map((e) => {
      const a = byId[e.from_id];
      const b = byId[e.to_id];
      if (!a || !b) return null;
      return { ...e, x1: a.x, y1: a.y, x2: b.x, y2: b.y };
    })
    .filter(Boolean);
});

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

async function refreshAll() {
  if (!appState.projectRoot) return;
  error.value = "";
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
    timeline.value = t.timeline || { calendar_note: "", events: [] };
    canon.value = c.canon || { facts: [] };
    relations.value = r.relations || { edges: [] };
    loreItems.value = flattenScopedLore(lore);
    try {
      const memory = await project.getMemory();
      const next = {};
      for (const s of (memory && memory.chapter_snapshots) || []) {
        if (!s || !s.chapter_id) continue;
        const text = String(s.summary || "").trim();
        if (text) next[s.chapter_id] = text;
      }
      snapshots.value = next;
    } catch {
      snapshots.value = {};
    }
    syncFocusDraft();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onCastChanged() {
  await refreshAll();
}

function onCastSelect(item) {
  if (item && item.id) {
    selectedNode.value = {
      id: `char:${item.id}`,
      label: item.title,
      meta: "角色",
    };
  }
}

function syncFocusDraft() {
  const ch = currentChapter.value;
  if (!ch) return;
  focusDraft.value = {
    pov_lore_id: ch.pov_lore_id || "",
    focus_arc_ids: (ch.focus_arc_ids || []).join(", "),
    must_do: ch.must_do || "",
    must_not: ch.must_not || "",
    reader_knows: ch.reader_knows || "",
    character_knows: ch.character_knows || "",
    beatsText: (ch.beats || [])
      .map(
        (b) =>
          `${b.title || ""}|${b.purpose || ""}|${b.conflict || ""}|${b.emotion || ""}|${b.location || ""}`
      )
      .join("\n"),
  };
}

watch(() => appState.projectRoot, refreshAll);
watch(() => appState.chapterId, () => {
  syncFocusDraft();
  void loadBeatProgress();
});
watch(
  () => appState.project && appState.project.chapters,
  () => {
    syncFocusDraft();
    void loadBeatProgress();
  },
  { deep: true }
);
onMounted(async () => {
  await refreshAll();
  await loadBeatProgress();
});

function addArc() {
  plot.value.arcs.push({
    id: story.newId(),
    kind: "main",
    title: "新弧",
    goal: "",
    status: "active",
    progress_note: "",
    related_lore_ids: [],
  });
}

function addPromise() {
  plot.value.promises.push({
    id: story.newId(),
    text: "新承诺",
    status: "open",
    planted_chapter_id: appState.chapterId || null,
    arc_id: null,
  });
}

async function onSavePlot() {
  try {
    await story.savePlot(plot.value);
    message.value = "故事线已保存";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function addEvent() {
  timeline.value.events.push({
    id: story.newId(),
    story_time: "",
    title: "新事件",
    summary: "",
    location: "",
    chapter_ids: appState.chapterId ? [appState.chapterId] : [],
    participant_lore_ids: [],
  });
}

async function onSaveTimeline() {
  try {
    await story.saveTimeline(timeline.value);
    message.value = "时间线已保存";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function addFact() {
  canon.value.facts.push({
    id: story.newId(),
    text: "新事实",
    locked: false,
    evidence_chapter_ids: [],
    related_lore_ids: [],
    tags: [],
  });
}

async function onSaveCanon() {
  try {
    await story.saveCanon(canon.value);
    message.value = "Canon 已保存";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function addEdge() {
  const first = loreItems.value[0];
  const second = loreItems.value[1] || first;
  relations.value.edges.push({
    id: story.newId(),
    from_id: first ? first.id : "",
    to_id: second ? second.id : "",
    kind: "related",
    label: "",
    strength: 3,
    public: true,
  });
}

async function onSaveRelations() {
  try {
    await story.saveRelations(relations.value);
    message.value = "关系已保存（已同步 lore.links）";
    await refreshAll();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function parseBeats(text) {
  return (text || "")
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
    .map((line) => {
      const [title, purpose, conflict, emotion, location] = line.split("|");
      return {
        id: story.newId(),
        title: title || "",
        purpose: purpose || "",
        conflict: conflict || "",
        emotion: emotion || "",
        location: location || null,
      };
    });
}

async function onSaveFocus() {
  if (!appState.chapterId) {
    error.value = "请先选择章节";
    return;
  }
  try {
    await project.updateChapterMeta(appState.chapterId, {
      patch: {
        pov_lore_id: focusDraft.value.pov_lore_id || "",
        focus_arc_ids: focusDraft.value.focus_arc_ids
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
        must_do: focusDraft.value.must_do,
        must_not: focusDraft.value.must_not,
        reader_knows: focusDraft.value.reader_knows,
        character_knows: focusDraft.value.character_knows,
        beats: parseBeats(focusDraft.value.beatsText),
      },
    });
    message.value = "本章焦点/节拍已保存";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function removeAt(arr, idx) {
  if (
    !(await appConfirmDelete("删除这一项？", {
      title: "删除条目",
    }))
  ) {
    return;
  }
  arr.splice(idx, 1);
}

function onMapSelect(n) {
  selectedNode.value = n;
  if (n.id.startsWith("ch:")) {
    const id = n.id.slice(3);
    appState.chapterId = id;
    appState.activeNav = "editor";
  } else if (n.id.startsWith("branch:outline") || n.kind === "volume" || n.kind === "chapter") {
    /* stay */
  }
}
</script>

<template>
  <section class="panel story-panel">
    <h1 class="panel-heading">总谱</h1>
    <p v-if="!appState.projectRoot" class="muted">请先打开作品。</p>
    <template v-else>
      <div class="story-layout">
        <div class="story-main">
          <div class="subtabs">
            <button
              v-for="t in [
                { id: 'map', label: '思维导图' },
                { id: 'plot', label: '故事线' },
                { id: 'focus', label: '本章焦点' },
                { id: 'timeline', label: '时间线' },
                { id: 'canon', label: 'Canon' },
                { id: 'relations', label: '关系' },
                { id: 'beats', label: '节拍' },
              ]"
              :key="t.id"
              type="button"
              class="chip"
              :class="tab === t.id ? 'chip-active' : ''"
              @click="tab = t.id"
            >
              {{ t.label }}
            </button>
          </div>
          <p v-if="message" class="muted">{{ message }}</p>

          <div class="story-scroll">
            <div v-if="tab === 'map'" class="block">
              <p class="muted map-hint">
                导图含：大纲、角色、故事线、时间线、Canon、关系。右侧可添加/删除角色；下方表单 Tab 可编辑。
              </p>
              <MindMapBoard :tree="mindTree" :height="480" @select="onMapSelect" />
              <p v-if="selectedNode" class="muted select-hint">
                选中：{{ selectedNode.label }}
                <span v-if="selectedNode.meta"> — {{ selectedNode.meta }}</span>
              </p>
            </div>

            <div v-if="tab === 'plot'" class="block">
              <div class="row-actions">
                <button type="button" class="app-btn" @click="addArc">加弧</button>
                <button type="button" class="app-btn" @click="addPromise">加承诺</button>
                <button type="button" class="app-btn app-btn-primary" @click="onSavePlot">保存故事线</button>
              </div>
              <h3 class="sub">故事弧</h3>
              <div v-for="(a, i) in plot.arcs" :key="a.id" class="card">
                <div class="grid2">
                  <input v-model="a.title" placeholder="标题" />
                  <select v-model="a.kind">
                    <option value="main">main</option>
                    <option value="sub">sub</option>
                    <option value="foreshadow">foreshadow</option>
                  </select>
                  <select v-model="a.status">
                    <option value="planted">planted</option>
                    <option value="active">active</option>
                    <option value="resolved">resolved</option>
                    <option value="abandoned">abandoned</option>
                  </select>
                  <input v-model="a.id" class="muted-id" readonly />
                </div>
                <input v-model="a.goal" placeholder="目标" />
                <textarea v-model="a.progress_note" rows="2" placeholder="进度备注" />
                <button type="button" class="app-btn" @click="removeAt(plot.arcs, i)">删除</button>
              </div>
              <h3 class="sub">承诺 / 伏笔</h3>
              <div v-for="(p, i) in plot.promises" :key="p.id" class="card">
                <textarea v-model="p.text" rows="2" />
                <select v-model="p.status">
                  <option value="open">open</option>
                  <option value="paid">paid</option>
                  <option value="broken">broken</option>
                </select>
                <button type="button" class="app-btn" @click="removeAt(plot.promises, i)">删除</button>
              </div>
            </div>

            <div v-if="tab === 'focus' || tab === 'beats'" class="block">
              <p class="muted">当前章：{{ currentChapter?.title || "未选" }}</p>
              <div class="field">
                <label class="field-label">POV lore id</label>
                <select v-model="focusDraft.pov_lore_id">
                  <option value="">（无）</option>
                  <option v-for="l in loreItems" :key="l.id" :value="l.id">{{ l.title }} ({{ l.id.slice(0, 8) }})</option>
                </select>
              </div>
              <div class="field">
                <label class="field-label">焦点弧 id（逗号分隔）</label>
                <input v-model="focusDraft.focus_arc_ids" type="text" />
                <p class="hint muted">可选：{{ (plot.arcs || []).map((a) => a.id.slice(0, 8) + ':' + a.title).join(' · ') }}</p>
              </div>
              <div class="field">
                <label class="field-label">必达</label>
                <textarea v-model="focusDraft.must_do" rows="2" />
              </div>
              <div class="field">
                <label class="field-label">禁止</label>
                <textarea v-model="focusDraft.must_not" rows="2" />
              </div>
              <div class="field">
                <label class="field-label">读者已知</label>
                <textarea v-model="focusDraft.reader_knows" rows="2" />
              </div>
              <div class="field">
                <label class="field-label">角色已知</label>
                <textarea v-model="focusDraft.character_knows" rows="2" />
              </div>
              <div class="field">
                <label class="field-label">节拍（每行 title|purpose|conflict|emotion|location）</label>
                <textarea v-model="focusDraft.beatsText" rows="6" />
                <ul v-if="beatProgressRows.length" class="beat-progress-list">
                  <li
                    v-for="row in beatProgressRows"
                    :key="row.id"
                    class="beat-progress-row"
                    :class="'st-' + row.status"
                  >
                    <span class="beat-idx">{{ row.index }}</span>
                    <span class="beat-title">{{ row.title || row.purpose }}</span>
                    <span class="beat-badge">{{ beatStatusLabel(row.status) }}</span>
                  </li>
                </ul>
                <div v-if="beatProgressRows.length" class="row-actions beat-progress-actions">
                  <button type="button" class="app-btn" @click="onResetBeatProgress">重置进度</button>
                  <button type="button" class="app-btn" @click="onSkipCurrentBeat">跳过当前节拍</button>
                </div>
              </div>
              <button type="button" class="app-btn app-btn-primary" @click="onSaveFocus">保存本章焦点</button>
            </div>

            <div v-if="tab === 'timeline'" class="block">
              <div class="field">
                <label class="field-label">纪年说明</label>
                <input v-model="timeline.calendar_note" type="text" />
              </div>
              <div class="row-actions">
                <button type="button" class="app-btn" @click="addEvent">加事件</button>
                <button type="button" class="app-btn app-btn-primary" @click="onSaveTimeline">保存时间线</button>
              </div>
              <div class="tl-rail">
                <div v-for="(ev, i) in timelineSorted" :key="ev.id" class="tl-item">
                  <div class="tl-dot" />
                  <div class="tl-card card">
                    <div class="grid2">
                      <input v-model="ev.story_time" placeholder="故事日 Y1-冬-03" />
                      <input v-model="ev.title" placeholder="标题" />
                    </div>
                    <textarea v-model="ev.summary" rows="2" placeholder="摘要" />
                    <input v-model="ev.location" placeholder="地点" />
                    <button
                      type="button"
                      class="app-btn"
                      @click="removeAt(timeline.events, timeline.events.findIndex((x) => x.id === ev.id))"
                    >
                      删除
                    </button>
                  </div>
                </div>
              </div>
              <div v-if="!timelineSorted.length" class="muted">暂无事件，点「加事件」开始。</div>
            </div>

            <div v-if="tab === 'canon'" class="block">
              <div class="row-actions">
                <button type="button" class="app-btn" @click="addFact">加事实</button>
                <button type="button" class="app-btn app-btn-primary" @click="onSaveCanon">保存 Canon</button>
              </div>
              <div v-for="(f, i) in canon.facts" :key="f.id" class="card">
                <textarea v-model="f.text" rows="2" />
                <CapsuleSwitch v-model="f.locked" label="锁定（续写不可违背）" />
                <button type="button" class="app-btn" @click="removeAt(canon.facts, i)">删除</button>
              </div>
            </div>

            <div v-if="tab === 'relations'" class="block">
              <div class="row-actions">
                <button type="button" class="app-btn" @click="addEdge">加边</button>
                <button type="button" class="app-btn app-btn-primary" @click="onSaveRelations">保存关系</button>
              </div>
              <svg class="graph" viewBox="0 0 320 280" xmlns="http://www.w3.org/2000/svg">
                <line
                  v-for="e in graphEdges"
                  :key="e.id"
                  :x1="e.x1"
                  :y1="e.y1"
                  :x2="e.x2"
                  :y2="e.y2"
                  class="g-edge"
                />
                <g v-for="n in graphNodes" :key="n.id">
                  <circle :cx="n.x" :cy="n.y" r="22" class="g-node" />
                  <text :x="n.x" :y="n.y + 4" text-anchor="middle" class="g-label">{{ n.label.slice(0, 5) }}</text>
                  <title>{{ n.label }}</title>
                </g>
              </svg>
              <p class="muted">完整含大纲的导图见「思维导图」Tab；此处为关系网圆形布局。</p>
              <div v-for="(e, i) in relations.edges" :key="e.id" class="card">
                <div class="grid2">
                  <select v-model="e.from_id">
                    <option v-for="l in loreItems" :key="'f' + l.id" :value="l.id">{{ l.title }}</option>
                  </select>
                  <select v-model="e.to_id">
                    <option v-for="l in loreItems" :key="'t' + l.id" :value="l.id">{{ l.title }}</option>
                  </select>
                  <input v-model="e.kind" placeholder="kind" />
                  <input v-model="e.label" placeholder="label" />
                  <input v-model.number="e.strength" type="number" min="1" max="5" />
                </div>
                <button type="button" class="app-btn" @click="removeAt(relations.edges, i)">删除</button>
              </div>
            </div>

          </div>
        </div>

        <CastSidePanel class="story-cast" @changed="onCastChanged" @select="onCastSelect" />
      </div>
    </template>
  </section>
</template>

<style scoped>
.story-panel {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.story-layout {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
  gap: 12px;
  overflow: hidden;
}
.story-main {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.story-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 4px;
}
.story-cast {
  min-height: 0;
}
.subtabs {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 10px 0 14px;
  flex-shrink: 0;
}
.chip {
  border: none;
  background: var(--chip-bg);
  color: var(--muted);
  border-radius: var(--radius-pill);
  padding: 4px 10px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
}
.chip-active {
  background: var(--accent-soft);
  color: var(--accent-hover);
}
.row-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 10px;
}
.card {
  padding: 12px;
  margin-bottom: 10px;
  border-radius: var(--radius-md);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.grid2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
.sub {
  font-size: 14px;
  margin: 12px 0 8px;
}
.muted-id {
  opacity: 0.6;
  font-size: 11px;
}
.hint {
  font-size: 11px;
  margin-top: 4px;
}
.check {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}
.graph {
  width: 100%;
  max-width: 360px;
  height: 280px;
  background: var(--surface-solid);
  border-radius: var(--radius-md);
  margin-bottom: 12px;
}
.g-edge {
  stroke: var(--muted);
  stroke-width: 1.5;
  opacity: 0.7;
}
.g-node {
  fill: var(--accent-soft);
  stroke: var(--accent-hover);
}
.g-label {
  font-size: 9px;
  fill: var(--text);
}
.error {
  color: var(--error);
}
.timeline-card {
  border-left: 3px solid var(--accent-hover);
}
.map-hint,
.select-hint {
  font-size: 12px;
  margin-bottom: 10px;
}
.tl-rail {
  position: relative;
  margin-top: 12px;
  padding-left: 18px;
}
.tl-rail::before {
  content: "";
  position: absolute;
  left: 5px;
  top: 8px;
  bottom: 8px;
  width: 2px;
  background: var(--accent-soft);
}
.tl-item {
  position: relative;
  margin-bottom: 12px;
}
.tl-dot {
  position: absolute;
  left: -16px;
  top: 18px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--accent-hover);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.tl-card {
  margin: 0;
}
.beat-progress-list {
  list-style: none;
  margin: 8px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.beat-progress-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  padding: 4px 8px;
  border-radius: 6px;
  background: var(--surface-2, rgba(0, 0, 0, 0.04));
}
.beat-progress-row.st-in_progress {
  outline: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
}
.beat-progress-row.st-completed {
  opacity: 0.65;
}
.beat-idx {
  font-weight: 600;
  min-width: 1.2em;
}
.beat-title {
  flex: 1;
}
.beat-badge {
  font-size: 11px;
  opacity: 0.85;
}
.beat-progress-actions {
  margin-top: 8px;
}
@media (max-width: 960px) {
  .story-layout {
    grid-template-columns: 1fr;
    overflow-y: auto;
  }
  .story-cast {
    min-height: 280px;
  }
}
</style>
