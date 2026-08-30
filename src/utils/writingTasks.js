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
    t === "chapter_summary" ||
    t === "consistency_check"
  );
}

export function isProseWritingTask(task) {
  const t = String(task || "");
  return t && !isBackgroundAnalysisTask(t);
}
