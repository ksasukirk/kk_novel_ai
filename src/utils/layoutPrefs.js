/**
 * 布局偏好（侧栏 / 写作页目录 / AI 面板）localStorage
 * 代码路径: kk_novel_ai/src/utils/layoutPrefs.js
 */

export const SIDEBAR_MODE_KEY = "kk_sidebar_mode";
export const EDITOR_TOC_VISIBLE_KEY = "kk_editor_toc_visible";
export const AI_PANEL_LAYOUT_KEY = "kk_ai_panel_layout";

/** @typedef {'expanded' | 'compact' | 'closed'} SidebarMode */
/** @typedef {'dock' | 'float' | 'hidden'} AiPanelLayout */

/** @returns {SidebarMode} */
export function readSidebarMode() {
  try {
    const v = localStorage.getItem(SIDEBAR_MODE_KEY);
    if (v === "compact" || v === "closed") return v;
  } catch {
    /* ignore */
  }
  return "expanded";
}

/** @param {SidebarMode} mode */
export function saveSidebarMode(mode) {
  try {
    localStorage.setItem(SIDEBAR_MODE_KEY, mode);
  } catch {
    /* ignore */
  }
}

/** @param {SidebarMode} mode @returns {SidebarMode} */
export function cycleSidebarMode(mode) {
  /** @type {Record<SidebarMode, SidebarMode>} */
  const next = { expanded: "compact", compact: "closed", closed: "expanded" };
  return next[mode] || "expanded";
}

/** @param {SidebarMode} mode */
export function sidebarToggleTitle(mode) {
  if (mode === "expanded") return "收为图标栏";
  if (mode === "compact") return "完全隐藏导航";
  return "展开导航";
}

export function readEditorTocVisible() {
  try {
    const v = localStorage.getItem(EDITOR_TOC_VISIBLE_KEY);
    if (v === "0" || v === "false") return false;
  } catch {
    /* ignore */
  }
  return true;
}

/** @param {boolean} visible */
export function saveEditorTocVisible(visible) {
  try {
    localStorage.setItem(EDITOR_TOC_VISIBLE_KEY, visible ? "1" : "0");
  } catch {
    /* ignore */
  }
}

/** @returns {AiPanelLayout} */
export function readAiPanelLayout() {
  try {
    const v = localStorage.getItem(AI_PANEL_LAYOUT_KEY);
    if (v === "dock" || v === "float" || v === "hidden") return v;
  } catch {
    /* ignore */
  }
  return "float";
}

/** @param {AiPanelLayout} layout */
export function saveAiPanelLayout(layout) {
  try {
    localStorage.setItem(AI_PANEL_LAYOUT_KEY, layout);
  } catch {
    /* ignore */
  }
}

/** @param {AiPanelLayout} layout @returns {AiPanelLayout} */
export function cycleAiPanelLayout(layout) {
  /** @type {Record<AiPanelLayout, AiPanelLayout>} */
  const next = { dock: "float", float: "hidden", hidden: "dock" };
  return next[layout] || "float";
}

/** @param {AiPanelLayout} layout */
export function aiPanelLayoutButtonLabel(layout) {
  if (layout === "dock") return "AI 浮条";
  if (layout === "float") return "隐藏 AI";
  return "显示 AI";
}
