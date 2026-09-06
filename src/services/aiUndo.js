/**
 * AI 写入 Undo 栈（内存）+ 可选落盘 history
 * 代码路径: kk_novel_ai/src/services/aiUndo.js
 */
import { appState } from "../stores/appState.js";
import { invoke } from "./tauri.js";
import { t } from "../i18n/index.js";
import { blocksFromContent } from "../utils/genBlock.js";
import { applyBranchDoc, syncBranchDocFromEditor } from "./projectClient.js";
import { migrateBlocksToBranchDoc } from "../utils/branchModel.js";

const MAX = 20;

/** 在改写章节前调用：压入快照 */
export async function pushAiUndo(label = t("draft.undoWrite")) {
  syncBranchDocFromEditor();
  const snapshot = {
    label,
    chapterId: appState.chapterId,
    content: appState.chapterContent || "",
    blocks: Array.isArray(appState.chapterBlocks)
      ? appState.chapterBlocks.map((b) => ({ ...b }))
      : [],
    branchDoc: appState.chapterBranchDoc
      ? JSON.parse(JSON.stringify(appState.chapterBranchDoc))
      : null,
    at: Date.now(),
  };
  if (!Array.isArray(appState.aiUndoStack)) appState.aiUndoStack = [];
  appState.aiUndoStack.push(snapshot);
  if (appState.aiUndoStack.length > MAX) appState.aiUndoStack.shift();
  if (appState.projectRoot && appState.chapterId) {
    try {
      await invoke("chapter_push_history", {
        root: appState.projectRoot,
        chapterId: appState.chapterId,
        content: snapshot.content,
      });
    } catch {
      /* ignore */
    }
  }
}

export function undoLastAi() {
  if (!appState.aiUndoStack || !appState.aiUndoStack.length) {
    appState.statusMessage = t("draft.noUndo");
    return false;
  }
  const snap = appState.aiUndoStack.pop();
  if (snap.chapterId && snap.chapterId === appState.chapterId) {
    if (snap.branchDoc) {
      applyBranchDoc(snap.branchDoc);
    } else if (Array.isArray(snap.blocks) && snap.blocks.length) {
      applyBranchDoc(migrateBlocksToBranchDoc(snap.blocks));
    } else {
      applyBranchDoc(
        migrateBlocksToBranchDoc(blocksFromContent(snap.content || ""))
      );
    }
    appState.dirty = true;
    appState.statusMessage = t("draft.undone", { msg: snap.label });
    return true;
  }
  appState.statusMessage = t("draft.undoMismatch");
  return false;
}
