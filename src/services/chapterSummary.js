/**
 * 写作页：AI 全篇（本章）总结 → 写入章纲 summary + 后端快照
 * 代码路径: kk_novel_ai/src/services/chapterSummary.js
 */
import { appState } from "../stores/appState.js";
import { runWriting } from "./llmClient.js";
import { saveChapter, updateChapterMeta } from "./projectClient.js";
import { createGenJob, discardJob } from "../stores/genJobs.js";
import { t } from "../i18n/index.js";

let inFlight = false;

export function isChapterSummaryBusy() {
  return inFlight;
}

/**
 * @param {{ chapterId?: string, overwriteConfirm?: () => Promise<boolean> }} [opts]
 * @returns {Promise<string|null>}
 */
export async function runChapterAiSummary(opts = {}) {
  const chapterId = String(opts.chapterId || appState.chapterId || "").trim();
  if (!chapterId || !appState.projectRoot) {
    throw new Error(t("editor.needProjectChapter"));
  }
  if (inFlight || appState.generating) {
    throw new Error(t("editor.summaryBusy"));
  }

  const ch = (appState.project?.chapters || []).find((c) => c && c.id === chapterId);
  const prevSummary = String((ch && ch.summary) || "").trim();
  if (prevSummary && typeof opts.overwriteConfirm === "function") {
    const ok = await opts.overwriteConfirm();
    if (!ok) return null;
  }

  inFlight = true;
  const prevStatus = appState.statusMessage;
  appState.statusMessage = t("editor.summarizingChapter");
  const job = createGenJob({
    label: t("editor.summaryJobLabel", {
      title: (ch && ch.title) || t("editor.thisChapter"),
    }),
    skipAutoAccept: true,
  });
  job.draftPlacement = "";
  job.draftTask = "chapter_summary";

  try {
    if (appState.dirty && appState.chapterId === chapterId) {
      await saveChapter();
    }
    const result = await runWriting(
      {
        project_root: appState.projectRoot,
        chapter_id: chapterId,
        task: "chapter_summary",
        instruction: "",
        selection: "",
      },
      { job, label: job.label }
    );
    const text = String(
      (result && (result.text || result.raw_text)) ||
        job.previewRawText ||
        job.previewText ||
        ""
    ).trim();
    if (text.length < 24) {
      throw new Error(t("editor.summaryTooShort"));
    }
    await updateChapterMeta(chapterId, { summary: text });
    appState.statusMessage = t("editor.summaryChapterDone");
    return text;
  } catch (e) {
    const msg = String(e && e.message ? e.message : e);
    appState.statusMessage = t("editor.summaryChapterFail", { msg });
    throw e;
  } finally {
    discardJob(job);
    inFlight = false;
    if (appState.statusMessage === t("editor.summarizingChapter")) {
      appState.statusMessage = prevStatus || "";
    }
  }
}
