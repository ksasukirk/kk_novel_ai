/**
 * 把全书大纲整理成思维导图并写入 project.outline_mindmap
 * 代码路径: kk_novel_ai/src/services/outlineMindmap.js
 */
import { appState } from "../stores/appState.js";
import { runWriting } from "./llmClient.js";
import { saveProjectMeta } from "./projectClient.js";
import { createGenJob, discardJob } from "../stores/genJobs.js";
import { parseOutlineMindmap } from "../utils/outlineMindTree.js";

function fallbackChapterId() {
  return (
    appState.chapterId ||
    ((appState.project && appState.project.chapters && appState.project.chapters[0] &&
      appState.project.chapters[0].id) ||
      "")
  );
}

/**
 * @param {{ instruction?: string }} [opts]
 * @returns {Promise<{ reason: string, root: object }>}
 */
export async function runOutlineToMindmap(opts = {}) {
  if (!appState.projectRoot || !appState.project) {
    throw new Error("请先打开作品");
  }
  const outline = String(appState.project.book_outline || "").trim();
  const chapters = appState.project.chapters || [];
  const hasChapterOutline = chapters.some(
    (c) => String((c && c.summary) || "").trim() || (Array.isArray(c.beats) && c.beats.length)
  );
  if (!outline && !hasChapterOutline) {
    throw new Error("请先填写全书大纲或章纲");
  }
  const chapterId = fallbackChapterId();
  if (!chapterId) {
    throw new Error("作品还没有章节，无法整理导图");
  }

  const prevPlacement = appState.draftPlacement;
  appState.draftPlacement = "";
  const job = createGenJob({ label: "整理成导图" });
  job.draftPlacement = "";
  job.draftTask = "outline_to_mindmap";

  let result;
  try {
    result = await runWriting(
      {
        project_root: appState.projectRoot,
        chapter_id: chapterId,
        task: "outline_to_mindmap",
        instruction: String(opts.instruction || "").trim(),
        selection: "",
      },
      { job, label: job.label }
    );
  } catch (e) {
    discardJob(job);
    appState.draftPlacement = prevPlacement;
    throw e;
  }
  discardJob(job);
  appState.draftPlacement = prevPlacement;

  const text =
    (result && (result.raw_text || result.text)) ||
    job.previewRawText ||
    job.previewText ||
    "";
  const { reason, root } = parseOutlineMindmap(text);
  if (!root) {
    throw new Error("未能整理出导图，请检查大纲后重试");
  }

  const now = new Date().toISOString();
  const outline_mindmap = {
    source: hasChapterOutline ? "structure" : "book",
    generated_at: now,
    reason,
    root,
  };
  await saveProjectMeta({
    ...appState.project,
    outline_mindmap,
  });
  appState.statusMessage = reason ? `导图已保存 · ${reason}` : "导图已保存";
  return { reason, root };
}
