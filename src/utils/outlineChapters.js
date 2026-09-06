/**
 * 拆章 JSON 容错与失败说明
 * 代码路径: kk_novel_ai/src/utils/outlineChapters.js
 */
import { t } from "../i18n/index.js";
import { parseLlmJson, salvageChapterObjects } from "./llmJson.js";

const PREVIEW_CHARS = 100;
const SPLIT_MAX_TOKENS_CAP = 16384;
const SPLIT_MAX_TOKENS_FLOOR = 4096;

/**
 * @param {string} text
 * @returns {string}
 */
export function takeSplitPreview(text) {
  const s = String(text || "")
    .replace(/\s+/g, " ")
    .trim();
  if (!s) return "";
  if (s.length <= PREVIEW_CHARS) return s;
  return `${s.slice(0, PREVIEW_CHARS)}…`;
}

/**
 * 长全书大纲需要更大的拆章输出预算（中文约 1.5 字/token）。
 * @param {string} outlineText
 * @returns {number}
 */
export function estimateSplitMaxTokens(outlineText) {
  const n = Array.from(String(outlineText || "").trim()).length;
  const promptTok = Math.ceil(n / 1.5);
  const outTok = Math.max(2048, Math.ceil(promptTok * 0.9));
  return Math.min(SPLIT_MAX_TOKENS_CAP, Math.max(SPLIT_MAX_TOKENS_FLOOR, outTok));
}

function mapChapterItem(item) {
  if (!item || typeof item !== "object" || Array.isArray(item)) return null;
  const title = String(item.title || item.Title || "").trim();
  const summary = String(item.summary || item.Summary || "").trim();
  const mustDo = String(item.must_do || item.mustDo || item.must_Do || "").trim();
  const mustNot = String(item.must_not || item.mustNot || item.must_Not || "").trim();
  if (!title && !summary) return null;
  return {
    title,
    summary,
    must_do: mustDo,
    must_not: mustNot,
    selected: true,
  };
}

function mapChapterData(data) {
  if (!data || typeof data !== "object") {
    return { chapters: [], reason: "" };
  }
  let list = [];
  let reason = "";
  if (Array.isArray(data)) {
    list = data;
  } else {
    reason = String(data.reason || "").trim();
    if (Array.isArray(data.chapters)) {
      list = data.chapters;
    } else if (data.title || data.summary) {
      list = [data];
    }
  }
  const chapters = [];
  for (const item of list) {
    const row = mapChapterItem(item);
    if (row) chapters.push(row);
  }
  return {
    chapters: chapters.slice(0, 30),
    reason,
  };
}

/**
 * @param {string} text
 * @returns {{ chapters: object[], reason: string, errorKind: string, parseError: string, preview: string }}
 */
export function parseOutlineToChapters(text) {
  const raw = String(text || "").trim();
  if (!raw) {
    return { chapters: [], reason: "", errorKind: "empty", parseError: "", preview: "" };
  }
  let parseError = "";
  let data = null;
  try {
    data = parseLlmJson(raw);
  } catch (e) {
    parseError = String((e && e.message) || e);
  }
  let mapped = mapChapterData(data);
  if (!mapped.chapters.length) {
    const salvaged = salvageChapterObjects(raw);
    mapped = mapChapterData({
      chapters: salvaged,
      reason: mapped.reason,
    });
  }
  if (mapped.chapters.length) {
    return {
      ...mapped,
      errorKind: "",
      parseError: "",
      preview: "",
    };
  }
  const hasJson = raw.includes("{") || raw.includes("[");
  let errorKind = "prose";
  if (parseError) errorKind = "json";
  else if (hasJson) errorKind = "no_chapters";
  return {
    chapters: [],
    reason: mapped.reason || "",
    errorKind,
    parseError,
    preview: takeSplitPreview(raw),
  };
}

/**
 * @param {"full"|"append"} mode
 * @param {string} planText
 * @param {{ errorKind?: string, parseError?: string, preview?: string }} parsed
 */
export function describeSplitFailure(mode, planText, parsed = {}) {
  const append = mode === "append";
  const raw = String(planText || "").trim();
  const preview = parsed.preview || takeSplitPreview(raw);
  const kind = parsed.errorKind || (raw ? "prose" : "empty");
  if (kind === "empty" || !raw) {
    return t(append ? "bookQ.noAppendEmpty" : "bookQ.noSplitEmpty");
  }
  const msg =
    parsed.parseError ||
    (kind === "no_chapters" ? t("bookQ.noSplitNoChapters") : t("bookQ.noSplitProse"));
  return t(append ? "bookQ.noAppendDetail" : "bookQ.noSplitDetail", { msg, preview });
}
