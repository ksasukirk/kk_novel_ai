/**
 * 分节 / 指令队列：按顺序调用续写 API
 * - plan：先让模型规划小节，再排队
 * - manual：用户多条指令列表，直接排队
 * 代码路径: kk_novel_ai/src/services/sectionQueue.js
 */
import { reactive } from "vue";
import { appState } from "../stores/appState.js";
import { runWriting } from "./llmClient.js";
import { acceptDraft, withBranchContext } from "./draftAccept.js";
import { saveChapter } from "./projectClient.js";
import { appConfirm } from "./confirmDialog.js";
import {
  canStartMoreJobs,
  createGenJob,
  discardJob,
  visibleGenJobs,
} from "../stores/genJobs.js";

export const MAX_QUEUE_SECTIONS = 8;
export const MAX_INSTRUCTION_STEPS = 12;

export const sectionQueueState = reactive({
  running: false,
  cancelled: false,
  /** "" | "planning" | "writing" | "done" | "cancelled" | "error" */
  phase: "",
  /** "" | "plan" | "manual" */
  mode: "",
  total: 0,
  index: 0,
  reason: "",
  sections: [],
  error: "",
});

export function queueStatusLine() {
  const s = sectionQueueState;
  if (!s.running && s.phase !== "done") return "";
  const kind = s.mode === "manual" ? "指令队列" : "分节队列";
  if (s.phase === "planning") return "正在分析需要几节…";
  if (s.phase === "writing") {
    const title = s.sections[s.index - 1]?.title || "";
    const bit = title ? ` · ${title}` : "";
    return `${kind} ${s.index}/${s.total}${bit}`;
  }
  if (s.phase === "done") return `${kind}已写完 ${s.total} 节`;
  if (s.phase === "cancelled") return `已取消${kind}`;
  if (s.phase === "error") return s.error || `${kind}失败`;
  return "";
}

export function cancelSectionQueue() {
  sectionQueueState.cancelled = true;
}

function resetQueue() {
  sectionQueueState.running = false;
  sectionQueueState.cancelled = false;
  sectionQueueState.phase = "";
  sectionQueueState.mode = "";
  sectionQueueState.total = 0;
  sectionQueueState.index = 0;
  sectionQueueState.reason = "";
  sectionQueueState.sections = [];
  sectionQueueState.error = "";
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function waitForSlot() {
  while (!canStartMoreJobs(1)) {
    if (sectionQueueState.cancelled) {
      throw new Error("已取消队列");
    }
    await sleep(350);
  }
}

function throwIfCancelled() {
  if (sectionQueueState.cancelled) {
    throw new Error("已取消队列");
  }
}

/**
 * 从模型输出里抠 JSON 分节计划
 * @param {string} text
 * @param {string} fallbackInstruction
 */
export function parseSectionPlan(text, fallbackInstruction = "") {
  const raw = String(text || "").trim();
  const fallback = [
    {
      title: "续写",
      instruction: String(fallbackInstruction || "").trim() || "承接上文续写一节，写完即停。",
    },
  ];
  if (!raw) return fallback;

  let body = raw;
  const fence = raw.match(/```(?:json)?\s*([\s\S]*?)```/i);
  if (fence) body = fence[1].trim();
  const start = body.indexOf("{");
  const end = body.lastIndexOf("}");
  if (start < 0 || end <= start) return fallback;

  let data;
  try {
    data = JSON.parse(body.slice(start, end + 1));
  } catch {
    return fallback;
  }

  const list = Array.isArray(data.sections)
    ? data.sections
    : Array.isArray(data.beats)
      ? data.beats
      : [];
  const sections = [];
  for (const item of list) {
    if (!item || typeof item !== "object") continue;
    const instruction = String(item.instruction || item.task || item.prompt || "").trim();
    const title = String(item.title || item.name || "").trim();
    if (!instruction && !title) continue;
    sections.push({
      title: title || `第${sections.length + 1}节`,
      instruction: instruction || title,
    });
  }
  if (!sections.length) return fallback;
  return sections.slice(0, MAX_QUEUE_SECTIONS);
}

/**
 * @param {Array<{ text?: string, instruction?: string, title?: string }|string>} steps
 */
export function normalizeInstructionSteps(steps) {
  const list = Array.isArray(steps) ? steps : [];
  const out = [];
  for (const raw of list) {
    if (out.length >= MAX_INSTRUCTION_STEPS) break;
    if (typeof raw === "string") {
      const t = raw.trim();
      if (!t) continue;
      out.push({ title: `第${out.length + 1}步`, instruction: t });
      continue;
    }
    if (!raw || typeof raw !== "object") continue;
    const instruction = String(raw.instruction || raw.text || "").trim();
    if (!instruction) continue;
    const title = String(raw.title || "").trim() || `第${out.length + 1}步`;
    out.push({ title, instruction });
  }
  return out;
}

function wrapSectionInstruction(userInstr, section, index, total, mode) {
  const user = String(userInstr || "").trim();
  const title = section.title || `第${index}节`;
  const tag = mode === "manual" ? "指令队列" : "分节队列";
  const parts = [
    `【${tag} ${index}/${total} · ${title}】只写本节；须达到或超出规定字数后再停；禁止提前写下一节，禁止复述已写内容。`,
  ];
  if (mode === "plan" && user) {
    parts.push(`总指令（全队列共用，本节只兑现其中这一拍）：\n${user}`);
  }
  parts.push(`本节任务：\n${section.instruction}`);
  return parts.join("\n");
}

/**
 * @param {Array<{title:string, instruction:string}>} sections
 * @param {{ mode: "plan"|"manual", userInstr?: string }} meta
 */
async function runQueuedContinues(sections, meta) {
  const mode = meta.mode || "plan";
  const userInstr = String(meta.userInstr || "").trim();
  sectionQueueState.mode = mode;
  sectionQueueState.sections = sections;
  sectionQueueState.total = sections.length;
  sectionQueueState.phase = "writing";

  for (let i = 0; i < sections.length; i++) {
    throwIfCancelled();
    await waitForSlot();
    if (appState.dirty) await saveChapter();

    const section = sections[i];
    sectionQueueState.index = i + 1;
    const wrapped = wrapSectionInstruction(
      userInstr,
      section,
      i + 1,
      sections.length,
      mode
    );
    appState.statusMessage = queueStatusLine();

    appState.draftPlacement = "editor";
    appState.draftTask = "continue";
    appState.draftSelection = "";
    appState.draftInstruction = wrapped;
    appState.draftPersistInstruction = section.instruction;
    appState.draftRewriteBlockKey = "";
    appState.draftAnchorBlockKey = "";
    appState.draftBranchMode = "";
    appState.draftBranchNodeId = "";
    appState.draftForkFromVariantId = "";

    const job = createGenJob({
      label: `第${i + 1}/${sections.length}节`,
    });
    try {
      await runWriting(
        withBranchContext(
          {
            project_root: appState.projectRoot,
            chapter_id: appState.chapterId,
            task: "continue",
            instruction: wrapped,
            selection: "",
          },
          "continue",
          ""
        ),
        { job, label: job.label }
      );
    } catch (e) {
      const msg = String(e.message || e);
      if (sectionQueueState.cancelled || /取消/.test(msg)) {
        throw new Error("已取消队列");
      }
      throw e;
    }
    throwIfCancelled();
    if (job.status === "done" && !job.accepted) {
      await acceptDraft(job);
    }
  }
}

function assertCanStartQueue() {
  if (!appState.projectRoot || !appState.chapterId) {
    throw new Error("请先打开作品并选择章节");
  }
  if (sectionQueueState.running) {
    throw new Error("队列已在进行中");
  }
  if (visibleGenJobs.value.length) {
    throw new Error("请先等当前草稿写完或取消，再开队列");
  }
}

/**
 * 用户指令列表：跳过规划，按顺序连续续写
 * @param {{ steps: Array, selection?: string }} opts
 */
export async function runInstructionQueue(opts = {}) {
  assertCanStartQueue();
  const sections = normalizeInstructionSteps(opts.steps);
  if (!sections.length) {
    throw new Error("请至少填写一条指令");
  }

  resetQueue();
  sectionQueueState.running = true;
  sectionQueueState.mode = "manual";
  sectionQueueState.phase = "writing";
  appState.statusMessage = `指令队列 ${sections.length} 步，开始生成…`;

  try {
    if (appState.dirty) await saveChapter();
    await runQueuedContinues(sections, { mode: "manual" });
    sectionQueueState.phase = "done";
    sectionQueueState.running = false;
    appState.statusMessage =
      sections.length > 1
        ? `指令队列已写完 ${sections.length} 步`
        : "生成已写入并保存";
  } catch (e) {
    const msg = String(e.message || e);
    const cancelled = sectionQueueState.cancelled || /取消/.test(msg);
    sectionQueueState.running = false;
    sectionQueueState.phase = cancelled ? "cancelled" : "error";
    sectionQueueState.error = cancelled ? "" : msg;
    appState.statusMessage = cancelled ? "已取消指令队列" : msg;
    if (!cancelled) throw e;
  }
}

/**
 * 规划 + 按节排队续写
 * @param {{ instruction?: string, selection?: string }} opts
 */
export async function runSectionQueue(opts = {}) {
  assertCanStartQueue();

  const userInstr = String(opts.instruction || "").trim();
  const selection = String(opts.selection || "").trim();

  resetQueue();
  sectionQueueState.running = true;
  sectionQueueState.mode = "plan";
  sectionQueueState.phase = "planning";
  appState.statusMessage = "正在分析需要几节…";

  try {
    if (appState.dirty) await saveChapter();
    throwIfCancelled();
    await waitForSlot();

    appState.draftPlacement = "";
    appState.draftTask = "section_plan";
    appState.draftSelection = selection;
    appState.draftInstruction = userInstr;
    appState.draftPersistInstruction = "";
    appState.draftRewriteBlockKey = "";
    appState.draftAnchorBlockKey = "";
    appState.draftBranchMode = "";
    appState.draftBranchNodeId = "";
    appState.draftForkFromVariantId = "";

    const planJob = createGenJob({ label: "分节规划" });
    planJob.draftPlacement = "";
    let planResult;
    try {
      planResult = await runWriting(
        withBranchContext(
          {
            project_root: appState.projectRoot,
            chapter_id: appState.chapterId,
            task: "section_plan",
            instruction: userInstr,
            selection,
          },
          "continue",
          ""
        ),
        { job: planJob, label: "分节规划" }
      );
    } finally {
      discardJob(planJob);
    }
    throwIfCancelled();

    const planText =
      (planResult && (planResult.raw_text || planResult.text)) ||
      planJob.previewRawText ||
      planJob.previewText ||
      "";
    const sections = parseSectionPlan(planText, userInstr);
    sectionQueueState.reason = "";
    try {
      const start = String(planText).indexOf("{");
      const end = String(planText).lastIndexOf("}");
      if (start >= 0 && end > start) {
        const data = JSON.parse(String(planText).slice(start, end + 1));
        sectionQueueState.reason = String(data.reason || "").trim();
      }
    } catch {
      /* ignore */
    }

    appState.statusMessage =
      sections.length > 1
        ? `计划 ${sections.length} 节，等待确认…`
        : "计划 1 节，开始续写";

    if (sections.length > 1) {
      const titles = sections
        .map((s, i) => `${i + 1}. ${s.title || `第${i + 1}节`}`)
        .join("\n");
      const reason = sectionQueueState.reason
        ? `\n原因：${sectionQueueState.reason}`
        : "";
      const ok = await appConfirm(
        `自动分节计划了 ${sections.length} 节，确认后会连续生成并写入：\n${titles}${reason}\n\n取消则只保留计划、不生成。`,
        {
          title: "确认多节生成",
          confirmText: "开始生成",
          cancelText: "取消",
        }
      );
      throwIfCancelled();
      if (!ok) {
        sectionQueueState.phase = "cancelled";
        sectionQueueState.running = false;
        appState.statusMessage = `已取消多节生成（计划 ${sections.length} 节）`;
        return;
      }
    }

    await runQueuedContinues(sections, { mode: "plan", userInstr });

    sectionQueueState.phase = "done";
    sectionQueueState.running = false;
    appState.statusMessage =
      sections.length > 1
        ? `分节队列已写完 ${sections.length} 节`
        : "生成已写入并保存";
  } catch (e) {
    const msg = String(e.message || e);
    const cancelled = sectionQueueState.cancelled || /取消/.test(msg);
    sectionQueueState.running = false;
    sectionQueueState.phase = cancelled ? "cancelled" : "error";
    sectionQueueState.error = cancelled ? "" : msg;
    appState.statusMessage = cancelled ? "已取消分节队列" : msg;
    if (!cancelled) throw e;
  }
}
