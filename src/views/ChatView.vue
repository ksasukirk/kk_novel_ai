<!--
  独立 AI 对话：本作上下文 / 自由聊，不写入章节
  代码路径: kk_novel_ai/src/views/ChatView.vue
-->
<script setup>
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { appState } from "../stores/appState.js";
import { chatState } from "../stores/chatState.js";
import CapsuleSwitch from "../components/CapsuleSwitch.vue";
import { toastSuccess, useToastError } from "../services/toast.js";
import {
  assistantLabel,
  cancelChat,
  ensureChatListeners,
  loadChatSession,
  newChatSession,
  saveChatPersona,
  sendChat,
  switchChatMode,
} from "../services/chatClient.js";
import { t } from "../i18n/index.js";

defineOptions({ name: "ChatView" });

const error = useToastError();
const listEl = ref(null);
const showPersona = ref(false);
const personaBusy = ref(false);
const hasProject = computed(() => !!(appState.projectRoot && appState.project));
const nameLabel = computed(() => assistantLabel());
const novelMode = computed({
  get() {
    return chatState.mode === "novel";
  },
  set(v) {
    void onToggleNovel(!!v);
  },
});

async function onToggleNovel(on) {
  if (on && !hasProject.value) {
    error.value = t("chat.needNovel");
    return;
  }
  try {
    await switchChatMode(on ? "novel" : "free");
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function scrollBottom() {
  await nextTick();
  const el = listEl.value;
  if (el) el.scrollTop = el.scrollHeight;
}

async function onSend() {
  const text = String(chatState.draft || "").trim();
  if (!text || chatState.busy) return;
  try {
    await sendChat(text);
    await scrollBottom();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function onNew() {
  try {
    await newChatSession();
  } catch (e) {
    error.value = String(e.message || e);
  }
}

async function persistPersona(showToast) {
  if (personaBusy.value) return;
  if (!chatState.loadedKey) {
    error.value = t("chat.sessionLoading");
    return;
  }
  personaBusy.value = true;
  try {
    await saveChatPersona();
    if (showToast) toastSuccess(t("chat.personaSaved"));
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    personaBusy.value = false;
  }
}

function onKeydown(e) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    void onSend();
  }
}

watch(
  () => chatState.messages.map((m) => (m.content || "").length).join("|"),
  () => {
    void scrollBottom();
  }
);

watch(
  () => appState.projectRoot,
  () => {
    if (chatState.mode === "novel") {
      if (!appState.projectRoot) {
        void switchChatMode("free");
      } else {
        chatState.loadedKey = "";
        void loadChatSession("novel").catch((e) => {
          error.value = String(e.message || e);
        });
      }
    }
  }
);

onMounted(async () => {
  await ensureChatListeners();
  const mode = hasProject.value && chatState.mode === "novel" ? "novel" : "free";
  try {
    await loadChatSession(mode);
  } catch (e) {
    error.value = String(e.message || e);
  }
  await scrollBottom();
});
</script>

<template>
  <section class="panel chat-page">
    <div class="chat-head">
      <h1 class="panel-heading">{{ $t("chat.title") }}</h1>
      <p class="muted">
        {{ $t("chat.intro") }}
      </p>
      <div class="chat-tools">
        <CapsuleSwitch
          v-model="novelMode"
          :label="$t('chat.novelCtx')"
          :disabled="!hasProject"
        />
        <CapsuleSwitch
          v-model="chatState.includeChapterBody"
          :label="$t('chat.attachBody')"
          :disabled="!novelMode || !hasProject"
        />
        <button type="button" class="app-btn" :disabled="chatState.busy" @click="onNew">
          {{ $t("chat.newSession") }}
        </button>
        <button
          type="button"
          class="app-btn"
          :class="showPersona ? 'chip-active' : ''"
          @click="showPersona = !showPersona"
        >
          {{ showPersona ? $t("chat.hidePersona") : $t("chat.showPersona") }}
        </button>
      </div>
      <div v-show="showPersona" class="chat-persona">
        <p class="muted">{{ $t("chat.personaHint") }}</p>
        <div class="field">
          <label class="field-label">{{ $t("chat.name") }}</label>
          <input
            v-model="chatState.assistantName"
            type="text"
            maxlength="40"
            :placeholder="$t('chat.namePh')"
            @change="persistPersona(false)"
          />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("chat.style") }}</label>
          <input
            v-model="chatState.assistantStyle"
            type="text"
            :placeholder="$t('chat.stylePh')"
            @change="persistPersona(false)"
          />
        </div>
        <div class="field">
          <label class="field-label">{{ $t("chat.role") }}</label>
          <textarea
            v-model="chatState.assistantPersona"
            rows="4"
            :placeholder="$t('chat.rolePh')"
            @change="persistPersona(false)"
          />
        </div>
        <button
          type="button"
          class="app-btn app-btn-primary"
          :disabled="personaBusy"
          @click="persistPersona(true)"
        >
          {{ $t("chat.savePersona") }}
        </button>
      </div>
      <p v-if="!hasProject" class="muted">{{ $t("chat.freeOnly") }}</p>
    </div>

    <div ref="listEl" class="chat-list" aria-live="polite">
      <p v-if="!chatState.messages.length" class="muted chat-empty">{{ $t("chat.empty") }}</p>
      <div
        v-for="(m, i) in chatState.messages"
        :key="i"
        class="chat-bubble"
        :class="m.role === 'user' ? 'is-user' : 'is-assistant'"
      >
        <span class="chat-role">{{ m.role === "user" ? $t("chat.you") : nameLabel }}</span>
        <div class="chat-body">{{ m.content || (chatState.busy && i === chatState.messages.length - 1 ? "…" : "") }}</div>
      </div>
    </div>

    <div class="chat-composer">
      <textarea
        v-model="chatState.draft"
        rows="3"
        :placeholder="$t('chat.inputPh')"
        :disabled="chatState.busy"
        @keydown="onKeydown"
      />
      <div class="chat-send-row">
        <button
          type="button"
          class="app-btn app-btn-primary"
          :disabled="chatState.busy || !String(chatState.draft || '').trim()"
          @click="onSend"
        >
          {{ chatState.busy ? $t("common.generating") : $t("common.send") }}
        </button>
        <button type="button" class="app-btn" :disabled="!chatState.busy" @click="cancelChat">
          {{ $t("common.cancel") }}
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.chat-page {
  display: flex;
  flex-direction: column;
  height: calc(100% - 8px);
  min-height: 420px;
  gap: 10px;
}
.chat-head {
  flex-shrink: 0;
}
.chat-tools {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
  margin-top: 10px;
}
.chat-persona {
  margin-top: 10px;
  padding: 10px 12px;
  border-radius: var(--radius-md);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.chat-persona .field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.chat-persona textarea {
  width: 100%;
  resize: vertical;
  min-height: 72px;
}
.chat-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 12px;
  border-radius: var(--radius-lg);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
}
.chat-empty {
  margin: 12px 0;
}
.chat-bubble {
  margin-bottom: 12px;
  max-width: min(720px, 100%);
}
.chat-bubble.is-user {
  margin-left: auto;
}
.chat-role {
  display: block;
  font-size: 11px;
  font-weight: 700;
  color: var(--muted);
  margin-bottom: 4px;
}
.chat-body {
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.55;
  padding: 10px 12px;
  border-radius: var(--radius-md);
  background: var(--panel);
  border: 1px solid var(--divider);
}
.is-user .chat-body {
  background: var(--accent-soft);
}
.chat-composer {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.chat-composer textarea {
  width: 100%;
  resize: vertical;
  min-height: 72px;
}
.chat-send-row {
  display: flex;
  gap: 8px;
}
</style>
