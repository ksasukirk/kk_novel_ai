/**
 * AI 面板共享表单状态（浮条 / 侧栏切换不丢内容）
 * 代码路径: kk_novel_ai/src/stores/aiPanelState.js
 */
import { reactive, ref, watch } from "vue";
import { toastErrorLines } from "../services/toast.js";

function newStepId() {
  return `step-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
}

export function createInstructionStep(text = "") {
  return { id: newStepId(), text: String(text || "") };
}

export const aiPanelForm = reactive({
  task: "outline_run",
  instruction: "",
  /** 按任务分槽，切任务不互相覆盖 */
  instructionByTask: {
    continue: "",
    polish: "",
    outline: "",
    consistency: "",
    chapter_summary: "",
    story_sync: "",
  },
  selection: "",
  showDiff: true,
  /** @deprecated 已取消自动分节；保留字段避免旧会话报错，恒为 false */
  sectionQueue: false,
  /** @deprecated 已并入任务 outline_run；保留字段避免旧会话报错 */
  outlineQueue: false,
  /** 手动指令队列：多输入框按顺序连续生成 */
  instructionQueue: false,
  /** @type {Array<{id:string, text:string}>} */
  instructionSteps: [createInstructionStep("")],
  /** 全书大纲 / 创作提示（与 project.book_outline 同步；未保存草稿不可被冲掉） */
  bookOutline: "",
  /**
   * 拆章预览
   * @type {Array<{title:string, summary:string, must_do:string, selected:boolean}>}
   */
  chapterPlan: [],
  syncMsg: "",
  error: "",
  floatExpanded: false,
});

/** 指令框光标（失焦后插入角色名用） */
export const instrCaret = ref({ start: null, end: null });

/** 当前聚焦的指令步骤 id（角色标签插入用） */
export const activeStepId = ref("");

/** 上次从工程同步/保存成功后的创作提示基线；与 bookOutline 不同即视为未保存草稿 */
let lastSyncedBookOutline = "";

/** 面板创作提示是否相对磁盘有未保存改动 */
export function isBookOutlineDirty() {
  return String(aiPanelForm.bookOutline || "") !== lastSyncedBookOutline;
}

/**
 * 从工程同步全书大纲到面板。
 * 默认：有未保存草稿则不覆盖；`force: true` 用于切换/打开作品。
 * @param {object|null|undefined} project
 * @param {{ force?: boolean }} [opts]
 * @returns {boolean} 是否写入了面板
 */
export function syncBookOutlineFromProject(project, opts = {}) {
  const next = String((project && project.book_outline) || "");
  const force = !!opts.force;
  const cur = String(aiPanelForm.bookOutline || "");
  if (!force && cur !== lastSyncedBookOutline) {
    // 草稿内容已与磁盘一致时只清脏，仍不强制改 UI
    if (cur === next) {
      lastSyncedBookOutline = next;
    }
    return false;
  }
  aiPanelForm.bookOutline = next;
  lastSyncedBookOutline = next;
  return true;
}

/** 保存成功后对齐基线（避免随后 getProject 再冲） */
export function noteBookOutlineSaved(text) {
  const v = String(text != null ? text : aiPanelForm.bookOutline || "");
  aiPanelForm.bookOutline = v;
  lastSyncedBookOutline = v;
}

watch(
  () => aiPanelForm.error,
  (v) => {
    if (!v) return;
    toastErrorLines(v);
    aiPanelForm.error = "";
  }
);

watch(
  () => aiPanelForm.task,
  (id, prev) => {
    if (prev && prev !== "outline_run") {
      aiPanelForm.instructionByTask[prev] = String(aiPanelForm.instruction || "");
    }
    if (id === "outline_run") return;
    if (!(id in aiPanelForm.instructionByTask)) {
      aiPanelForm.instructionByTask[id] = "";
    }
    aiPanelForm.instruction = String(aiPanelForm.instructionByTask[id] || "");
  }
);

watch(
  () => aiPanelForm.instruction,
  (v) => {
    const t = aiPanelForm.task;
    if (t === "outline_run") return;
    aiPanelForm.instructionByTask[t] = String(v || "");
  }
);
