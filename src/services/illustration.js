/**
 * 配图：提示词任务 + 图像 API + 插入章节插图块
 * 代码路径: kk_novel_ai/src/services/illustration.js
 */
import { reactive } from "vue";
import { invoke } from "./tauri.js";
import { appState } from "../stores/appState.js";
import { saveChapter } from "./projectClient.js";
import {
  createIllustrationBlock,
  cryptoRandomId,
  isIllustrationBlock,
} from "../utils/genBlock.js";
import {
  activePathBlocks,
  insertIllustrationAfterGen,
  removeInlineByKey,
} from "../utils/branchModel.js";
import { formatSheetsForPrompt } from "../utils/loreVisual.js";
import * as story from "./storyClient.js";

export const imageGenState = reactive({
  busy: false,
  message: "",
});

export const imagePromptDialog = reactive({
  open: false,
  title: "配图",
  prompt: "",
  negative: "",
  caption: "",
  busy: false,
  error: "",
  _resolve: null,
});

const dataUrlCache = new Map();

export function parseJsonObject(text) {
  const v = parseJsonValue(text);
  if (!v || typeof v !== "object" || Array.isArray(v)) {
    throw new Error("模型未返回 JSON 对象");
  }
  return v;
}

export function parseJsonValue(text) {
  let s = String(text || "").trim();
  const fence = s.match(/```(?:json)?\s*([\s\S]*?)```/i);
  if (fence) s = fence[1].trim();
  const objStart = s.indexOf("{");
  const arrStart = s.indexOf("[");
  const useArr =
    arrStart >= 0 && (objStart < 0 || arrStart < objStart);
  if (useArr) {
    const end = s.lastIndexOf("]");
    if (end > arrStart) s = s.slice(arrStart, end + 1);
  } else if (objStart >= 0) {
    const end = s.lastIndexOf("}");
    if (end > objStart) s = s.slice(objStart, end + 1);
  }
  return JSON.parse(s);
}

export async function hashSourceText(s) {
  const enc = new TextEncoder().encode(String(s || ""));
  if (globalThis.crypto?.subtle) {
    const buf = await crypto.subtle.digest("SHA-256", enc);
    return [...new Uint8Array(buf)]
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("")
      .slice(0, 16);
  }
  let h = 2166136261;
  for (const ch of String(s)) {
    h ^= ch.charCodeAt(0);
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0).toString(16);
}

export function illustrationRel(chapterId, id) {
  return `assets/illustrations/${chapterId}/${id}.png`;
}

export async function loadIllustrationDataUrl(rel) {
  if (!rel || !appState.projectRoot) return "";
  const key = `${appState.projectRoot}::${rel}`;
  if (dataUrlCache.has(key)) return dataUrlCache.get(key);
  try {
    const r = await invoke("image_read_data_url", {
      root: appState.projectRoot,
      rel,
    });
    const url = r && r.data_url ? String(r.data_url) : "";
    if (url) dataUrlCache.set(key, url);
    return url;
  } catch {
    return "";
  }
}

export function forgetIllustrationCache(rel) {
  if (!rel) {
    dataUrlCache.clear();
    return;
  }
  const key = `${appState.projectRoot}::${rel}`;
  dataUrlCache.delete(key);
}

export function openImagePromptDialog(seed = {}) {
  return new Promise((resolve) => {
    imagePromptDialog.open = true;
    imagePromptDialog.title = seed.title || "配图";
    imagePromptDialog.prompt = String(seed.prompt || "");
    imagePromptDialog.negative = String(seed.negative || "");
    imagePromptDialog.caption = String(seed.caption || "");
    imagePromptDialog.busy = false;
    imagePromptDialog.error = "";
    imagePromptDialog._resolve = resolve;
  });
}

export function closeImagePromptDialog(result) {
  const fn = imagePromptDialog._resolve;
  imagePromptDialog._resolve = null;
  imagePromptDialog.open = false;
  imagePromptDialog.busy = false;
  if (fn) fn(result || null);
}

export async function runContentToImagePrompt({
  selection,
  instruction,
  chapterId,
} = {}) {
  const cid = chapterId || appState.chapterId;
  const result = await invoke("writing_run", {
    request: {
      project_root: appState.projectRoot,
      chapter_id: cid,
      task: "content_to_image_prompt",
      selection: String(selection || ""),
      instruction: String(instruction || ""),
    },
  });
  const raw = String((result && (result.text || result.raw_text)) || "");
  const obj = parseJsonObject(raw);
  return {
    prompt: String(obj.prompt || "").trim(),
    negative: String(obj.negative || "").trim(),
    caption: String(obj.caption || "").trim(),
  };
}

export async function runBeatsToStoryboard({ instruction, chapterId } = {}) {
  const cid = chapterId || appState.chapterId;
  const ch = ((appState.project && appState.project.chapters) || []).find(
    (c) => c.id === cid
  );
  const result = await invoke("writing_run", {
    request: {
      project_root: appState.projectRoot,
      chapter_id: cid,
      task: "beats_to_storyboard",
      selection: String((ch && ch.summary) || ""),
      instruction: String(instruction || ""),
    },
  });
  const raw = String((result && (result.text || result.raw_text)) || "");
  const obj = parseJsonValue(raw);
  if (Array.isArray(obj)) return obj;
  return obj && Array.isArray(obj.shots) ? obj.shots : [];
}

export async function generateImageFile({ rel, prompt, negative, size } = {}) {
  if (imageGenState.busy) {
    throw new Error("正在生成另一张图，请稍候");
  }
  imageGenState.busy = true;
  imageGenState.message = "正在出图…";
  try {
    const r = await invoke("image_generate", {
      request: {
        project_root: appState.projectRoot,
        rel,
        prompt: String(prompt || ""),
        negative: String(negative || ""),
        size: size || undefined,
      },
    });
    forgetIllustrationCache(rel);
    return r;
  } finally {
    imageGenState.busy = false;
    imageGenState.message = "";
  }
}

async function loadCharacterLore() {
  try {
    const { listLoreScoped } = await import("./projectClient.js");
    const scoped = await listLoreScoped();
    const local = (scoped.local || []).map((row) => row.entry).filter(Boolean);
    const global = (scoped.global || []).map((row) => row.entry).filter(Boolean);
    const byId = new Map();
    for (const e of [...global, ...local]) {
      if (e && (e.kind === "character" || !e.kind)) byId.set(e.id, e);
    }
    return [...byId.values()];
  } catch {
    return [];
  }
}

function sheetsMatchingText(entries, text) {
  const all = (entries || []).filter((e) => e && (!e.kind || e.kind === "character"));
  const body = String(text || "");
  const hit = all.filter((e) => {
    const title = String(e.title || "").trim();
    return title && body.includes(title);
  });
  return hit.length ? hit : all;
}

function boardStyleInstruction(board, extraSheets) {
  const parts = [];
  const style = String((board && board.style_prefix) || "").trim();
  const neg = String((board && board.negative) || "").trim();
  if (style) parts.push(`【画风】\n${style}`);
  if (neg) parts.push(`【默认负向】\n${neg}`);
  const sheets = String(extraSheets || "").trim();
  if (sheets) parts.push(`【形象卡】\n${sheets}`);
  return parts.join("\n\n");
}

export async function promptFromBlock(block, loreItems, board) {
  const text = String((block && (block.digest || block.text)) || "").trim();
  const instruction = boardStyleInstruction(
    board,
    formatSheetsForPrompt(loreItems)
  );
  return await runContentToImagePrompt({
    selection: text,
    instruction,
  });
}

export async function promptFromShot(shot, loreItems, board) {
  const names = (loreItems || []).filter((e) =>
    (shot.character_lore_ids || []).includes(e.id)
  );
  const scene = [
    shot.visual || "",
    shot.location ? `地点：${shot.location}` : "",
    shot.mood ? `氛围：${shot.mood}` : "",
    shot.dialogue ? `对白：${shot.dialogue}` : "",
  ]
    .filter(Boolean)
    .join("\n");
  return await runContentToImagePrompt({
    selection: scene,
    instruction: boardStyleInstruction(board, formatSheetsForPrompt(names)),
  });
}

export async function persistIllustrationAfterGen(genBlockKey, illus) {
  const doc = appState.chapterBranchDoc;
  appState.chapterBranchDoc = insertIllustrationAfterGen(
    doc,
    genBlockKey,
    illus
  );
  appState.chapterBlocks = activePathBlocks(appState.chapterBranchDoc);
  await saveChapter();
}

export async function deleteIllustrationBlock(blockKey) {
  if (!blockKey) return false;
  appState.chapterBranchDoc = removeInlineByKey(
    appState.chapterBranchDoc,
    blockKey
  );
  appState.chapterBlocks = activePathBlocks(appState.chapterBranchDoc);
  await saveChapter();
  return true;
}

export async function isIllustrationStale(block, sourceText) {
  const expect = String((block && block.source_hash) || "");
  if (!expect) return false;
  const h = await hashSourceText(sourceText);
  return h !== expect;
}

/**
 * 编辑器：根据 gen 块生成提示词 → 确认 → 出图 → 插入
 */
export async function illustrateGenBlock(block, loreItems) {
  if (!block || block.type !== "gen") throw new Error("只能给生成段配图");
  if (!appState.projectRoot || !appState.chapterId) {
    throw new Error("请先打开作品和章节");
  }
  let board = { style_prefix: "", negative: "" };
  try {
    const r = await story.getStoryboard();
    board = r.storyboard || board;
  } catch {
    /* 无分镜文件 */
  }
  const sourceText = String(block.text || "");
  const lore =
    loreItems && loreItems.length ? loreItems : await loadCharacterLore();
  const drafted = await runContentToImagePrompt({
    selection: sourceText,
    instruction: boardStyleInstruction(
      board,
      formatSheetsForPrompt(sheetsMatchingText(lore, sourceText))
    ),
  });
  const confirmed = await openImagePromptDialog({
    title: "本段配图",
    prompt: drafted.prompt,
    negative: drafted.negative || board.negative || "",
    caption: drafted.caption,
  });
  if (!confirmed) return null;
  const id = cryptoRandomId();
  const rel = illustrationRel(appState.chapterId, id);
  const gen = await generateImageFile({
    rel,
    prompt: confirmed.prompt,
    negative: confirmed.negative,
  });
  const illus = createIllustrationBlock({
    id,
    caption: confirmed.caption,
    rel: gen.rel || rel,
    prompt: confirmed.prompt,
    negative: confirmed.negative,
    model: (gen && gen.model) || "",
    source_hash: await hashSourceText(sourceText),
    source: { kind: "block", block_key: block.key, shot_id: "" },
  });
  await persistIllustrationAfterGen(block.key, illus);
  return illus;
}

export async function regenerateIllustration(block, sourceText) {
  if (!isIllustrationBlock(block)) throw new Error("不是插图块");
  const confirmed = await openImagePromptDialog({
    title: "重生成插图",
    prompt: block.prompt || "",
    negative: block.negative || "",
    caption: block.caption || "",
  });
  if (!confirmed) return null;
  const rel =
    block.rel || illustrationRel(appState.chapterId, block.id || cryptoRandomId());
  const gen = await generateImageFile({
    rel,
    prompt: confirmed.prompt,
    negative: confirmed.negative,
  });
  const next = {
    ...block,
    caption: confirmed.caption,
    rel: gen.rel || rel,
    prompt: confirmed.prompt,
    negative: confirmed.negative,
    model: (gen && gen.model) || block.model || "",
    source_hash: await hashSourceText(sourceText || block.prompt || ""),
  };
  const list = (appState.chapterBlocks || []).map((b) =>
    b.key === block.key ? next : b
  );
  appState.chapterBlocks = list;
  const { syncBranchDocFromEditor } = await import("./projectClient.js");
  syncBranchDocFromEditor();
  await saveChapter();
  forgetIllustrationCache(rel);
  return next;
}
