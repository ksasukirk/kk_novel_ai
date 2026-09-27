/**
 * 导入写作小说：单文件/多文件 + 可选导入后 AI 总结
 * 代码路径: kk_novel_ai/src/services/novelImportFlow.js
 */
import { appState } from "../stores/appState.js";
import { loadSettings, saveSettings } from "./llmClient.js";
import * as project from "./projectClient.js";
import { scanTropesFromRoot, scanTropesQueue, isTropeScanBusy } from "./tropeScan.js";
import { t } from "../i18n/index.js";

const NOVEL_EXTS = new Set([".txt", ".md"]);

/** @param {string} path */
export function isNovelImportPath(path) {
  const p = String(path || "").trim().replace(/\\/g, "/");
  const i = p.lastIndexOf(".");
  if (i < 0) return false;
  return NOVEL_EXTS.has(p.slice(i).toLowerCase());
}

/** @param {string[]} paths */
export function filterNovelImportPaths(paths) {
  const out = [];
  const seen = new Set();
  for (const raw of paths || []) {
    const p = String(raw || "").trim();
    if (!p || !isNovelImportPath(p) || seen.has(p.toLowerCase())) continue;
    seen.add(p.toLowerCase());
    out.push(p);
  }
  return out;
}

export function importAutoSummaryEnabled(settings = appState.settings) {
  if (!settings) return true;
  return settings.import_auto_trope_summary !== false;
}

/** 把导入对话框勾选写回全局设置 */
export async function persistImportAutoSummary(enabled) {
  const on = !!enabled;
  if (!appState.settings) return on;
  if (appState.settings.import_auto_trope_summary === on) return on;
  await saveSettings({
    ...appState.settings,
    import_auto_trope_summary: on,
  });
  return on;
}

/**
 * @param {string} filePath
 * @param {{ title?: string, translateTitles?: boolean, translateLocale?: string, autoSummary?: boolean, openAfter?: boolean }} [opts]
 */
export async function importNovelFile(filePath, opts = {}) {
  const file = String(filePath || "").trim();
  if (!file) throw new Error(t("project.importNovelNeedFile"));
  const autoSummary = opts.autoSummary != null ? !!opts.autoSummary : importAutoSummaryEnabled();
  const r = await project.importNovelTxt(file, opts.title || "", {
    translateTitles: !!opts.translateTitles,
    translateLocale: opts.translateLocale || "",
  });
  try {
    await loadSettings();
  } catch {
    /* ignore */
  }
  const root = r && r.root ? String(r.root) : "";
  if (root && opts.openAfter !== false) {
    await project.openProject(root);
    appState.activeNav = "editor";
    if (appState.chapterId) {
      try {
        await project.loadChapter(appState.chapterId);
      } catch {
        /* ignore */
      }
    }
  }
  if (root && autoSummary && !isTropeScanBusy()) {
    appState.statusMessage = t("project.importNovelSummarizing");
    try {
      await scanTropesFromRoot(root);
    } catch (e) {
      appState.statusMessage = t("project.importNovelSummaryFail", {
        msg: e && e.message ? e.message : String(e),
      });
    }
  }
  return r;
}

/**
 * 批量导入（拖放多文件）；最后一本 openAfter；可选排队总结
 * @param {string[]} filePaths
 * @param {{ autoSummary?: boolean, openLast?: boolean, translateTitles?: boolean, translateLocale?: string }} [opts]
 */
export async function importNovelFiles(filePaths, opts = {}) {
  const files = filterNovelImportPaths(filePaths);
  if (!files.length) throw new Error(t("project.importNovelDropNone"));
  const autoSummary = opts.autoSummary != null ? !!opts.autoSummary : importAutoSummaryEnabled();
  const openLast = opts.openLast !== false;
  const imported = [];
  const failed = [];
  for (let i = 0; i < files.length; i++) {
    const file = files[i];
    const isLast = i === files.length - 1;
    try {
      appState.statusMessage = t("project.importNovelDropProgress", {
        current: i + 1,
        total: files.length,
      });
      const r = await project.importNovelTxt(file, "", {
        translateTitles: !!opts.translateTitles,
        translateLocale: opts.translateLocale || "",
      });
      const root = r && r.root ? String(r.root) : "";
      if (root) {
        imported.push({
          path: root,
          title: (r && r.title) || "",
          translate_error: !!(r && r.translate_attempted && r.translate_error),
        });
      }
      try {
        await loadSettings();
      } catch {
        /* ignore */
      }
      if (root && openLast && isLast) {
        await project.openProject(root);
        appState.activeNav = "editor";
        if (appState.chapterId) {
          try {
            await project.loadChapter(appState.chapterId);
          } catch {
            /* ignore */
          }
        }
      }
    } catch (e) {
      failed.push({ file, msg: e && e.message ? String(e.message) : String(e) });
    }
  }
  if (autoSummary && imported.length && !isTropeScanBusy()) {
    appState.statusMessage = t("project.importNovelSummarizing");
    try {
      if (imported.length === 1) {
        await scanTropesFromRoot(imported[0].path);
      } else {
        await scanTropesQueue(imported.map((it) => ({ path: it.path, title: it.title })));
      }
    } catch (e) {
      appState.statusMessage = t("project.importNovelSummaryFail", {
        msg: e && e.message ? e.message : String(e),
      });
    }
  } else if (imported.length) {
    appState.statusMessage = t("project.importNovelDropDone", {
      ok: imported.length,
      fail: failed.length,
    });
  }
  return { imported, failed, files };
}
