/**
 * 情节/喜好卡中英显示：界面跟 ui_locale，缺英文回退中文
 * 代码路径: kk_novel_ai/src/utils/tropeI18n.js
 */
import { appState } from "../stores/appState.js";
import { normalizeLocale } from "../i18n/index.js";

export function isUiEnglish() {
  return normalizeLocale(appState.settings && appState.settings.ui_locale) === "en";
}

export function displayTropeTitle(item) {
  if (isUiEnglish()) {
    const en = String((item && item.attrs && item.attrs.title_en) || "").trim();
    if (en) return en;
  }
  return String((item && item.title) || "").trim();
}

export function displayTropeContent(item) {
  if (isUiEnglish()) {
    const en = String((item && item.attrs && item.attrs.content_en) || "").trim();
    if (en) return en;
  }
  return String((item && item.content) || "").trim();
}
