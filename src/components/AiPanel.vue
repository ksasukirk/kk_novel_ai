<!--
  AI 面板：侧栏 dock / 阅读区底部 float 双形态（无 Teleport，由 EditorView 决定挂载位置）
  代码路径: kk_novel_ai/src/components/AiPanel.vue
-->
<script setup>
import { computed, nextTick, ref, toRefs, watch } from "vue";
import { appState } from "../stores/appState.js";
import { aiPanelForm, instrCaret, activeStepId, createInstructionStep } from "../stores/aiPanelState.js";
import { runWriting } from "../services/llmClient.js";
import { withBranchContext } from "../services/draftAccept.js";
import { saveChapter, updateChapterMeta } from "../services/projectClient.js";
import { undoLastAi } from "../services/aiUndo.js";
import { applyStoryPatch } from "../services/storyClient.js";
import { lineDiff } from "../utils/lineDiff.js";
import { isBackgroundAnalysisTask } from "../utils/writingTasks.js";
import { clearDraftPreview, rejectDraft } from "../services/draftAccept.js";
import { canStartMoreJobs } from "../stores/genJobs.js";
import {
  MAX_INSTRUCTION_STEPS,
  queueStatusLine,
  runInstructionQueue,
  sectionQueueState,
} from "../services/sectionQueue.js";
import {
  cancelOutlineQueue,
  outlineQueueStatusLine,
  outlineQueueState,
} from "../services/outlineQueue.js";
import {
  applyChapterPlan,
  findFirstPendingOutlineChapter,
  resolveBookOutlineSeed,
  runContinueOutline,
  runFullOutlinePipeline,
  runSplitAndApply,
  saveBookOutline,
} from "../services/bookOutlineQueue.js";
import { appConfirm } from "../services/confirmDialog.js";
import GenProgressBar from "./GenProgressBar.vue";
import CapsuleSwitch from "./CapsuleSwitch.vue";

const props = defineProps({
  /** dock=右侧侧栏；float=阅读区底部长条 */
  layout: {
    type: String,
    default: "float",
    validator: (v) => v === "float" || v === "dock",
  },
});

const emit = defineEmits(["toggle-layout"]);

const {
  task,
  instruction,
  selection,
  showDiff,
  sectionQueue,
  instructionQueue,
  instructionSteps,
  bookOutline,
  chapterPlan,
  syncMsg,
  error,
  floatExpanded,
} = toRefs(aiPanelForm);

const instructionEl = ref(null);
const stepEls = ref({});

const isFloat = computed(() => props.layout === "float");

/** 浮条输入：按纲生成绑创作提示，其它任务绑指令 */
const floatPrompt = computed({
  get() {
    return task.value === "outline_run" ? bookOutline.value : instruction.value;
  },
  set(v) {
    if (task.value === "outline_run") bookOutline.value = v;
    else instruction.value = v;
  },
});

const filledStepCount = computed(
  () => instructionSteps.value.filter((s) => String(s.text || "").trim()).length
);

/** 本篇已有角色名（来自 characterList） */
const characterTags = computed(() => {
  const list = appState.characterList || [];
  const out = [];
  const seen = new Set();
  for (const e of list) {
    if (!e || (e.kind && e.kind !== "character")) continue;
    const title = String(e.title || "").trim();
    if (!title || seen.has(title)) continue;
    seen.add(title);
    out.push({ id: e.id || title, title });
  }
  return out;
});

function rememberInstrCaret() {
  if (task.value === "outline_run") return;
  const el = instructionEl.value;
  if (!el) return;
  instrCaret.value = {
    start: el.selectionStart ?? instruction.value.length,
    end: el.selectionEnd ?? instruction.value.length,
  };
}

function rememberStepCaret(step, ev) {
  if (!step?.id) return;
  activeStepId.value = step.id;
  const el = ev?.target;
  if (!el || el.selectionStart == null) return;
  instrCaret.value = {
    start: el.selectionStart,
    end: el.selectionEnd ?? el.selectionStart,
  };
}

function setStepEl(id, el) {
  if (!id) return;
  if (el) stepEls.value[id] = el;
  else delete stepEls.value[id];
}

/** 点击角色标签：插入当前指令框（或指令队列焦点步） */
function insertCharacterName(name) {
  const insert = String(name || "").trim();
  if (!insert) return;

  if (instructionQueue.value && task.value === "continue") {
    let step =
      instructionSteps.value.find((s) => s.id === activeStepId.value) ||
      instructionSteps.value[0];
    if (!step) {
      step = createInstructionStep("");
      instructionSteps.value.push(step);
    }
    const text = step.text || "";
    const len = text.length;
    const start =
      instrCaret.value.start == null ? len : Math.min(instrCaret.value.start, len);
    const end =
      instrCaret.value.end == null ? len : Math.min(instrCaret.value.end, len);
    const before = text.slice(0, start);
    const after = text.slice(end);
    const spaceBefore = before.length > 0 && !/\s$/.test(before) ? " " : "";
    const spaceAfter = after.length > 0 && !/^\s/.test(after) ? " " : "";
    step.text = before + spaceBefore + insert + spaceAfter + after;
    const caret = before.length + spaceBefore.length + insert.length;
    instrCaret.value = { start: caret, end: caret };
    activeStepId.value = step.id;
    nextTick(() => {
      const el = stepEls.value[step.id];
      if (!el) return;
      el.focus();
      el.setSelectionRange(caret, caret);
    });
    return;
  }

  const text = instruction.value || "";
  const len = text.length;
  const start =
    instrCaret.value.start == null ? len : Math.min(instrCaret.value.start, len);
  const end =
    instrCaret.value.end == null ? len : Math.min(instrCaret.value.end, len);
  const before = text.slice(0, start);
  const after = text.slice(end);
  const spaceBefore = before.length > 0 && !/\s$/.test(before) ? " " : "";
  const spaceAfter = after.length > 0 && !/^\s/.test(after) ? " " : "";
  const piece = `${spaceBefore}${insert}${spaceAfter}`;
  instruction.value = before + piece + after;
  const caret = before.length + spaceBefore.length + insert.length;
  instrCaret.value = { start: caret, end: caret };
  nextTick(() => {
    const el = instructionEl.value;
    if (!el) return;
    el.focus();
    el.setSelectionRange(caret, caret);
  });
}

function addInstructionStep() {
  if (instructionSteps.value.length >= MAX_INSTRUCTION_STEPS) return;
  const step = createInstructionStep("");
  instructionSteps.value.push(step);
  activeStepId.value = step.id;
  nextTick(() => {
    const el = stepEls.value[step.id];
    if (el) el.focus();
  });
}

function removeInstructionStep(id) {
  if (instructionSteps.value.length <= 1) {
    instructionSteps.value[0].text = "";
    return;
  }
  const idx = instructionSteps.value.findIndex((s) => s.id === id);
  if (idx < 0) return;
  instructionSteps.value.splice(idx, 1);
  if (activeStepId.value === id) {
    activeStepId.value = instructionSteps.value[Math.max(0, idx - 1)]?.id || "";
  }
}

function moveInstructionStep(id, dir) {
  const idx = instructionSteps.value.findIndex((s) => s.id === id);
  if (idx < 0) return;
  const j = idx + dir;
  if (j < 0 || j >= instructionSteps.value.length) return;
  const tmp = instructionSteps.value[idx];
  instructionSteps.value[idx] = instructionSteps.value[j];
  instructionSteps.value[j] = tmp;
}

function onInstructionQueueToggle(on) {
  if (on) {
    sectionQueue.value = false;
    floatExpanded.value = true;
    if (!instructionSteps.value.length) {
      instructionSteps.value.push(createInstructionStep(instruction.value || ""));
    } else if (
      instructionSteps.value.length === 1 &&
      !String(instructionSteps.value[0].text || "").trim() &&
      String(instruction.value || "").trim()
    ) {
      instructionSteps.value[0].text = instruction.value;
    }
    activeStepId.value = instructionSteps.value[0]?.id || "";
  }
}

watch(instructionQueue, (on) => {
  onInstructionQueueToggle(!!on);
});

watch(task, (id) => {
  if (id === "polish") floatExpanded.value = true;
});

const tasks = [
  { id: "outline_run", label: "使用大纲生成章节描述" },
  { id: "continue", label: "续写" },
  { id: "polish", label: "润色" },
  { id: "outline", label: "章纲" },
  { id: "consistency", label: "一致性" },
  { id: "chapter_summary", label: "章摘要" },
  { id: "story_sync", label: "同步总谱" },
];

const EDITOR_DRAFT_TASKS = new Set(["continue", "polish"]);

const useEditorDraft = computed(() => EDITOR_DRAFT_TASKS.has(task.value));

const currentTaskLabel = computed(() => {
  const t = tasks.find((x) => x.id === task.value);
  return (t && t.label) || "使用大纲生成章节描述";
});

const selectedPlanCount = computed(
  () => (chapterPlan.value || []).filter((c) => c && c.selected !== false).length
);

/** 上方大纲或底部指令，任一有内容即可生成章节队列 */
const outlineSeedReady = computed(() => !!resolveBookOutlineSeed({
  bookOutline: bookOutline.value,
  instruction: instruction.value,
}));

const hasPendingOutlineChapter = computed(() => !!findFirstPendingOutlineChapter());

const previewMeta = computed(() => {
  const parts = [];
  const q = queueStatusLine();
  if (q) parts.push(q);
  if (appState.lastModelUsed) parts.push(appState.lastModelUsed);
  if (appState.generating) parts.push("生成中…");
  if (appState.lastTruncated) parts.push("疑似复读（未截断）");
  if (appState.lastIncomplete) parts.push("疑似半截");
  const u = appState.lastUsage;
  if (u && (u.total_tokens || u.prompt_tokens || u.completion_tokens)) {
    const src = u.source === "api" ? "api" : "估";
    parts.push(
      `tokens ${u.total_tokens || (u.prompt_tokens || 0) + (u.completion_tokens || 0)} (${src})`
    );
  }
  if (appState.lastCostCny != null && appState.lastCostCny > 0) {
    parts.push(`¥${Number(appState.lastCostCny).toFixed(4)}`);
  }
  return parts.join(" · ");
});

const currentChapter = computed(() => {
  const list = (appState.project && appState.project.chapters) || [];
  return list.find((c) => c.id === appState.chapterId) || null;
});

const focusHint = computed(() => {
  const ch = currentChapter.value;
  if (!ch) return "未选章节";
  const arcs = (ch.focus_arc_ids || []).join(", ") || "未绑弧";
  const must = ch.must_do || "（无必达）";
  return `POV:${ch.pov_lore_id || "无"} · 弧:${arcs} · 必达:${must}`;
});

const syncPatch = computed(() => {
  if (task.value !== "story_sync" || !appState.previewText) return null;
  const raw = appState.previewText.trim();
  const fence = raw.match(/```(?:json)?\s*([\s\S]*?)```/);
  const text = fence ? fence[1].trim() : raw;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
});

const diffBase = computed(() => {
  if (task.value === "polish" && selection.value) return selection.value;
  return "";
});

const diffRows = computed(() => {
  if (!showDiff.value || !appState.previewText || !diffBase.value) return [];
  return lineDiff(diffBase.value, appState.previewText);
});

const showPanelPreview = computed(
  () => !useEditorDraft.value && task.value !== "outline_run"
);

const canSend = computed(
  () =>
    canStartMoreJobs(1) &&
    !sectionQueueState.running &&
    !outlineQueueState.running
);
const canCancel = computed(
  () =>
    !!appState.generating ||
    !!sectionQueueState.running ||
    !!outlineQueueState.running
);

/** 浮条是否处于生成中（应尽量收成单行） */
const floatBusy = computed(
  () => !!appState.generating || !!outlineQueueState.running || !!sectionQueueState.running
);

/**
 * 浮条上方扩展区可见性：
 * 生成中强制收起；任务芯片与下拉重复故不再渲染芯片行。
 */
const floatExtraVisible = computed(() => {
  if (showPanelPreview.value) return true;
  if (task.value === "continue" && instructionQueue.value) return true;
  if (task.value === "polish" && (floatExpanded.value || !!selection.value)) return true;
  if (floatBusy.value) return false;
  return !!floatExpanded.value;
});

const floatFootStatus = computed(() => {
  const oq = outlineQueueStatusLine();
  if (oq) return oq;
  const sq = queueStatusLine();
  if (sq) return sq;
  if (previewMeta.value) return previewMeta.value;
  return focusHint.value;
});

watch(floatBusy, (busy) => {
  if (busy && isFloat.value) floatExpanded.value = false;
});

async function onRun() {
  error.value = "";
  syncMsg.value = "";
  if (!appState.projectRoot) {
    error.value = "请先打开作品";
    return;
  }
  try {
    if (task.value === "outline_run") {
      // 主按钮：拆章写入目录后立刻按弹窗里的章开写正文
      // 目录可为空（会自动建占位章再拆）
      await onGenerateChapterQueue();
      return;
    }
    if (!appState.chapterId) {
      error.value = "请先选择章节，或用「按纲生成」写入目录";
      return;
    }
    if (task.value === "continue") {
      const blocked = await guardEmptyChapterContinue();
      if (blocked) return;
    }
    if (task.value === "continue" && instructionQueue.value) {
      await runInstructionQueue({
        steps: instructionSteps.value,
        selection: selection.value,
      });
      return;
    }
    // 已取消「自动分节 / 小节」：续写与按纲都按整章一块
    sectionQueue.value = false;
    if (appState.dirty) await saveChapter();
    appState.draftPlacement = isBackgroundAnalysisTask(task.value)
      ? "panel"
      : useEditorDraft.value
        ? "editor"
        : "panel";
    appState.draftTask = task.value;
    appState.draftSelection = selection.value || "";
    appState.draftInstruction = instruction.value || "";
    appState.draftPersistInstruction = "";
    appState.draftRewriteBlockKey = "";
    appState.draftAnchorBlockKey = "";
    appState.draftBranchMode = "";
    appState.draftBranchNodeId = "";
    appState.draftForkFromVariantId = "";
    const base = {
      project_root: appState.projectRoot,
      chapter_id: appState.chapterId,
      task: task.value,
      instruction: instruction.value,
      selection: selection.value,
    };
    const req =
      task.value === "continue" || task.value === "outline"
        ? withBranchContext(base, "continue", "")
        : base;
    await runWriting(req);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

/** 本章无章纲且有全书大纲时，裸续写软拦截 */
async function guardEmptyChapterContinue() {
  const project = appState.project;
  if (!project) return false;
  const ch = (project.chapters || []).find((c) => c.id === appState.chapterId);
  const hasSummary =
    ch &&
    (String(ch.summary || "").trim() ||
      (Array.isArray(ch.beats) && ch.beats.length > 0));
  if (hasSummary) return false;
  const bookOutlineText = String(
    project.book_outline || bookOutline.value || ""
  ).trim();
  if (!bookOutlineText) return false;
  const ok = await appConfirm(
    "本章还没有章纲。建议先「拆成章节」或写入本章纲，再按纲写；若仍要直接续写，点继续。",
    {
      title: "本章无章纲",
      confirmText: "仍要续写",
      cancelText: "去按纲",
    }
  );
  if (!ok) {
    task.value = "outline_run";
    floatExpanded.value = true;
    error.value = "本章无章纲，请先拆章或写入本章纲";
    return true;
  }
  return false;
}

async function onGenerateChapterQueue() {
  error.value = "";
  syncMsg.value = "";
  try {
    const seed = resolveBookOutlineSeed({
      bookOutline: bookOutline.value,
      instruction: instruction.value,
    });
    if (!seed) {
      error.value = "请先填写创作提示：写在上方「创作提示」框，或底部指令栏";
      floatExpanded.value = true;
      return;
    }
    const r = await runSplitAndApply({
      bookOutline: seed,
      instruction: instruction.value,
    });
    const n = (r.updatedIds || []).length + (r.createdIds || []).length;
    syncMsg.value = r.writingCancelled
      ? `已写入目录 ${n} 章，写作已取消`
      : `已写入 ${n} 章并按纲写完`;
  } catch (e) {
    const msg = String(e.message || e);
    if (msg.includes("取消")) {
      syncMsg.value = msg;
      return;
    }
    error.value = msg;
    if (isFloat.value) floatExpanded.value = true;
  }
}

async function onSaveBookOutline() {
  error.value = "";
  try {
    const seed = resolveBookOutlineSeed({
      bookOutline: bookOutline.value,
      instruction: instruction.value,
    });
    await saveBookOutline(seed || bookOutline.value);
    syncMsg.value = "创作提示已保存";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onSplitChapters() {
  await onGenerateChapterQueue();
}

async function onContinueSplit() {
  error.value = "";
  syncMsg.value = "";
  try {
    const r = await runContinueOutline({
      instruction: instruction.value,
    });
    syncMsg.value = r.writingCancelled
      ? `已续拆 ${(r.createdIds || []).length} 章，写作已取消`
      : `已续拆 ${(r.createdIds || []).length} 章并按纲写完`;
  } catch (e) {
    const msg = String(e.message || e);
    if (msg.includes("取消")) {
      syncMsg.value = msg;
      return;
    }
    error.value = msg;
    if (isFloat.value) floatExpanded.value = true;
  }
}

async function onApplyChapterPlan() {
  error.value = "";
  syncMsg.value = "";
  try {
    const r = await applyChapterPlan(chapterPlan.value, {
      instruction: instruction.value,
    });
    const n = r.updatedIds.length + r.createdIds.length;
    syncMsg.value = r.writingCancelled
      ? `已写入目录 ${n} 章，写作已取消`
      : `已写入 ${n} 章并按纲写完`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onWriteAllByOutline() {
  error.value = "";
  syncMsg.value = "";
  try {
    await runFullOutlinePipeline({
      instruction: instruction.value,
      applyPlanFirst: selectedPlanCount.value > 0,
    });
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onWriteOutlineToChapter() {
  error.value = "";
  const text = String(appState.previewText || "").trim();
  if (!text) {
    error.value = "没有可写入的章纲预览";
    return;
  }
  if (!appState.chapterId) {
    error.value = "请先选择章节";
    return;
  }
  try {
    await updateChapterMeta(appState.chapterId, {
      summary: text,
      status: "pending",
    });
    syncMsg.value = "已写入本章纲";
    appState.previewText = "";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

/** 次要：把预览塞进全书大纲（慎用，章纲格式勿当全书大纲） */
async function onWriteOutlineToBook() {
  error.value = "";
  const text = String(appState.previewText || "").trim();
  if (!text) {
    error.value = "没有可写入的章纲预览";
    return;
  }
  try {
    bookOutline.value = text;
    await saveBookOutline(text);
    syncMsg.value = "已写入全书大纲（请确认不是单场章纲）";
    task.value = "outline_run";
    floatExpanded.value = true;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function onFloatKeydown(e) {
  if (e.key !== "Enter") return;
  if (e.shiftKey) return;
  e.preventDefault();
  if (canSend.value) void onRun();
}

function clearInstruction() {
  instruction.value = "";
  instrCaret.value = { start: 0, end: 0 };
  if (instructionQueue.value) {
    instructionSteps.value = [createInstructionStep("")];
    activeStepId.value = instructionSteps.value[0].id;
  }
}

function runButtonLabel() {
  if (outlineQueueState.running) {
    if (outlineQueueState.phase === "splitting_chapters") {
      return isFloat.value ? "拆章中" : "生成章节队列中…";
    }
    return isFloat.value ? "按纲中" : "按纲写正文中…";
  }
  if (sectionQueueState.running) return isFloat.value ? "队列中" : "队列中…";
  if (appState.generating) return isFloat.value ? "再发" : "再发一路";
  if (task.value === "outline_run") {
    return isFloat.value ? "生成队列" : "生成章节队列";
  }
  if (task.value === "continue" && instructionQueue.value) {
    return isFloat.value ? "连跑" : `按队列生成（${filledStepCount.value}）`;
  }
  return isFloat.value ? "发送" : "开始生成";
}
function clearSelection() {
  selection.value = "";
}

async function onCancelAll() {
  if (outlineQueueState.running) cancelOutlineQueue();
  await rejectDraft();
}

async function onDiscard() {
  if (outlineQueueState.running) {
    cancelOutlineQueue();
  }
  if (appState.draftPlacement === "editor") {
    await rejectDraft();
  } else {
    clearDraftPreview();
    syncMsg.value = "";
  }
}

function onUndoAi() {
  undoLastAi();
}

async function onApplySync() {
  error.value = "";
  syncMsg.value = "";
  if (!syncPatch.value) {
    error.value = "预览不是合法 JSON patch";
    return;
  }
  try {
    const r = await applyStoryPatch(syncPatch.value);
    syncMsg.value = `已应用：${(r.updated || []).join(", ")}`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function onToggleLayout() {
  emit("toggle-layout");
}
</script>

<template>
  <div class="ai-panel-root" :data-layout="layout">
    <div v-if="isFloat" class="ai-float" :class="{ 'is-busy': floatBusy }" role="region" aria-label="AI 指令">
      <div v-if="floatExtraVisible" class="ai-float-extra">
        <!-- 浮条用下拉选任务，不再重复渲染芯片行 -->
        <div v-if="task === 'polish'" class="ai-float-selection">
          <div class="field-label-row">
            <span class="field-label">选区（润色）</span>
            <button
              type="button"
              class="clear-btn"
              title="清空选区"
              :disabled="!selection"
              @click="clearSelection"
            >
              清
            </button>
          </div>
          <textarea
            v-model="selection"
            rows="2"
            placeholder="粘贴要润色的段落"
          />
        </div>
        <div
          v-if="characterTags.length && floatExpanded && !floatBusy"
          class="char-tag-row float-char-tags"
          aria-label="本篇角色"
        >
          <button
            v-for="c in characterTags"
            :key="c.id"
            type="button"
            class="char-tag"
            :title="`填入「${c.title}」`"
            @click="insertCharacterName(c.title)"
          >
            {{ c.title }}
          </button>
        </div>
        <div v-if="task === 'continue'" class="ai-float-queue">
          <CapsuleSwitch
            v-model="instructionQueue"
            label="指令队列"
            :disabled="sectionQueueState.running"
          />
        </div>
        <div
          v-if="task === 'continue' && instructionQueue"
          class="instr-step-list float-steps"
        >
          <div
            v-for="(step, si) in instructionSteps"
            :key="step.id"
            class="instr-step"
            :class="{
              current:
                sectionQueueState.mode === 'manual' &&
                sectionQueueState.phase === 'writing' &&
                si === sectionQueueState.index - 1,
              done:
                sectionQueueState.mode === 'manual' &&
                (sectionQueueState.phase === 'done' ||
                  (sectionQueueState.phase === 'writing' && si < sectionQueueState.index - 1)),
            }"
          >
            <div class="instr-step-head">
              <span class="instr-step-idx">{{ si + 1 }}</span>
              <button
                type="button"
                class="step-icon-btn"
                title="上移"
                :disabled="si === 0 || sectionQueueState.running"
                @click="moveInstructionStep(step.id, -1)"
              >
                ↑
              </button>
              <button
                type="button"
                class="step-icon-btn"
                title="下移"
                :disabled="si >= instructionSteps.length - 1 || sectionQueueState.running"
                @click="moveInstructionStep(step.id, 1)"
              >
                ↓
              </button>
              <button
                type="button"
                class="step-icon-btn danger"
                title="删除"
                :disabled="sectionQueueState.running"
                @click="removeInstructionStep(step.id)"
              >
                ×
              </button>
            </div>
            <textarea
              :ref="(el) => setStepEl(step.id, el)"
              v-model="step.text"
              rows="2"
              :placeholder="`第 ${si + 1} 步指令（只写这一拍）`"
              :disabled="sectionQueueState.running"
              @focus="activeStepId = step.id"
              @select="rememberStepCaret(step, $event)"
              @click="rememberStepCaret(step, $event)"
              @keyup="rememberStepCaret(step, $event)"
            />
          </div>
          <button
            type="button"
            class="app-btn step-add-btn"
            :disabled="
              sectionQueueState.running || instructionSteps.length >= MAX_INSTRUCTION_STEPS
            "
            @click="addInstructionStep"
          >
            加一步（{{ instructionSteps.length }}/{{ MAX_INSTRUCTION_STEPS }}）
          </button>
        </div>
        <template v-if="showPanelPreview">
          <div class="field preview-field">
            <label class="field-label">
              预览
              <CapsuleSwitch v-model="showDiff" label="Diff" class="diff-toggle" />
            </label>
            <p v-if="previewMeta" class="muted preview-meta">{{ previewMeta }}</p>
            <div v-if="diffRows.length" class="diff-box">
              <div
                v-for="(row, idx) in diffRows"
                :key="idx"
                class="diff-line"
                :class="row.type"
              >
                <span class="mark">{{
                  row.type === "add" ? "+" : row.type === "remove" ? "-" : " "
                }}</span>
                {{ row.text }}
              </div>
            </div>
            <textarea
              v-else
              class="preview"
              :value="appState.previewText"
              readonly
              rows="6"
            />
          </div>
          <div class="actions">
            <button
              v-if="task === 'story_sync'"
              type="button"
              class="app-btn app-btn-primary"
              :disabled="!syncPatch"
              @click="onApplySync"
            >
              确认应用总谱 patch
            </button>
            <button
              v-if="task === 'outline' && appState.previewText"
              type="button"
              class="app-btn app-btn-primary"
              @click="onWriteOutlineToChapter"
            >
              写入本章纲
            </button>
            <button
              v-if="task === 'outline' && appState.previewText"
              type="button"
              class="app-btn"
              title="仅当预览确为全书大纲时使用"
              @click="onWriteOutlineToBook"
            >
              写入全书大纲
            </button>
            <button type="button" class="app-btn" @click="onDiscard">丢弃</button>
          </div>
        </template>
        <p v-if="syncMsg" class="muted float-sync-msg">{{ syncMsg }}</p>
      </div>

      <div class="ai-float-bar">
        <div class="ai-float-task-wrap">
          <select v-model="task" class="ai-float-task" :title="focusHint">
            <option v-for="t in tasks" :key="t.id" :value="t.id">{{ t.label }}</option>
          </select>
        </div>
        <textarea
          v-if="!(task === 'continue' && instructionQueue)"
          ref="instructionEl"
          v-model="floatPrompt"
          class="ai-float-input"
          rows="1"
          :placeholder="
            task === 'outline_run'
              ? '创作提示 · Enter 生成章节队列'
              : `指令（${currentTaskLabel}）· Enter 发送`
          "
          @select="rememberInstrCaret"
          @click="rememberInstrCaret"
          @keyup="rememberInstrCaret"
          @blur="rememberInstrCaret"
          @keydown="onFloatKeydown"
        />
        <div
          v-else
          class="ai-float-input queue-hint muted"
        >
          已开指令队列 · 上方编辑各步 · {{ filledStepCount }} 条待跑
        </div>
        <div class="ai-float-actions">
          <button
            type="button"
            class="ai-float-icon-btn"
            :title="floatExpanded ? '收起选项' : '展开角色/队列等'"
            :disabled="floatBusy"
            @click="floatExpanded = !floatExpanded"
          >
            {{ floatExpanded && !floatBusy ? "收起" : "更多" }}
          </button>
          <button
            type="button"
            class="app-btn app-btn-primary ai-float-send"
            :disabled="!canSend || (instructionQueue && task === 'continue' && filledStepCount < 1)"
            @click="onRun"
          >
            {{ runButtonLabel() }}
          </button>
          <button
            type="button"
            class="app-btn"
            :disabled="!canCancel"
            title="取消全部进行中的生成"
            @click="onCancelAll"
          >
            取消
          </button>
        </div>
      </div>

      <div class="ai-float-foot">
        <span class="muted tip" :title="floatFootStatus">{{ floatFootStatus }}</span>
        <span v-if="syncMsg && !floatExtraVisible" class="muted tip float-sync-inline" :title="syncMsg">{{
          syncMsg
        }}</span>
        <template v-if="task === 'outline_run'">
          <button
            type="button"
            class="link-btn"
            :disabled="!canSend"
            title="续拆后续章节到目录"
            @click="onContinueSplit"
          >
            续拆
          </button>
          <button
            type="button"
            class="link-btn"
            :disabled="!canSend || !hasPendingOutlineChapter"
            title="目录待写章齐后再点；会写正文"
            @click="onWriteAllByOutline"
          >
            全写
          </button>
          <button
            type="button"
            class="link-btn"
            :disabled="!canSend"
            title="保存创作提示"
            @click="onSaveBookOutline"
          >
            存提示
          </button>
        </template>
        <CapsuleSwitch
          v-if="task === 'continue' && !floatExtraVisible"
          v-model="instructionQueue"
          label="指令队列"
          :disabled="sectionQueueState.running"
        />
        <GenProgressBar variant="compact" />
        <div class="ai-float-foot-actions">
          <button type="button" class="link-btn" @click="onUndoAi">撤销</button>
          <button
            type="button"
            class="link-btn"
            title="切回右侧侧栏形态"
            @click="onToggleLayout"
          >
            侧栏
          </button>
        </div>
      </div>
    </div>

    <aside v-else class="ai-panel">
      <div class="dock-head">
        <h2 class="panel-heading">AI</h2>
        <button
          type="button"
          class="link-btn"
          title="切换为阅读区底部浮条"
          @click="onToggleLayout"
        >
          浮条模式
        </button>
      </div>
      <p class="focus-hint muted">{{ focusHint }}</p>
      <div class="task-row">
        <button
          v-for="t in tasks"
          :key="t.id"
          type="button"
          class="chip"
          :class="task === t.id ? 'chip-active' : ''"
          @click="task = t.id"
        >
          {{ t.label }}
        </button>
      </div>
      <div v-if="task === 'outline_run'" class="field outline-run-block">
        <div class="field-label-row">
          <label class="field-label">创作提示</label>
          <button
            type="button"
            class="clear-btn"
            title="保存提示"
            :disabled="!canSend"
            @click="onSaveBookOutline"
          >
            存
          </button>
        </div>
        <textarea
          v-model="bookOutline"
          rows="2"
          placeholder="一句话或几句提示即可 · 生成后弹窗确认写入左侧目录"
          :disabled="outlineQueueState.running"
        />
        <div class="outline-run-actions">
          <button
            type="button"
            class="app-btn app-btn-primary"
            :disabled="!canSend || !outlineSeedReady"
            @click="onGenerateChapterQueue"
          >
            生成章节队列
          </button>
          <button
            type="button"
            class="app-btn"
            :disabled="!canSend"
            @click="onContinueSplit"
          >
            续拆后续
          </button>
          <button
            type="button"
            class="app-btn"
            :disabled="!canSend || !hasPendingOutlineChapter"
            title="目录待写章齐后再点；会写正文"
            @click="onWriteAllByOutline"
          >
            全部按纲写
          </button>
        </div>
        <p
          v-if="outlineQueueStatusLine()"
          class="muted queue-status"
        >
          {{ outlineQueueStatusLine() }}
        </p>
      </div>
      <div v-if="task !== 'outline_run'" class="field">
        <div class="field-label-row">
          <label class="field-label">{{
            instructionQueue && task === 'continue'
              ? '指令队列'
              : '指令'
          }}</label>
          <button
            type="button"
            class="clear-btn"
            title="清空指令"
            :disabled="instructionQueue ? filledStepCount < 1 : !instruction"
            @click="clearInstruction"
          >
            <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
              <path
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                d="m13 11 9-9M14.6 12.6c1.1.9 1.4 2.4.8 3.6l-6.5 9.2c-.6.9-1.9 1.1-2.8.5l-4.2-3.1c-.9-.7-1.1-1.9-.5-2.8L8 14.1M5.4 15.4l4.2 3.1M11.8 12.4 8 8.6"
              />
            </svg>
          </button>
        </div>
        <template v-if="task === 'continue' && instructionQueue">
          <div class="instr-step-list">
            <div
              v-for="(step, si) in instructionSteps"
              :key="step.id"
              class="instr-step"
              :class="{
                current:
                  sectionQueueState.mode === 'manual' &&
                  sectionQueueState.phase === 'writing' &&
                  si === sectionQueueState.index - 1,
                done:
                  sectionQueueState.mode === 'manual' &&
                  (sectionQueueState.phase === 'done' ||
                    (sectionQueueState.phase === 'writing' && si < sectionQueueState.index - 1)),
              }"
            >
              <div class="instr-step-head">
                <span class="instr-step-idx">第 {{ si + 1 }} 步</span>
                <button
                  type="button"
                  class="step-icon-btn"
                  title="上移"
                  :disabled="si === 0 || sectionQueueState.running"
                  @click="moveInstructionStep(step.id, -1)"
                >
                  ↑
                </button>
                <button
                  type="button"
                  class="step-icon-btn"
                  title="下移"
                  :disabled="si >= instructionSteps.length - 1 || sectionQueueState.running"
                  @click="moveInstructionStep(step.id, 1)"
                >
                  ↓
                </button>
                <button
                  type="button"
                  class="step-icon-btn danger"
                  title="删除"
                  :disabled="sectionQueueState.running"
                  @click="removeInstructionStep(step.id)"
                >
                  ×
                </button>
              </div>
              <textarea
                :ref="(el) => setStepEl(step.id, el)"
                v-model="step.text"
                rows="2"
                :placeholder="`只写这一拍，例如：隔着内裤坐到脸上磨`"
                :disabled="sectionQueueState.running"
                @focus="activeStepId = step.id"
                @select="rememberStepCaret(step, $event)"
                @click="rememberStepCaret(step, $event)"
                @keyup="rememberStepCaret(step, $event)"
              />
            </div>
            <button
              type="button"
              class="app-btn step-add-btn"
              :disabled="
                sectionQueueState.running || instructionSteps.length >= MAX_INSTRUCTION_STEPS
              "
              @click="addInstructionStep"
            >
              加一步（{{ instructionSteps.length }}/{{ MAX_INSTRUCTION_STEPS }}）
            </button>
          </div>
        </template>
        <textarea
          v-else
          ref="instructionEl"
          v-model="instruction"
          rows="3"
          placeholder="例如：写一场雨夜对决"
          @select="rememberInstrCaret"
          @click="rememberInstrCaret"
          @keyup="rememberInstrCaret"
          @blur="rememberInstrCaret"
        />
        <div v-if="characterTags.length" class="char-tag-row" aria-label="本篇角色">
          <button
            v-for="c in characterTags"
            :key="c.id"
            type="button"
            class="char-tag"
            :title="`填入「${c.title}」`"
            @click="insertCharacterName(c.title)"
          >
            {{ c.title }}
          </button>
        </div>
      </div>
      <div v-if="task === 'continue'" class="field queue-field">
        <CapsuleSwitch
          v-model="instructionQueue"
          label="指令队列：多条指令按顺序连续生成"
          :disabled="sectionQueueState.running"
        />
        <p
          v-if="
            sectionQueueState.running ||
            sectionQueueState.phase === 'done' ||
            outlineQueueState.running ||
            outlineQueueState.phase === 'done'
          "
          class="muted queue-status"
        >
          {{ outlineQueueStatusLine() || queueStatusLine() }}
        </p>
        <ol
          v-if="sectionQueueState.sections.length && sectionQueueState.mode === 'plan'"
          class="queue-list"
        >
          <li
            v-for="(sec, qi) in sectionQueueState.sections"
            :key="`${qi}-${sec.title}`"
            :class="{
              current: sectionQueueState.phase === 'writing' && qi === sectionQueueState.index - 1,
              done:
                sectionQueueState.phase === 'done' ||
                (sectionQueueState.phase === 'writing' && qi < sectionQueueState.index - 1),
            }"
          >
            {{ sec.title }}
          </li>
        </ol>
      </div>
      <div class="field">
        <div class="field-label-row">
          <label class="field-label">选区（润色用）</label>
          <button
            type="button"
            class="clear-btn"
            title="清空选区"
            :disabled="!selection"
            @click="clearSelection"
          >
            <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
              <path
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                d="m13 11 9-9M14.6 12.6c1.1.9 1.4 2.4.8 3.6l-6.5 9.2c-.6.9-1.9 1.1-2.8.5l-4.2-3.1c-.9-.7-1.1-1.9-.5-2.8L8 14.1M5.4 15.4l4.2 3.1M11.8 12.4 8 8.6"
              />
            </svg>
          </button>
        </div>
        <textarea v-model="selection" rows="3" placeholder="粘贴要润色的段落" />
      </div>
      <div class="actions">
        <button
          type="button"
          class="app-btn app-btn-primary"
          :disabled="!canSend || (instructionQueue && task === 'continue' && filledStepCount < 1)"
          @click="onRun"
        >
          {{ runButtonLabel() }}
        </button>
        <button
          type="button"
          class="app-btn"
          :disabled="!canCancel"
          title="取消全部进行中的生成"
          @click="onCancelAll"
        >
          取消
        </button>
        <button type="button" class="app-btn" @click="onUndoAi">撤销上次 AI</button>
      </div>
      <div class="progress-slot">
        <GenProgressBar variant="panel" />
      </div>

      <div v-if="useEditorDraft" class="editor-draft-hint muted">
        <p>续写 / 润色在正文区流式显示，<strong>完成后自动写入</strong>。</p>
        <p v-if="task === 'continue' && instructionQueue">
          指令队列：按你填的每一步顺序连续调用续写，一步一节。
        </p>
        <p v-else-if="task === 'continue'">
          续写按整章一块写入；按纲生成也是每章只写一整段。
        </p>
        <p>生成块可点「重写」或「删除」；生成中可取消。</p>
        <p v-if="previewMeta" class="preview-meta">{{ previewMeta }}</p>
        <button
          v-if="canCancel && (appState.draftPlacement === 'editor' || sectionQueueState.running || outlineQueueState.running)"
          type="button"
          class="app-btn"
          style="margin-top: 8px"
          @click="onDiscard"
        >
          取消生成
        </button>
      </div>

      <template v-if="!useEditorDraft && task !== 'outline_run'">
        <div class="field preview-field">
          <label class="field-label">
            预览
            <CapsuleSwitch v-model="showDiff" label="Diff" class="diff-toggle" />
          </label>
          <p v-if="previewMeta" class="muted preview-meta">{{ previewMeta }}</p>
          <div v-if="diffRows.length" class="diff-box">
            <div
              v-for="(row, idx) in diffRows"
              :key="idx"
              class="diff-line"
              :class="row.type"
            >
              <span class="mark">{{
                row.type === "add" ? "+" : row.type === "remove" ? "-" : " "
              }}</span>
              {{ row.text }}
            </div>
          </div>
          <textarea
            v-else
            class="preview"
            :value="appState.previewText"
            readonly
            rows="12"
          />
        </div>
        <div class="actions">
          <button
            v-if="task === 'story_sync'"
            type="button"
            class="app-btn app-btn-primary"
            :disabled="!syncPatch"
            @click="onApplySync"
          >
            确认应用总谱 patch
          </button>
          <button
            v-if="task === 'outline' && appState.previewText"
            type="button"
            class="app-btn app-btn-primary"
            @click="onWriteOutlineToChapter"
          >
            写入本章纲
          </button>
          <button
            v-if="task === 'outline' && appState.previewText"
            type="button"
            class="app-btn"
            title="仅当预览确为全书大纲时使用"
            @click="onWriteOutlineToBook"
          >
            写入全书大纲
          </button>
          <button type="button" class="app-btn" @click="onDiscard">丢弃</button>
        </div>
      </template>

      <p v-if="syncMsg" class="muted">{{ syncMsg }}</p>
    </aside>
  </div>
</template>

<style scoped>
.ai-panel-root {
  display: contents;
}
.ai-panel-root[data-layout="float"] {
  display: block;
  flex-shrink: 0;
  width: 100%;
  pointer-events: auto;
}
.ai-panel-root[data-layout="dock"] {
  display: flex;
  flex-shrink: 0;
  align-self: stretch;
  min-height: 0;
}
.ai-panel {
  width: 320px;
  flex-shrink: 0;
  align-self: stretch;
  border-left: none;
  padding: 14px 16px;
  background: var(--panel);
  overflow: auto;
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow);
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.dock-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 2px;
}
.dock-head .panel-heading {
  margin: 0;
}
.focus-hint {
  font-size: 11px;
  line-height: 1.4;
  margin: 0 0 6px;
  flex-shrink: 0;
}
.field-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.field-label-row .field-label {
  margin: 0;
}
.clear-btn {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  height: 28px;
  padding: 0 6px;
  border: none;
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
}
.clear-btn:hover:not(:disabled) {
  background: var(--accent-soft);
  color: var(--accent-hover);
}
.clear-btn:disabled {
  opacity: 0.35;
  cursor: default;
}
.char-tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 8px;
}
.char-tag {
  border: none;
  background: var(--chip-bg);
  color: var(--text);
  border-radius: var(--radius-pill);
  padding: 3px 10px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
  line-height: 1.4;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.char-tag:hover {
  background: var(--accent-soft);
  color: var(--accent-hover);
}
.queue-field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.queue-status {
  font-size: 12px;
  margin: 0;
}
.queue-list {
  margin: 0;
  padding-left: 1.2em;
  font-size: 12px;
  line-height: 1.6;
}
.queue-list li.current {
  font-weight: 700;
  color: var(--accent-hover, var(--accent));
}
.queue-list li.done {
  opacity: 0.55;
}
.ai-float-queue {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 12px;
  margin-top: 8px;
}
.instr-step-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 8px;
}
.instr-step-list.float-steps {
  max-height: 280px;
  overflow: auto;
  padding-right: 2px;
}
.instr-step {
  border: 1px solid color-mix(in srgb, var(--muted) 28%, transparent);
  border-radius: var(--radius-md);
  padding: 8px;
  background: var(--surface-solid, #fafafa);
}
.instr-step.current {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 35%, transparent);
}
.instr-step.done {
  opacity: 0.62;
}
.instr-step-head {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-bottom: 6px;
}
.instr-step-idx {
  font-size: 12px;
  font-weight: 700;
  margin-right: auto;
  color: var(--accent-hover, var(--accent));
}
.instr-step textarea {
  width: 100%;
  box-sizing: border-box;
  resize: vertical;
  min-height: 48px;
}
.step-icon-btn {
  border: none;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  font-size: 13px;
  font-weight: 700;
  line-height: 1;
  padding: 2px 6px;
  border-radius: 4px;
}
.step-icon-btn:hover:not(:disabled) {
  background: var(--accent-soft);
  color: var(--accent-hover);
}
.step-icon-btn.danger:hover:not(:disabled) {
  color: #b33;
}
.step-icon-btn:disabled {
  opacity: 0.35;
  cursor: default;
}
.step-add-btn {
  align-self: flex-start;
}
.ai-float-input.queue-hint {
  display: flex;
  align-items: center;
  min-height: 36px;
  padding: 0 10px;
  font-size: 12px;
}
.task-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 10px 0;
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
.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 8px;
  flex-shrink: 0;
}
.progress-slot {
  flex: 0 0 auto;
  height: auto;
  max-height: 36px;
  margin: 8px 0 4px;
  overflow: hidden;
}
.editor-draft-hint {
  margin-top: 12px;
  font-size: 12px;
  line-height: 1.5;
}
.editor-draft-hint p {
  margin: 0 0 6px;
}
.outline-run-block {
  margin-top: 4px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.outline-run-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}
.outline-run-block .queue-status {
  font-size: 12px;
  margin: 0;
}
.preview-field {
  flex: 1 1 auto;
  min-height: 120px;
  margin-top: 8px;
}
.preview {
  font-family: var(--font-mono);
  font-size: 12px;
  min-height: 100px;
  resize: vertical;
}
.diff-toggle {
  margin-left: 8px;
  font-weight: 500;
  font-size: 12px;
  color: var(--muted);
}
.preview-meta {
  font-size: 11px;
  margin: 4px 0 6px;
}
.diff-box {
  max-height: 220px;
  overflow: auto;
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.45;
  background: var(--surface-solid);
  border-radius: var(--radius-md);
  padding: 8px;
  box-shadow: var(--shadow-sm);
}
.diff-line {
  white-space: pre-wrap;
  word-break: break-word;
}
.diff-line.add {
  background: rgba(46, 160, 67, 0.18);
  color: #1a7f37;
}
.diff-line.remove {
  background: rgba(248, 81, 73, 0.16);
  color: #cf222e;
}
.mark {
  display: inline-block;
  width: 1em;
  opacity: 0.7;
}
.error {
  color: var(--error);
  margin-top: 8px;
}
.link-btn {
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: var(--radius-pill);
}
.link-btn:hover {
  color: var(--accent-hover);
  background: var(--accent-soft);
}

/* —— 浮条 —— */
.ai-float {
  flex-shrink: 0;
  margin: 0 4px 4px;
  padding: 6px 10px 5px;
  background: var(--panel, #fff);
  border: 1px solid color-mix(in srgb, var(--accent) 18%, transparent);
  border-radius: 14px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.08), var(--shadow-sm, none);
  z-index: 12;
}
.ai-float.is-busy {
  padding-top: 5px;
  padding-bottom: 4px;
}
.ai-float-extra {
  margin-bottom: 6px;
  padding-bottom: 6px;
  border-bottom: 1px solid color-mix(in srgb, var(--muted) 25%, transparent);
}
.ai-float-tasks {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 8px;
}
.ai-float-selection textarea {
  width: 100%;
  box-sizing: border-box;
  min-height: 48px;
  resize: vertical;
}
.float-char-tags {
  flex-wrap: nowrap;
  overflow-x: auto;
  margin-top: 4px;
  gap: 4px;
  padding-bottom: 2px;
  -webkit-overflow-scrolling: touch;
}
.float-char-tags .char-tag {
  flex: 0 0 auto;
  padding: 2px 8px;
  font-size: 11px;
}
.float-sync-msg {
  margin: 4px 0 0;
  font-size: 11px;
}
.float-sync-inline {
  flex: 0 1 160px;
  max-width: 220px;
}
.ai-float-bar {
  display: flex;
  align-items: flex-end;
  gap: 6px;
}
.ai-float-task-wrap {
  flex: 0 0 auto;
}
.ai-float-task {
  height: 32px;
  border: 1px solid color-mix(in srgb, var(--muted) 35%, transparent);
  border-radius: var(--radius-pill);
  background: var(--surface-solid, #f7f7f7);
  color: var(--text);
  font-size: 12px;
  font-weight: 650;
  padding: 0 8px;
  cursor: pointer;
}
.ai-float-input {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 32px;
  max-height: 72px;
  margin: 0;
  padding: 6px 10px;
  border: 1px solid color-mix(in srgb, var(--muted) 30%, transparent);
  border-radius: 10px;
  background: var(--surface-solid, #fafafa);
  color: var(--text);
  font-size: 13px;
  line-height: 1.4;
  resize: none;
  field-sizing: content;
}
.ai-float-input:focus {
  outline: 1px solid color-mix(in srgb, var(--accent) 55%, transparent);
  border-color: color-mix(in srgb, var(--accent) 45%, transparent);
}
.ai-float-actions {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}
.ai-float-send {
  min-width: 56px;
  padding-left: 10px;
  padding-right: 10px;
}
.ai-float-icon-btn {
  border: none;
  background: var(--chip-bg, #f0f0f0);
  color: var(--muted);
  border-radius: var(--radius-pill);
  padding: 5px 8px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}
.ai-float-icon-btn:hover:not(:disabled) {
  background: var(--accent-soft);
  color: var(--accent-hover);
}
.ai-float-icon-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.ai-float-foot {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  gap: 6px 8px;
  margin-top: 4px;
  min-height: 20px;
  overflow: hidden;
}
.ai-float-foot .tip {
  font-size: 11px;
  flex: 1 1 80px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ai-float-foot :deep(.gen-progress.compact) {
  flex: 0 0 140px;
  width: 140px;
  max-width: 140px;
  height: 18px;
  max-height: 20px;
}
.ai-float-foot-actions {
  display: flex;
  gap: 2px;
  margin-left: auto;
  flex-shrink: 0;
}
</style>
