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
import { isCancelledMsg, t, tLocale } from "../i18n/index.js";

function writingT(key, values) {
  return tLocale(appState.settings?.writing_locale || "zh-CN", key, values);
}

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
  const kind = s.mode === "manual" ? t("sectionQ.kindManual") : t("sectionQ.kindPlan");
  if (s.phase === "planning") return t("sectionQ.planning");
  if (s.phase === "writing") {
    const title = s.sections[s.index - 1]?.title || "";
    const bit = title ? ` · ${title}` : "";
    return t("sectionQ.writing", { kind, index: s.index, total: s.total, bit });
  }
  if (s.phase === "done") return t("sectionQ.done", { kind, n: s.total });
  if (s.phase === "cancelled") return t("sectionQ.cancelled", { kind });
  if (s.phase === "error") return s.error || t("sectionQ.failed", { kind });
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
      throw new Error(t("sectionQ.cancelledErr"));
    }
    await sleep(350);
  }
}

function throwIfCancelled() {
  if (sectionQueueState.cancelled) {
    throw new Error(t("sectionQ.cancelledErr"));
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
      title: writingT("ai.taskContinue"),
      instruction: String(fallbackInstruction || "").trim() || writingT("sectionQ.fallbackInstr"),
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
      title: title || writingT("sectionQ.sectionN", { n: sections.length + 1 }),
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
      const step = raw.trim();
      if (!step) continue;
      out.push({ title: writingT("sectionQ.stepN", { n: out.length + 1 }), instruction: step });
      continue;
    }
    if (!raw || typeof raw !== "object") continue;
    const instruction = String(raw.instruction || raw.text || "").trim();
    if (!instruction) continue;
    const title = String(raw.title || "").trim() || writingT("sectionQ.stepN", { n: out.length + 1 });
    out.push({ title, instruction });
  }
  return out;
}

function wrapSectionInstruction(userInstr, section, index, total, mode) {
  const user = String(userInstr || "").trim();
  const title = section.title || writingT("sectionQ.sectionN", { n: index });
  const tag = mode === "manual" ? writingT("sectionQ.kindManual") : writingT("sectionQ.kindPlan");
  const parts = [
    writingT("sectionQ.wrap", { tag, index, total, title }),
  ];
  if (mode === "plan" && user) {
    parts.push(writingT("sectionQ.sharedInstr", { user }));
  }
  parts.push(writingT("sectionQ.sectionTask", { instruction: section.instruction }));
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
      label: t("sectionQ.labelSection", { index: i + 1, total: sections.length }),
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
      if (sectionQueueState.cancelled || isCancelledMsg(msg)) {
        throw new Error(t("sectionQ.cancelledErr"));
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
    throw new Error(t("outlineQ.needOpen"));
  }
  if (sectionQueueState.running) {
    throw new Error(t("sectionQ.alreadyRunning"));
  }
  if (visibleGenJobs.value.length) {
    throw new Error(t("sectionQ.waitDraft"));
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
    throw new Error(t("sectionQ.needStep"));
  }

  resetQueue();
  sectionQueueState.running = true;
  sectionQueueState.mode = "manual";
  sectionQueueState.phase = "writing";
  appState.statusMessage = t("sectionQ.startManual", { n: sections.length });

  try {
    if (appState.dirty) await saveChapter();
    await runQueuedContinues(sections, { mode: "manual" });
    sectionQueueState.phase = "done";
    sectionQueueState.running = false;
    appState.statusMessage =
      sections.length > 1
        ? t("sectionQ.doneManual", { n: sections.length })
        : t("draft.writeSaved");
  } catch (e) {
    const msg = String(e.message || e);
    const cancelled = sectionQueueState.cancelled || isCancelledMsg(msg);
    sectionQueueState.running = false;
    sectionQueueState.phase = cancelled ? "cancelled" : "error";
    sectionQueueState.error = cancelled ? "" : msg;
    appState.statusMessage = cancelled ? t("sectionQ.cancelledManual") : msg;
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
  appState.statusMessage = t("sectionQ.planning");

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

    const planJob = createGenJob({ label: t("sectionQ.labelPlan") });
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
        { job: planJob, label: t("sectionQ.labelPlan") }
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
        ? t("sectionQ.planWait", { n: sections.length })
        : t("sectionQ.planOne");

    if (sections.length > 1) {
      const titles = sections
        .map((s, i) => `${i + 1}. ${s.title || writingT("sectionQ.sectionN", { n: i + 1 })}`)
        .join("\n");
      const reason = sectionQueueState.reason
        ? t("sectionQ.reason", { reason: sectionQueueState.reason })
        : "";
      const ok = await appConfirm(
        t("sectionQ.confirmBody", { n: sections.length, titles, reason }),
        {
          title: t("sectionQ.confirmTitle"),
          confirmText: t("sectionQ.startGen"),
          cancelText: t("common.cancel"),
        }
      );
      throwIfCancelled();
      if (!ok) {
        sectionQueueState.phase = "cancelled";
        sectionQueueState.running = false;
        appState.statusMessage = t("sectionQ.cancelledMulti", { n: sections.length });
        return;
      }
    }

    await runQueuedContinues(sections, { mode: "plan", userInstr });

    sectionQueueState.phase = "done";
    sectionQueueState.running = false;
    appState.statusMessage =
      sections.length > 1
        ? t("sectionQ.donePlan", { n: sections.length })
        : t("draft.writeSaved");
  } catch (e) {
    const msg = String(e.message || e);
    const cancelled = sectionQueueState.cancelled || isCancelledMsg(msg);
    sectionQueueState.running = false;
    sectionQueueState.phase = cancelled ? "cancelled" : "error";
    sectionQueueState.error = cancelled ? "" : msg;
    appState.statusMessage = cancelled ? t("sectionQ.cancelledPlan") : msg;
    if (!cancelled) throw e;
  }
}
