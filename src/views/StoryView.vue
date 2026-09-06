<!--
  总谱：故事线 / 时间线 / Canon / 关系 / 节拍
  代码路径: kk_novel_ai/src/views/StoryView.vue
-->
<script setup>
import { computed, onActivated, onMounted, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import * as story from "../services/storyClient.js";
import * as project from "../services/projectClient.js";
import MindMapBoard from "../components/MindMapBoard.vue";
import RelationForceGraph from "../components/RelationForceGraph.vue";
import CapsuleSwitch from "../components/CapsuleSwitch.vue";
import CastSidePanel from "../components/CastSidePanel.vue";
import { buildNovelMindTree } from "../utils/mindmapLayout.js";
import { appConfirm, appConfirmDelete } from "../services/confirmDialog.js";
import { useToastError } from "../services/toast.js";
import { t } from "../i18n/index.js";
import { outlineQueueState } from "../services/outlineQueue.js";
import { sectionQueueState } from "../services/sectionQueue.js";
import {
  cancelStoryRebuild,
  rebuildStoryFromExistingWork,
  storyRebuildState,
} from "../services/storySync.js";
import {
  generateImageFile,
  hashSourceText,
  illustrationRel,
  imageGenState,
  loadIllustrationDataUrl,
  persistIllustrationAfterGen,
  promptFromShot,
  runBeatsToStoryboard,
} from "../services/illustration.js";
import { createIllustrationBlock, cryptoRandomId } from "../utils/genBlock.js";

defineProps({
  embedded: { type: Boolean, default: false },
});

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
    title: (appState.project && appState.project.title) || t("outline.workDefault"),
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
const storyboard = ref({ style_prefix: "", negative: "", chapters: [] });
const shotThumbs = ref({});
const boardBusy = ref(false);

function jsonSnap(v) {
  try {
    return JSON.stringify(v);
  } catch {
    return "";
  }
}

const loadedSnap = {
  plot: "",
  timeline: "",
  canon: "",
  relations: "",
  storyboard: "",
  focus: "",
  focusChapterId: "",
};

function isBlockDirty(key, current) {
  const base = loadedSnap[key];
  if (!base) return false;
  return jsonSnap(current) !== base;
}

function takeIfClean(key, currentRef, incoming) {
  if (isBlockDirty(key, currentRef.value)) return;
  currentRef.value = incoming;
  loadedSnap[key] = jsonSnap(incoming);
}

function noteBlockSaved(key, current) {
  loadedSnap[key] = jsonSnap(current);
}

function resetLoadedSnaps() {
  loadedSnap.plot = "";
  loadedSnap.timeline = "";
  loadedSnap.canon = "";
  loadedSnap.relations = "";
  loadedSnap.storyboard = "";
  loadedSnap.focus = "";
  loadedSnap.focusChapterId = "";
}

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
    message.value = t("story.beatReset");
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onSkipCurrentBeat() {
  const id = beatProgress.value.current_beat_id;
  if (!id || !appState.chapterId) return;
  try {
    beatProgress.value = await project.skipBeatProgress(appState.chapterId, id);
    message.value = t("story.beatSkipped");
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function beatStatusLabel(st) {
  if (st === "in_progress") return t("story.beatInProgress");
  if (st === "completed") return t("editor.done");
  if (st === "skipped") return t("story.beatSkippedStatus");
  return t("editor.pending");
}

const chapterShots = computed(() => {
  const cid = appState.chapterId;
  if (!cid) return [];
  const ch = (storyboard.value.chapters || []).find((c) => c.chapter_id === cid);
  return ch ? ch.shots || [] : [];
});

function ensureBoardChapter() {
  const cid = appState.chapterId;
  if (!cid) return null;
  if (!Array.isArray(storyboard.value.chapters)) storyboard.value.chapters = [];
  let ch = storyboard.value.chapters.find((c) => c.chapter_id === cid);
  if (!ch) {
    ch = { chapter_id: cid, shots: [] };
    storyboard.value.chapters.push(ch);
  }
  if (!Array.isArray(ch.shots)) ch.shots = [];
  return ch;
}

function addShot() {
  const ch = ensureBoardChapter();
  if (!ch) {
    error.value = t("story.needChapter");
    return;
  }
  const beats = (currentChapter.value && currentChapter.value.beats) || [];
  ch.shots.push({
    id: story.newId(),
    beat_id: beats[0] ? beats[0].id : "",
    seq: ch.shots.length + 1,
    location: "",
    character_lore_ids: [],
    visual: "",
    dialogue: "",
    mood: "",
    note: "",
    image: null,
  });
}

function removeShot(i) {
  const ch = ensureBoardChapter();
  if (!ch) return;
  ch.shots.splice(i, 1);
  ch.shots.forEach((s, idx) => {
    s.seq = idx + 1;
  });
}

function moveShot(i, dir) {
  const ch = ensureBoardChapter();
  if (!ch) return;
  const j = i + dir;
  if (j < 0 || j >= ch.shots.length) return;
  const tmp = ch.shots[i];
  ch.shots[i] = ch.shots[j];
  ch.shots[j] = tmp;
  ch.shots.forEach((s, idx) => {
    s.seq = idx + 1;
  });
}

function loreTitle(id) {
  const e = loreItems.value.find((x) => x.id === id);
  return e ? e.title : id;
}

function toggleShotChar(shot, loreId) {
  const ids = Array.isArray(shot.character_lore_ids) ? shot.character_lore_ids : [];
  const i = ids.indexOf(loreId);
  if (i >= 0) ids.splice(i, 1);
  else ids.push(loreId);
  shot.character_lore_ids = ids;
}

async function refreshShotThumbs() {
  const next = { ...shotThumbs.value };
  for (const s of chapterShots.value) {
    const rel = s.image && s.image.rel;
    if (!rel) continue;
    if (next[rel]) continue;
    next[rel] = await loadIllustrationDataUrl(rel);
  }
  shotThumbs.value = next;
}

watch(chapterShots, () => {
  void refreshShotThumbs();
}, { deep: true });

async function onSaveStoryboard() {
  try {
    await story.saveStoryboard(storyboard.value);
    noteBlockSaved("storyboard", storyboard.value);
    message.value = t("story.boardSaved");
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onGenerateStoryboard() {
  if (!appState.chapterId) {
    error.value = t("story.needChapter");
    return;
  }
  boardBusy.value = true;
  try {
    const shots = await runBeatsToStoryboard({});
    const ch = ensureBoardChapter();
    const beats = (currentChapter.value && currentChapter.value.beats) || [];
    ch.shots = (shots || []).map((s, i) => {
      let beatId = String(s.beat_id || "");
      if (!beatId && beats[i]) beatId = beats[i].id;
      const titles = Array.isArray(s.character_titles) ? s.character_titles : [];
      const ids = titles
        .map((t) => {
          const hit = loreItems.value.find(
            (e) => (e.title || "") === t || (e.title || "").endsWith(t)
          );
          return hit ? hit.id : "";
        })
        .filter(Boolean);
      return {
        id: story.newId(),
        beat_id: beatId || null,
        seq: Number(s.seq) || i + 1,
        location: String(s.location || ""),
        character_lore_ids: ids,
        visual: String(s.visual || ""),
        dialogue: String(s.dialogue || ""),
        mood: String(s.mood || ""),
        note: String(s.note || ""),
        image: null,
      };
    });
    await story.saveStoryboard(storyboard.value);
    noteBlockSaved("storyboard", storyboard.value);
    message.value = t("story.shotsGenerated", { n: ch.shots.length });
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    boardBusy.value = false;
  }
}

async function onGenerateShotImage(shot) {
  if (!shot) return;
  boardBusy.value = true;
  try {
    const drafted = await promptFromShot(shot, loreItems.value, storyboard.value);
    const { openImagePromptDialog } = await import("../services/illustration.js");
    const confirmed = await openImagePromptDialog({
      title: t("story.genShotTitle"),
      prompt: drafted.prompt,
      negative: drafted.negative || storyboard.value.negative || "",
      caption: drafted.caption,
    });
    if (!confirmed) return;
    const id = shot.id || cryptoRandomId();
    const rel = illustrationRel(appState.chapterId, `shot-${id}`);
    const gen = await generateImageFile({
      rel,
      prompt: confirmed.prompt,
      negative: confirmed.negative,
    });
    shot.image = {
      rel: gen.rel || rel,
      prompt: confirmed.prompt,
      negative: confirmed.negative,
      seed: null,
      model: (gen && gen.model) || "",
      source_hash: await hashSourceText(shot.visual || ""),
    };
    await story.saveStoryboard(storyboard.value);
    noteBlockSaved("storyboard", storyboard.value);
    shotThumbs.value = { ...shotThumbs.value, [shot.image.rel]: "" };
    await refreshShotThumbs();
    message.value = t("story.shotImageDone");
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    boardBusy.value = false;
  }
}

async function onInsertShotIntoChapter(shot) {
  if (!shot || !appState.chapterId) return;
  try {
    if (!appState.chapterBranchDoc) {
      await project.loadChapter(appState.chapterId);
    }
    let rel = shot.image && shot.image.rel;
    if (!rel) {
      await onGenerateShotImage(shot);
      rel = shot.image && shot.image.rel;
    }
    if (!rel) return;
    const genBlocks = (appState.chapterBlocks || []).filter((b) => b.type === "gen");
    const genKey = genBlocks.length ? genBlocks[genBlocks.length - 1].key : "";
    const illus = createIllustrationBlock({
      caption: shot.visual ? String(shot.visual).slice(0, 40) : t("story.shotCaption"),
      rel,
      prompt: (shot.image && shot.image.prompt) || "",
      negative: (shot.image && shot.image.negative) || "",
      model: (shot.image && shot.image.model) || "",
      source_hash: (shot.image && shot.image.source_hash) || "",
      source: { kind: "shot", block_key: genKey, shot_id: shot.id },
    });
    await persistIllustrationAfterGen(genKey, illus);
    message.value = t("story.inserted");
  } catch (e) {
    error.value = String(e.message || e);
  }
}

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

async function reloadLore() {
  if (!appState.projectRoot) return;
  try {
    await project.ensureCharactersLink();
    const lore = await project.listLoreScoped();
    loreItems.value = flattenScopedLore(lore);
  } catch {
    /* 无设定也可 */
  }
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
    takeIfClean("plot", plot, p.plot || { arcs: [], promises: [] });
    takeIfClean("timeline", timeline, t.timeline || { calendar_note: "", events: [] });
    takeIfClean("canon", canon, c.canon || { facts: [] });
    takeIfClean("relations", relations, r.relations || { edges: [] });
    loreItems.value = flattenScopedLore(lore);
    try {
      const sb = await story.getStoryboard();
      takeIfClean(
        "storyboard",
        storyboard,
        sb.storyboard || { style_prefix: "", negative: "", chapters: [] }
      );
    } catch {
      takeIfClean("storyboard", storyboard, { style_prefix: "", negative: "", chapters: [] });
    }
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
    syncFocusDraft(false);
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
      meta: t("common.character"),
    };
  }
}

function onGraphSelect(n) {
  if (!n || !n.id) {
    return;
  }
  selectedNode.value = {
    id: `char:${n.id}`,
    label: n.label,
    meta: t("common.character"),
  };
}

function syncFocusDraft(force) {
  const ch = currentChapter.value;
  const cid = appState.chapterId || "";
  if (!force && loadedSnap.focusChapterId === cid && isBlockDirty("focus", focusDraft.value)) {
    return;
  }
  if (!ch) {
    if (force) {
      focusDraft.value = {
        pov_lore_id: "",
        focus_arc_ids: "",
        must_do: "",
        must_not: "",
        reader_knows: "",
        character_knows: "",
        beatsText: "",
      };
      loadedSnap.focus = jsonSnap(focusDraft.value);
      loadedSnap.focusChapterId = cid;
    }
    return;
  }
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
  loadedSnap.focus = jsonSnap(focusDraft.value);
  loadedSnap.focusChapterId = cid;
}

watch(
  () => appState.projectRoot,
  () => {
    resetLoadedSnaps();
    void refreshAll();
  }
);
watch(() => appState.storyRevision, () => {
  if (appState.projectRoot) void refreshAll();
});
watch(() => appState.castRevision, () => {
  if (appState.projectRoot) void reloadLore();
});
watch(() => appState.chapterId, () => {
  syncFocusDraft(true);
  void loadBeatProgress();
});
watch(
  () => appState.project && appState.project.chapters,
  () => {
    void loadBeatProgress();
  },
  { deep: true }
);
onMounted(async () => {
  await refreshAll();
  await loadBeatProgress();
});
onActivated(() => {
  if (appState.projectRoot) void reloadLore();
});

function addArc() {
  plot.value.arcs.push({
    id: story.newId(),
    kind: "main",
    title: t("story.newArc"),
    goal: "",
    status: "active",
    progress_note: "",
    related_lore_ids: [],
  });
}

function addPromise() {
  plot.value.promises.push({
    id: story.newId(),
    text: t("story.newPromise"),
    status: "open",
    planted_chapter_id: appState.chapterId || null,
    arc_id: null,
  });
}

async function onSavePlot() {
  try {
    await story.savePlot(plot.value);
    noteBlockSaved("plot", plot.value);
    message.value = t("story.plotSaved");
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function addEvent() {
  timeline.value.events.push({
    id: story.newId(),
    story_time: "",
    title: t("story.newEvent"),
    summary: "",
    location: "",
    chapter_ids: appState.chapterId ? [appState.chapterId] : [],
    participant_lore_ids: [],
  });
}

async function onSaveTimeline() {
  try {
    await story.saveTimeline(timeline.value);
    noteBlockSaved("timeline", timeline.value);
    message.value = t("story.timelineSaved");
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function addFact() {
  canon.value.facts.push({
    id: story.newId(),
    text: t("story.newFact"),
    locked: false,
    evidence_chapter_ids: [],
    related_lore_ids: [],
    tags: [],
  });
}

async function onSaveCanon() {
  try {
    await story.saveCanon(canon.value);
    noteBlockSaved("canon", canon.value);
    message.value = t("story.canonSaved");
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
    noteBlockSaved("relations", relations.value);
    message.value = t("story.relationsSaved");
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
    error.value = t("story.needChapter");
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
    noteBlockSaved("focus", focusDraft.value);
    loadedSnap.focusChapterId = appState.chapterId;
    message.value = t("story.focusSaved");
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function removeAt(arr, idx) {
  if (
    !(await appConfirmDelete(t("story.deleteItemQ"), {
      title: t("story.deleteItemTitle"),
    }))
  ) {
    return;
  }
  arr.splice(idx, 1);
}

const rebuildBusy = computed(() => !!storyRebuildState.running);
const rebuildBlocked = computed(
  () =>
    !appState.projectRoot ||
    !!appState.generating ||
    !!outlineQueueState.running ||
    !!sectionQueueState.running
);

async function onRebuildStory() {
  if (rebuildBusy.value || rebuildBlocked.value) return;
  error.value = "";
  message.value = "";
  const ok = await appConfirm(t("story.rebuildConfirm"), {
    title: t("story.rebuildConfirmTitle"),
    confirmText: t("story.rebuildStart"),
    cancelText: t("common.cancel"),
    danger: true,
  });
  if (!ok) return;
  try {
    if (appState.dirty) await project.saveChapter();
    const r = await rebuildStoryFromExistingWork();
    await refreshAll();
    if (r && r.cancelled) {
      message.value =
        r.failed && r.failed.length
          ? t("story.rebuildCancelledFail", { ok: r.ok, n: r.failed.length })
          : t("story.rebuildCancelledOk", { ok: r.ok });
    } else if (r && r.failed && r.failed.length) {
      message.value = t("story.rebuildPartial", {
        ok: r.ok,
        failed: r.failed.join(t("common.listSep")),
      });
    } else {
      message.value = t("story.rebuildDone", { n: (r && r.ok) || 0 });
    }
  } catch (e) {
    error.value = String((e && e.message) || e);
  }
}

function onCancelRebuild() {
  cancelStoryRebuild();
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
  <section class="panel story-panel" :class="{ 'story-panel-embed': embedded }">
    <div v-if="!embedded" class="story-head">
      <h1 class="panel-heading">{{ $t("story.title") }}</h1>
      <div v-if="appState.projectRoot" class="story-head-actions">
        <button
          v-if="rebuildBusy"
          type="button"
          class="app-btn"
          @click="onCancelRebuild"
        >
          {{ $t("story.cancelRebuild") }}
        </button>
        <button
          type="button"
          class="app-btn app-btn-primary"
          :disabled="rebuildBusy || rebuildBlocked"
          :title="$t('story.rebuildHint')"
          @click="onRebuildStory"
        >
          {{ rebuildBusy ? $t("story.rebuilding") : $t("story.rebuild") }}
        </button>
      </div>
    </div>
    <p v-if="rebuildBusy" class="muted rebuild-progress">
      {{ $t("story.rebuildProgress", { index: storyRebuildState.index, total: storyRebuildState.total }) }}
      <span v-if="storyRebuildState.chapterTitle"> · {{ storyRebuildState.chapterTitle }}</span>
    </p>
    <p v-if="!appState.projectRoot" class="muted">{{ $t("story.needProject") }}</p>
    <template v-else>
      <div class="story-layout">
        <div class="story-main">
          <div class="subtabs">
            <div class="subtabs-chips">
              <button
                v-for="t in [
                  { id: 'map', label: $t('story.tabMap') },
                  { id: 'plot', label: $t('story.tabPlot') },
                  { id: 'focus', label: $t('story.tabFocus') },
                  { id: 'timeline', label: $t('story.tabTimeline') },
                  { id: 'canon', label: $t('story.tabCanon') },
                  { id: 'relations', label: $t('story.tabRelations') },
                  { id: 'beats', label: $t('story.tabBeats') },
                  { id: 'board', label: $t('story.tabBoard') },
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
            <div v-if="embedded && appState.projectRoot" class="subtabs-actions">
              <button
                v-if="rebuildBusy"
                type="button"
                class="app-btn"
                @click="onCancelRebuild"
              >
                {{ $t("story.cancelRebuild") }}
              </button>
              <button
                type="button"
                class="app-btn app-btn-primary"
                :disabled="rebuildBusy || rebuildBlocked"
                :title="$t('story.rebuildHint')"
                @click="onRebuildStory"
              >
                {{ rebuildBusy ? $t("story.rebuilding") : $t("story.rebuildShort") }}
              </button>
            </div>
          </div>
          <p v-if="message" class="muted">{{ message }}</p>

          <div class="story-scroll" :class="{ 'story-scroll-fill': tab === 'relations' }">
            <div v-if="tab === 'map'" class="block">
              <p class="muted map-hint">{{ $t("story.mapHint") }}</p>
              <MindMapBoard :tree="mindTree" :height="480" @select="onMapSelect" />
              <p v-if="selectedNode" class="muted select-hint">
                {{ $t("story.selected", { label: selectedNode.label }) }}
                <span v-if="selectedNode.meta"> — {{ selectedNode.meta }}</span>
              </p>
            </div>

            <div v-if="tab === 'plot'" class="block">
              <div class="row-actions">
                <button type="button" class="app-btn" @click="addArc">{{ $t("story.addArc") }}</button>
                <button type="button" class="app-btn" @click="addPromise">{{ $t("story.addPromise") }}</button>
                <button type="button" class="app-btn app-btn-primary" @click="onSavePlot">{{ $t("story.savePlot") }}</button>
              </div>
              <h3 class="sub">{{ $t("story.arcs") }}</h3>
              <div v-for="(a, i) in plot.arcs" :key="a.id" class="card">
                <div class="grid2">
                  <input v-model="a.title" :placeholder="$t('story.phTitle')" />
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
                <input v-model="a.goal" :placeholder="$t('story.phGoal')" />
                <textarea v-model="a.progress_note" rows="2" :placeholder="$t('story.phProgress')" />
                <button type="button" class="app-btn" @click="removeAt(plot.arcs, i)">{{ $t("common.delete") }}</button>
              </div>
              <h3 class="sub">{{ $t("story.promises") }}</h3>
              <div v-for="(p, i) in plot.promises" :key="p.id" class="card">
                <textarea v-model="p.text" rows="2" />
                <select v-model="p.status">
                  <option value="open">open</option>
                  <option value="paid">paid</option>
                  <option value="broken">broken</option>
                </select>
                <button type="button" class="app-btn" @click="removeAt(plot.promises, i)">{{ $t("common.delete") }}</button>
              </div>
            </div>

            <div v-if="tab === 'focus' || tab === 'beats'" class="block">
              <p class="muted">{{ $t("story.currentChapter", { title: currentChapter?.title || $t("story.noChapter") }) }}</p>
              <div class="field">
                <label class="field-label">POV lore id</label>
                <select v-model="focusDraft.pov_lore_id">
                  <option value="">{{ $t("common.none") }}</option>
                  <option v-for="l in loreItems" :key="l.id" :value="l.id">{{ l.title }} ({{ l.id.slice(0, 8) }})</option>
                </select>
              </div>
              <div class="field">
                <label class="field-label">{{ $t("story.focusArcIds") }}</label>
                <input v-model="focusDraft.focus_arc_ids" type="text" />
                <p class="hint muted">{{ $t("story.optional", { text: (plot.arcs || []).map((a) => a.id.slice(0, 8) + ':' + a.title).join(' · ') }) }}</p>
              </div>
              <div class="field">
                <label class="field-label">{{ $t("story.mustDo") }}</label>
                <textarea v-model="focusDraft.must_do" rows="2" />
              </div>
              <div class="field">
                <label class="field-label">{{ $t("story.mustNot") }}</label>
                <textarea v-model="focusDraft.must_not" rows="2" />
              </div>
              <div class="field">
                <label class="field-label">{{ $t("story.readerKnows") }}</label>
                <textarea v-model="focusDraft.reader_knows" rows="2" />
              </div>
              <div class="field">
                <label class="field-label">{{ $t("story.characterKnows") }}</label>
                <textarea v-model="focusDraft.character_knows" rows="2" />
              </div>
              <div class="field">
                <label class="field-label">{{ $t("story.beatsFormat") }}</label>
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
                  <button type="button" class="app-btn" @click="onResetBeatProgress">{{ $t("story.resetProgress") }}</button>
                  <button type="button" class="app-btn" @click="onSkipCurrentBeat">{{ $t("story.skipBeat") }}</button>
                </div>
              </div>
              <button type="button" class="app-btn app-btn-primary" @click="onSaveFocus">{{ $t("story.saveFocus") }}</button>
            </div>

            <div v-if="tab === 'board'" class="block">
              <p class="muted">{{ $t("story.boardHint", { title: currentChapter?.title || $t("story.noChapter") }) }}</p>
              <div class="field">
                <label class="field-label">{{ $t("story.styleLock") }}</label>
                <input v-model="storyboard.style_prefix" type="text" :placeholder="$t('story.stylePh')" />
              </div>
              <div class="field">
                <label class="field-label">{{ $t("story.defaultNeg") }}</label>
                <input v-model="storyboard.negative" type="text" placeholder="text, watermark, extra fingers" />
              </div>
              <div class="row-actions">
                <button type="button" class="app-btn" :disabled="boardBusy" @click="addShot">{{ $t("story.addShot") }}</button>
                <button type="button" class="app-btn" :disabled="boardBusy || !appState.chapterId" @click="onGenerateStoryboard">
                  {{ boardBusy ? $t("common.processing") : $t("story.genFromBeats") }}
                </button>
                <button type="button" class="app-btn app-btn-primary" @click="onSaveStoryboard">{{ $t("story.saveBoard") }}</button>
              </div>
              <p v-if="imageGenState.message" class="muted">{{ imageGenState.message }}</p>
              <div v-for="(s, i) in chapterShots" :key="s.id" class="card board-shot">
                <div class="grid2">
                  <input v-model.number="s.seq" type="number" min="1" :title="$t('story.seq')" />
                  <select v-model="s.beat_id">
                    <option value="">{{ $t("story.noBeat") }}</option>
                    <option v-for="b in (currentChapter && currentChapter.beats) || []" :key="b.id" :value="b.id">
                      {{ b.title || b.purpose || b.id.slice(0, 8) }}
                    </option>
                  </select>
                </div>
                <input v-model="s.location" :placeholder="$t('story.phLocation')" />
                <textarea v-model="s.visual" rows="2" :placeholder="$t('story.phVisual')" />
                <textarea v-model="s.dialogue" rows="2" :placeholder="$t('story.phDialogue')" />
                <input v-model="s.mood" :placeholder="$t('story.phMood')" />
                <p class="hint muted">{{ $t("story.people") }}</p>
                <div class="tag-row">
                  <button
                    v-for="l in loreItems.filter((x) => x.kind === 'character')"
                    :key="l.id"
                    type="button"
                    class="chip"
                    :class="(s.character_lore_ids || []).includes(l.id) ? 'chip-active' : ''"
                    @click="toggleShotChar(s, l.id)"
                  >
                    {{ l.title }}
                  </button>
                </div>
                <p v-if="(s.character_lore_ids || []).length" class="hint muted">
                  {{ $t("story.selectedChars", { names: (s.character_lore_ids || []).map(loreTitle).join($t("common.listSep")) }) }}
                </p>
                <img
                  v-if="s.image && s.image.rel && shotThumbs[s.image.rel]"
                  class="shot-thumb"
                  :src="shotThumbs[s.image.rel]"
                  alt=""
                />
                <div class="row-actions">
                  <button type="button" class="app-btn" :disabled="boardBusy" @click="moveShot(i, -1)">{{ $t("common.up") }}</button>
                  <button type="button" class="app-btn" :disabled="boardBusy" @click="moveShot(i, 1)">{{ $t("common.down") }}</button>
                  <button type="button" class="app-btn" :disabled="boardBusy" @click="onGenerateShotImage(s)">{{ $t("story.genShotImage") }}</button>
                  <button type="button" class="app-btn" :disabled="boardBusy" @click="onInsertShotIntoChapter(s)">{{ $t("story.insertChapter") }}</button>
                  <button type="button" class="app-btn" @click="removeShot(i)">{{ $t("common.delete") }}</button>
                </div>
              </div>
            </div>

            <div v-if="tab === 'timeline'" class="block">
              <div class="field">
                <label class="field-label">{{ $t("story.calendarNote") }}</label>
                <input v-model="timeline.calendar_note" type="text" />
              </div>
              <div class="row-actions">
                <button type="button" class="app-btn" @click="addEvent">{{ $t("story.addEvent") }}</button>
                <button type="button" class="app-btn app-btn-primary" @click="onSaveTimeline">{{ $t("story.saveTimeline") }}</button>
              </div>
              <div class="tl-rail">
                <div v-for="(ev, i) in timelineSorted" :key="ev.id" class="tl-item">
                  <div class="tl-dot" />
                  <div class="tl-card card">
                    <div class="grid2">
                      <input v-model="ev.story_time" :placeholder="$t('story.phStoryDay')" />
                      <input v-model="ev.title" :placeholder="$t('story.phTitle')" />
                    </div>
                    <textarea v-model="ev.summary" rows="2" :placeholder="$t('story.phSummary')" />
                    <input v-model="ev.location" :placeholder="$t('story.phLocation')" />
                    <button
                      type="button"
                      class="app-btn"
                      @click="removeAt(timeline.events, timeline.events.findIndex((x) => x.id === ev.id))"
                    >
                      {{ $t("common.delete") }}
                    </button>
                  </div>
                </div>
              </div>
              <div v-if="!timelineSorted.length" class="muted">{{ $t("story.timelineEmpty") }}</div>
            </div>

            <div v-if="tab === 'canon'" class="block">
              <div class="row-actions">
                <button type="button" class="app-btn" @click="addFact">{{ $t("story.addFact") }}</button>
                <button type="button" class="app-btn app-btn-primary" @click="onSaveCanon">{{ $t("story.saveCanon") }}</button>
              </div>
              <div v-for="(f, i) in canon.facts" :key="f.id" class="card">
                <textarea v-model="f.text" rows="2" />
                <CapsuleSwitch v-model="f.locked" :label="$t('story.lockCanon')" />
                <button type="button" class="app-btn" @click="removeAt(canon.facts, i)">{{ $t("common.delete") }}</button>
              </div>
            </div>

            <div v-if="tab === 'relations'" class="block relations-block">
              <RelationForceGraph
                fill
                :edges="relations.edges"
                :lore-items="loreItems"
                @select="onGraphSelect"
              >
                <template #toolbar>
                  <button type="button" class="app-btn" @click="addEdge">{{ $t("story.addEdge") }}</button>
                  <button type="button" class="app-btn app-btn-primary" @click="onSaveRelations">{{ $t("story.saveRelations") }}</button>
                </template>
              </RelationForceGraph>
              <p v-if="selectedNode" class="muted select-hint">
                {{ $t("story.selected", { label: selectedNode.label }) }}
                <span v-if="selectedNode.meta"> — {{ selectedNode.meta }}</span>
              </p>
              <div class="edge-list">
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
                  <button type="button" class="app-btn" @click="removeAt(relations.edges, i)">{{ $t("common.delete") }}</button>
                </div>
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
.story-panel-embed {
  padding: 0;
  box-shadow: none;
  background: transparent;
  border-radius: 0;
}
.story-head {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}
.story-head-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.rebuild-progress {
  flex-shrink: 0;
  margin: 0 0 4px;
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
.story-scroll-fill {
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.story-scroll-fill .relations-block {
  flex: 1;
}
.relations-block {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  gap: 6px;
}
.edge-list {
  flex-shrink: 0;
  max-height: 140px;
  overflow-y: auto;
}
.story-cast {
  min-height: 0;
}
.subtabs {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 6px 8px;
  margin: 0 0 8px;
  flex-shrink: 0;
}
.subtabs-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.subtabs-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-left: auto;
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
.error {
  color: var(--error);
}
.timeline-card {
  border-left: 3px solid var(--accent-hover);
}
.shot-thumb {
  display: block;
  max-width: 220px;
  max-height: 160px;
  object-fit: cover;
  border-radius: 8px;
  margin: 8px 0;
}
.tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 4px 0 8px;
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
