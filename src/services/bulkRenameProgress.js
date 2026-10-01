/**
 * 批量 / 一键 AI 重命名进度（供顶栏 GenProgressBar 读取）
 * 代码路径: kk_novel_ai/src/services/bulkRenameProgress.js
 */
import { reactive } from "vue";

export const bulkRenameState = reactive({
  running: false,
  current: 0,
  total: 0,
  title: "",
  pct: 0,
  abort: false,
});

export function resetBulkRenameProgress() {
  bulkRenameState.running = false;
  bulkRenameState.current = 0;
  bulkRenameState.total = 0;
  bulkRenameState.title = "";
  bulkRenameState.pct = 0;
  bulkRenameState.abort = false;
}

export function beginBulkRename(total) {
  bulkRenameState.running = true;
  bulkRenameState.current = 0;
  bulkRenameState.total = Math.max(0, Number(total) || 0);
  bulkRenameState.title = "";
  bulkRenameState.pct = 0;
  bulkRenameState.abort = false;
}

export function tickBulkRename(current, total, title) {
  const t = Math.max(0, Number(total) || 0);
  const c = Math.max(0, Number(current) || 0);
  bulkRenameState.current = c;
  bulkRenameState.total = t;
  bulkRenameState.title = String(title || "");
  bulkRenameState.pct = t > 0 ? Math.min(100, Math.round((c / t) * 100)) : 0;
}

export function requestBulkRenameAbort() {
  bulkRenameState.abort = true;
}

export function endBulkRename() {
  bulkRenameState.running = false;
  bulkRenameState.abort = false;
  bulkRenameState.pct = bulkRenameState.total > 0 ? 100 : 0;
}
