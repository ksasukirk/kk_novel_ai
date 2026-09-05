/**
 * 对话会话：读写落盘 + 拼系统提示 + 流式 chat
 * 代码路径: kk_novel_ai/src/services/chatClient.js
 */
import { invoke, listen } from "./tauri.js";
import { appState } from "../stores/appState.js";
import { chatState } from "../stores/chatState.js";
import { peekChapterBlocks } from "./projectClient.js";
import { contentFromBlocks } from "../utils/genBlock.js";
import { cancelGeneration } from "./llmClient.js";

const MAX_TURNS = 40;
const OUTLINE_CHARS = 2000;
const BODY_CHARS = 4000;

let listening = false;
let unlistenChunk = null;
let unlistenStart = null;
let unlistenDone = null;
let unlistenErr = null;

function sessionKey(mode, root) {
  return `${mode}::${mode === "novel" ? root || "" : "app"}`;
}

function clip(s, n) {
  const t = String(s || "").trim();
  if (t.length <= n) return t;
  return `${t.slice(0, n)}\n…（已截断）`;
}

export async function ensureChatListeners() {
  if (listening) return;
  listening = true;
  unlistenStart = await listen("llm-start", (event) => {
    const p = event.payload || {};
    if (p.task !== "llm_chat") return;
    if (p.request_id) chatState.requestId = String(p.request_id);
  });
  unlistenChunk = await listen("llm-chunk", (event) => {
    const p = event.payload || {};
    if (p.task !== "llm_chat") return;
    if (p.request_id && chatState.requestId && p.request_id !== chatState.requestId) return;
    if (p.request_id) chatState.requestId = String(p.request_id);
    const delta = String(p.delta || "");
    if (!delta) return;
    const last = chatState.messages[chatState.messages.length - 1];
    if (!last || last.role !== "assistant") {
      chatState.messages.push({ role: "assistant", content: delta });
    } else {
      last.content = String(last.content || "") + delta;
    }
  });
  unlistenDone = await listen("llm-done", (event) => {
    const p = event.payload || {};
    if (p.task !== "llm_chat") return;
    const last = chatState.messages[chatState.messages.length - 1];
    if (last && last.role === "assistant" && p.text) {
      last.content = String(p.text);
    }
  });
  unlistenErr = await listen("llm-error", (event) => {
    const p = event.payload || {};
    if (p.task !== "llm_chat") return;
    chatState.error = String(p.error || "对话失败");
  });
  void unlistenChunk;
  void unlistenStart;
  void unlistenDone;
  void unlistenErr;
}

export async function loadChatSession(mode) {
  const m = mode === "novel" ? "novel" : "free";
  const root = m === "novel" ? appState.projectRoot || "" : "";
  const key = sessionKey(m, root);
  if (chatState.loadedKey === key && chatState.mode === m) return;
  const r = await invoke("chat_session_get", {
    mode: m,
    root: m === "novel" ? root || null : null,
  });
  const sess = (r && r.session) || {};
  chatState.mode = m;
  chatState.messages = Array.isArray(sess.messages)
    ? sess.messages.map((x) => ({
        role: String(x.role || ""),
        content: String(x.content || ""),
      }))
    : [];
  chatState.loadedKey = key;
  chatState.error = "";
}

export async function saveChatSession() {
  const m = chatState.mode === "novel" ? "novel" : "free";
  const root = m === "novel" ? appState.projectRoot || "" : "";
  await invoke("chat_session_save", {
    mode: m,
    root: m === "novel" ? root || null : null,
    session: {
      mode: m,
      messages: chatState.messages.filter(
        (x) => x && (x.role === "user" || x.role === "assistant") && String(x.content || "").trim()
      ),
    },
  });
}

async function novelSystemPrompt() {
  const p = appState.project || {};
  const ch = ((p.chapters || []).find((c) => c.id === appState.chapterId)) || null;
  const names = (appState.characterList || [])
    .filter((e) => e && (!e.kind || e.kind === "character"))
    .map((e) => e.title)
    .filter(Boolean)
    .slice(0, 40);
  let body = "";
  if (chatState.includeChapterBody && appState.chapterId) {
    try {
      const blocks = await peekChapterBlocks(appState.chapterId);
      body = clip(contentFromBlocks(blocks), BODY_CHARS);
    } catch {
      body = clip(appState.chapterContent || "", BODY_CHARS);
    }
  }
  const lines = [
    "你是小说创作助手，用中文对话。只讨论、建议、分析；不要输出要直接落盘的章节补丁，也不要假装已经改了正文。",
    `书名：${p.title || "未命名"}`,
    `全书大纲摘要：${clip(p.book_outline || "（空）", OUTLINE_CHARS)}`,
  ];
  if (ch) {
    lines.push(`当前章：${ch.title || ""}`);
    if (ch.summary) lines.push(`章纲：${clip(ch.summary, 800)}`);
  }
  if (names.length) lines.push(`角色：${names.join("、")}`);
  if (body) lines.push(`本章正文（截断）：\n${body}`);
  return lines.join("\n");
}

function freeSystemPrompt() {
  return "你是通用助手，用中文对话。这不是写作引擎：不要改用户作品文件，不要输出落盘用的章节 JSON。";
}

export async function sendChat(text) {
  const content = String(text || "").trim();
  if (!content) throw new Error("请先输入内容");
  if (chatState.busy) throw new Error("上一句还在生成");
  const mode = chatState.mode === "novel" ? "novel" : "free";
  if (mode === "novel" && !appState.projectRoot) {
    throw new Error("本作对话需要先打开作品");
  }
  await ensureChatListeners();
  chatState.error = "";
  chatState.busy = true;
  chatState.messages.push({ role: "user", content });
  chatState.messages.push({ role: "assistant", content: "" });
  chatState.draft = "";
  try {
    const system =
      mode === "novel" ? await novelSystemPrompt() : freeSystemPrompt();
    const history = chatState.messages
      .slice(0, -1)
      .filter((m) => m.role === "user" || (m.role === "assistant" && String(m.content || "").trim()))
      .slice(-MAX_TURNS)
      .map((m) => ({ role: m.role, content: String(m.content || "") }));
    const messages = [{ role: "system", content: system }, ...history];
    await invoke("llm_chat_stream", {
      messages,
      options: { stream: true },
    });
    await saveChatSession();
  } catch (e) {
    const last = chatState.messages[chatState.messages.length - 1];
    if (last && last.role === "assistant" && !String(last.content || "").trim()) {
      chatState.messages.pop();
    } else {
      try {
        await saveChatSession();
      } catch {
        /* ignore */
      }
    }
    throw e;
  } finally {
    chatState.busy = false;
    chatState.requestId = "";
  }
}

export async function cancelChat() {
  const rid = chatState.requestId;
  if (rid) {
    try {
      await cancelGeneration(rid);
    } catch {
      /* ignore */
    }
  }
}

export async function newChatSession() {
  chatState.messages = [];
  chatState.error = "";
  await saveChatSession();
}

export async function switchChatMode(next) {
  const mode = next === "novel" ? "novel" : "free";
  if (mode === chatState.mode && chatState.loadedKey) return;
  if (chatState.loadedKey) {
    try {
      await saveChatSession();
    } catch {
      /* ignore */
    }
  }
  chatState.loadedKey = "";
  await loadChatSession(mode);
}
