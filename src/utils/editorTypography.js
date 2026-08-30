/**
 * 编辑器字体 / 字号
 * 代码路径: kk_novel_ai/src/utils/editorTypography.js
 */

/** 默认黑体 CSS 栈 */
export const DEFAULT_EDITOR_FONT_CSS =
  'SimHei, "黑体", "Heiti SC", STHeiti, "Microsoft YaHei", sans-serif';

export const DEFAULT_EDITOR_FONT_SIZE = 16;

/** 预设：id 写入 settings.editor_font_family */
export const EDITOR_FONT_PRESETS = [
  {
    id: "heiti",
    label: "黑体",
    css: DEFAULT_EDITOR_FONT_CSS,
  },
  {
    id: "yahei",
    label: "微软雅黑",
    css: '"Microsoft YaHei", "PingFang SC", "Noto Sans SC", sans-serif',
  },
  {
    id: "songti",
    label: "宋体",
    css: 'SimSun, "宋体", "Songti SC", STSong, serif',
  },
  {
    id: "kaiti",
    label: "楷体",
    css: 'KaiTi, "楷体", "Kaiti SC", STKaiti, serif',
  },
  {
    id: "fangsong",
    label: "仿宋",
    css: 'FangSong, "仿宋", STFangsong, serif',
  },
  {
    id: "mono",
    label: "等宽",
    css: '"Cascadia Code", Consolas, "Fira Code", monospace',
  },
];

export const EDITOR_FONT_SIZES = [12, 14, 16, 18, 20, 22, 24, 28];

export function resolveEditorFontCss(familyIdOrCss) {
  const raw = (familyIdOrCss || "").trim();
  if (!raw) return DEFAULT_EDITOR_FONT_CSS;
  const preset = EDITOR_FONT_PRESETS.find((p) => p.id === raw || p.label === raw);
  if (preset) return preset.css;
  // 已是 CSS 栈或系统字体名
  return raw;
}

export function resolveEditorFontSize(size) {
  const n = Number(size);
  if (!Number.isFinite(n) || n < 10 || n > 48) return DEFAULT_EDITOR_FONT_SIZE;
  return Math.round(n);
}

/** 把当前设置写到 :root CSS 变量，供写作区使用 */
export function applyEditorTypography(settings) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  const family = resolveEditorFontCss(settings && settings.editor_font_family);
  const size = resolveEditorFontSize(settings && settings.editor_font_size);
  root.style.setProperty("--editor-font-family", family);
  root.style.setProperty("--editor-font-size", `${size}px`);
}

export function presetIdFromSettings(settings) {
  const raw = ((settings && settings.editor_font_family) || "heiti").trim();
  const byId = EDITOR_FONT_PRESETS.find((p) => p.id === raw);
  if (byId) return byId.id;
  const byLabel = EDITOR_FONT_PRESETS.find((p) => p.label === raw);
  if (byLabel) return byLabel.id;
  const byCss = EDITOR_FONT_PRESETS.find((p) => p.css === raw);
  if (byCss) return byCss.id;
  return "heiti";
}
