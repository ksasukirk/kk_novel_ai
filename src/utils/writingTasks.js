/**
 * 写作任务分类：哪些只产出结构化结果、不得写入章节正文
 * 代码路径: kk_novel_ai/src/utils/writingTasks.js
 */
export function isBackgroundAnalysisTask(task) {
  const t = String(task || "");
  return (
    t === "block_digest" ||
    t === "digest" ||
    t === "cast_extract" ||
    t === "auto_cast" ||
    t === "section_plan" ||
    t === "plan_sections" ||
    t === "outline_to_beats" ||
    t === "split_beats" ||
    t === "outline_to_chapters" ||
    t === "split_chapters" ||
    t === "outline_to_mindmap" ||
    t === "mindmap_outline" ||
    t === "chapter_summary" ||
    t === "summarize" ||
    t === "consistency_check" ||
    t === "story_sync" ||
    t === "sync_story" ||
    t === "beats_to_storyboard" ||
    t === "storyboard_from_beats" ||
    t === "content_to_image_prompt" ||
    t === "image_prompt"
  );
}

export function isStorySyncTask(task) {
  const t = String(task || "");
  return t === "story_sync" || t === "sync_story";
}

/**
 * 后台分析任务默认不进章节预览。
 * AI 面板手动点的「同步总谱」仍走预览 + 确认（userFacingStorySync=true）。
 */
export function shouldMuteLlmUi(task, userFacingStorySync = false) {
  const t = String(task || "");
  if (t === "llm_chat") return true;
  if (!isBackgroundAnalysisTask(task)) return false;
  if (isStorySyncTask(task) && userFacingStorySync) return false;
  return true;
}

export function isProseWritingTask(task) {
  const t = String(task || "");
  return t && !isBackgroundAnalysisTask(t);
}
