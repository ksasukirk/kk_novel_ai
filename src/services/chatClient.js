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
import { t, tLocale } from "../i18n/index.js";

const MAX_TURNS = 40;
const OUTLINE_CHARS = 2000;
const BODY_CHARS = 4000;
const STYLE_CHARS = 800;
const PERSONA_CHARS = 4000;

let listening = false;
let unlistenChunk = null;
let unlistenStart = null;
let unlistenDone = null;
let unlistenErr = null;

function sessionKey(mode, root) {
  return `${mode}::${mode === "novel" ? root || "" : "app"}`;
}

function clip(s, n) {
  const text = String(s || "").trim();
  if (text.length <= n) return text;
  return `${text.slice(0, n)}\n${t("chat.truncated")}`;
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
    chatState.error = String(p.error || t("chat.failed"));
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
  chatState.assistantName = String(sess.assistant_name || "");
  chatState.assistantStyle = String(sess.assistant_style || "");
  chatState.assistantPersona = String(sess.assistant_persona || "");
  chatState.loadedKey = key;
  chatState.error = "";
}

export async function saveChatSession() {
  const m = chatState.mode === "novel" ? "novel" : "free";
  const root = m === "novel" ? appState.projectRoot || "" : "";
  await invoke("chat_session_save", {
    mode: m,
    root: m === "novel" ? root || null : null,
    session: sessionPayload(m),
  });
}

function sessionPayload(mode) {
  return {
    mode,
    messages: chatState.messages.filter(
      (x) => x && (x.role === "user" || x.role === "assistant") && String(x.content || "").trim()
    ),
    assistant_name: String(chatState.assistantName || ""),
    assistant_style: String(chatState.assistantStyle || ""),
    assistant_persona: String(chatState.assistantPersona || ""),
  };
}

/** 只落盘人设（可在尚无消息时调用） */
export async function saveChatPersona() {
  await saveChatSession();
}

function writingT(key, values) {
  const loc = (appState.settings && appState.settings.writing_locale) || "zh-CN";
  return tLocale(loc, key, values);
}

export function assistantLabel() {
  return String(chatState.assistantName || "").trim() || t("chat.assistant");
}

function personaPromptLines() {
  const lines = [];
  const name = String(chatState.assistantName || "").trim();
  const style = String(chatState.assistantStyle || "").trim();
  const persona = String(chatState.assistantPersona || "").trim();
  if (name) lines.push(writingT("writingSys.callName", { name }));
  if (style) lines.push(writingT("writingSys.style", { style: clip(style, STYLE_CHARS) }));
  if (persona) lines.push(writingT("writingSys.persona", { persona: clip(persona, PERSONA_CHARS) }));
  return lines;
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
    ...personaPromptLines(),
    writingT("writingSys.novel"),
    writingT("writingSys.book", { title: p.title || writingT("writingSys.unnamed") }),
    writingT("writingSys.outline", {
      text: clip(p.book_outline || writingT("writingSys.unnamed"), OUTLINE_CHARS),
    }),
  ];
  if (ch) {
    lines.push(writingT("writingSys.chapter", { title: ch.title || "" }));
    if (ch.summary) lines.push(writingT("writingSys.chOutline", { text: clip(ch.summary, 800) }));
  }
  if (names.length) lines.push(writingT("writingSys.cast", { names: names.join("、") }));
  if (body) lines.push(writingT("writingSys.body", { text: body }));
  return lines.join("\n");
}

function freeSystemPrompt() {
  const lines = [...personaPromptLines(), writingT("writingSys.free")];
  return lines.join("\n");
}

export async function sendChat(text) {
  const content = String(text || "").trim();
  if (!content) throw new Error(t("chat.needInput"));
  if (chatState.busy) throw new Error(t("chat.busy"));
  const mode = chatState.mode === "novel" ? "novel" : "free";
  if (mode === "novel" && !appState.projectRoot) {
    throw new Error(t("chat.needOpen"));
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
