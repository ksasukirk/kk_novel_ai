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
  /** 全书大纲（与 project.book_outline 同步） */
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

/** 从工程同步全书大纲到面板 */
export function syncBookOutlineFromProject(project) {
  aiPanelForm.bookOutline = String((project && project.book_outline) || "");
}

watch(
  () => aiPanelForm.error,
  (v) => {
    if (!v) return;
    toastErrorLines(v);
    aiPanelForm.error = "";
  }
);