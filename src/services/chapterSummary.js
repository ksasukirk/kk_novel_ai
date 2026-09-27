/**
 * 写作页「AI全篇总结」：与作品页「重新总结」同一条链路
 * （情节扫描 + 按正文重建章纲/生成块）
 * 代码路径: kk_novel_ai/src/services/chapterSummary.js
 */
import { appState } from "../stores/appState.js";
import { saveChapter } from "./projectClient.js";
import { isTropeScanBusy, scanTropesFromRoot } from "./tropeScan.js";
import { t } from "../i18n/index.js";

export function isChapterSummaryBusy() {
  return isTropeScanBusy();
}

/**
 * @param {{ root?: string }} [opts]
 * @returns {Promise<object|null>}
 */
export async function runChapterAiSummary(opts = {}) {
  const root = String(opts.root || appState.projectRoot || "").trim();
  if (!root) {
    throw new Error(t("editor.needProjectChapter"));
  }
  if (isTropeScanBusy()) {
    throw new Error(t("editor.summaryBusy"));
  }
  if (appState.dirty && appState.projectRoot === root) {
    try {
      await saveChapter();
    } catch {
      /* 保存失败仍尝试总结，后端读盘 */
    }
  }
  const r = await scanTropesFromRoot(root);
  if (!r) {
    throw new Error(t("editor.summaryBusy"));
  }
  return r;
}
