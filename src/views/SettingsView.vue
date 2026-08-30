<!--
  设置：LM Studio 多模型槽
  代码路径: kk_novel_ai/src/views/SettingsView.vue
-->
<script setup>
import { onMounted, ref } from "vue";
import { appState } from "../stores/appState.js";
import { loadSettings, saveSettings, refreshHealth, listModels } from "../services/llmClient.js";
import { invoke } from "../services/tauri.js";
import CapsuleSwitch from "../components/CapsuleSwitch.vue";
import {
  EDITOR_FONT_PRESETS,
  EDITOR_FONT_SIZES,
  DEFAULT_EDITOR_FONT_SIZE,
  presetIdFromSettings,
  applyEditorTypography,
} from "../utils/editorTypography.js";
import { isMobileUx } from "../utils/platform.js";

const form = ref(null);
const models = ref([]);
const message = ref("");
const error = ref("");
const pickTarget = ref("model");
const fontPresets = EDITOR_FONT_PRESETS;
const fontSizes = EDITOR_FONT_SIZES;
/** 仅用于重新输入；不回填已保存明文 */
const apiKeyDraft = ref("");
const apiKeyConfigured = ref(false);
const mobileUx = ref(isMobileUx());
/** 内存中保留已保存 key，供未改时原样写回（不进输入框） */
let savedApiKey = "";

onMounted(async () => {
  try {
    const s = await loadSettings();
    savedApiKey = String((s && s.api_key) || "");
    apiKeyConfigured.value = !!savedApiKey;
    apiKeyDraft.value = "";
    form.value = {
      analysis_model: "",
      analysis_temperature: 0.3,
      embedding_model: "",
      frequency_penalty: 0.55,
      presence_penalty: 0.25,
      llm_timeout_secs: 600,
      writing_retry_on_loop: true,
      writing_model_fallback: true,
      writing_pro_model: "",
      writing_route_pro_on_continue: true,
      writing_auto_digest: true,
      writing_auto_cast: true,
      writing_strip_rhetoric: true,
      skip_delete_confirm: true,
      disable_thinking: null,
      editor_font_family: "heiti",
      editor_font_size: DEFAULT_EDITOR_FONT_SIZE,
      price_input_per_1m: 0,
      price_output_per_1m: 0,
      writing_target_chars: 1800,
      ...s,
      api_key: "",
    };
    // 旧配置无规定字数：用 max_tokens 当作规定字数
    if (!form.value.writing_target_chars || form.value.writing_target_chars < 200) {
      form.value.writing_target_chars = Math.max(200, Number(form.value.max_tokens) || 1800);
    }
    syncMaxFromTarget();
    form.value.editor_font_family = presetIdFromSettings(form.value);
    if (!form.value.editor_font_size) {
      form.value.editor_font_size = DEFAULT_EDITOR_FONT_SIZE;
    }
    // Option<bool> from backend may be null → 商汤/DeepSeek 等推理模默认关思考链，避免 content 空
    if (form.value.disable_thinking == null) {
      const u = String(form.value.base_url || "").toLowerCase();
      form.value.disable_thinking =
        u.includes("sensenova.cn") || u.includes("deepseek.com");
    }
    if (form.value.skip_delete_confirm == null) {
      form.value.skip_delete_confirm = true;
    }
    if (form.value.writing_auto_cast == null) {
      form.value.writing_auto_cast = true;
    }
    if (form.value.writing_strip_rhetoric == null) {
      form.value.writing_strip_rhetoric = true;
    }
    applyEditorTypography(form.value);
    await onHealth();
  } catch (e) {
    error.value = String(e.message || e);
  }
});

async function onHealth() {
  error.value = "";
  try {
    await refreshHealth();
    message.value = appState.statusMessage;
    if (appState.llmOnline) {
      const r = await listModels();
      const data = r.models && r.models.data ? r.models.data : [];
      models.value = Array.isArray(data) ? data : [];
    }
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function syncMaxFromTarget() {
  if (!form.value) return;
  const chars = Math.max(200, Number(form.value.writing_target_chars) || 1800);
  form.value.writing_target_chars = chars;
  // 与后端 resolve_writing_max_tokens 一致：1.8×，允许超出规定字数
  form.value.max_tokens = Math.max(256, Math.ceil(chars * 1.8));
}

async function onSave() {
  error.value = "";
  try {
    syncMaxFromTarget();
    const nextKey = apiKeyDraft.value.trim();
    const payload = {
      ...form.value,
      api_key: nextKey || savedApiKey,
    };
    await saveSettings(payload);
    applyEditorTypography(form.value);
    if (nextKey) {
      savedApiKey = nextKey;
      apiKeyConfigured.value = true;
      apiKeyDraft.value = "";
    }
    form.value.api_key = "";
    message.value = "设置已保存";
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function onFontPreview() {
  if (!form.value) return;
  applyEditorTypography(form.value);
}

function pickModel(id) {
  form.value[pickTarget.value] = id;
}

async function onRebuildRag() {
  error.value = "";
  message.value = "";
  if (!appState.projectRoot) {
    error.value = "请先打开作品再重建索引";
    return;
  }
  try {
    message.value = "正在重建 embedding 索引…";
    const r = await invoke("rag_rebuild", { root: appState.projectRoot });
    message.value = `索引完成，条目约 ${r.indexed || 0}`;
  } catch (e) {
    error.value = String(e.message || e);
  }
}
</script>

<template>
  <section class="panel" v-if="form">
    <h1 class="panel-heading">设置</h1>
    <p class="muted">
      <template v-if="mobileUx">
        手机端请填写局域网或公网 OpenAI 兼容 API（不能使用 127.0.0.1 / localhost，那会指向手机自身）。
        例：http://192.168.1.8:1234/v1 或 https://api.example.com/v1。局域网 HTTP 仅建议在可信网络使用。
      </template>
      <template v-else>
        对接 LM Studio Local Server（默认 http://127.0.0.1:1234/v1）。写作 / 分析 / Embedding 分槽。
      </template>
    </p>

    <h2 class="panel-sub">写作区外观</h2>
    <div class="grid2">
      <div class="field">
        <label class="field-label">字体（默认黑体）</label>
        <select v-model="form.editor_font_family" @change="onFontPreview">
          <option v-for="p in fontPresets" :key="p.id" :value="p.id">{{ p.label }}</option>
        </select>
      </div>
      <div class="field">
        <label class="field-label">字号（px）</label>
        <select v-model.number="form.editor_font_size" @change="onFontPreview">
          <option v-for="n in fontSizes" :key="n" :value="n">{{ n }}</option>
        </select>
      </div>
    </div>
    <p
      class="font-preview muted"
      :style="{
        fontFamily: fontPresets.find((p) => p.id === form.editor_font_family)?.css,
        fontSize: form.editor_font_size + 'px',
      }"
    >
      预览：娜娜在雨棚下写字 —— The quick brown fox 0123456789
    </p>

    <div class="field">
      <label class="field-label">Base URL</label>
      <input
        v-model="form.base_url"
        type="text"
        :placeholder="mobileUx ? 'http://192.168.x.x:1234/v1 或 https://…/v1' : 'http://127.0.0.1:1234/v1'"
      />
    </div>
    <div class="field">
      <label class="field-label">API Key</label>
      <input
        v-model="apiKeyDraft"
        type="password"
        autocomplete="new-password"
        spellcheck="false"
        :placeholder="apiKeyConfigured ? '已保存，重新输入以覆盖（不可查看）' : '输入 API Key'"
      />
      <p class="muted api-key-hint">
        {{ apiKeyConfigured ? "当前已配置密钥，输入框不会回显明文，只能重新填写覆盖。" : "保存后将以密文方式保管，界面不可再查看。" }}
      </p>
    </div>
    <div class="field">
      <label class="field-label">写作模型（续写 / 润色 / 章纲扩展）</label>
      <input v-model="form.model" type="text" placeholder="从下方列表选择或手填" @focus="pickTarget = 'model'" />
    </div>
    <div class="field">
      <label class="field-label">分析模型（摘要 / 一致性，空则回退写作模型）</label>
      <input
        v-model="form.analysis_model"
        type="text"
        placeholder="可选"
        @focus="pickTarget = 'analysis_model'"
      />
    </div>
    <div class="field">
      <label class="field-label">Embedding 模型（RAG，空则仅关键词召回）</label>
      <input
        v-model="form.embedding_model"
        type="text"
        placeholder="如 text-embedding-nomic-embed-text-v1.5"
        @focus="pickTarget = 'embedding_model'"
      />
    </div>
    <div class="grid2">
      <div class="field">
        <label class="field-label">temperature（写作）</label>
        <input v-model.number="form.temperature" type="number" step="0.1" />
      </div>
      <div class="field">
        <label class="field-label">analysis_temperature</label>
        <input v-model.number="form.analysis_temperature" type="number" step="0.1" />
      </div>
      <div class="field">
        <label class="field-label">规定字数（每块至少达到；允许超出）</label>
        <input
          v-model.number="form.writing_target_chars"
          type="number"
          min="200"
          step="100"
          @change="syncMaxFromTarget"
        />
      </div>
      <div class="field">
        <label class="field-label">max_tokens（自动 = 规定字数×1.8，供超出用）</label>
        <input v-model.number="form.max_tokens" type="number" min="256" readonly class="readonly-num" />
      </div>
      <div class="field">
        <label class="field-label">frequency_penalty（抑复读，建议 0.4～0.8）</label>
        <input v-model.number="form.frequency_penalty" type="number" step="0.05" min="0" max="2" />
      </div>
      <div class="field">
        <label class="field-label">presence_penalty（鼓励新内容，建议 0.1～0.4）</label>
        <input v-model.number="form.presence_penalty" type="number" step="0.05" min="0" max="2" />
      </div>
      <div class="field">
        <label class="field-label">llm_timeout_secs（大模型建议 600+）</label>
        <input v-model.number="form.llm_timeout_secs" type="number" min="60" />
      </div>
      <div class="field">
        <label class="field-label">context_budget</label>
        <input v-model.number="form.context_budget" type="number" />
      </div>
      <div class="field">
        <label class="field-label">recent_window_chars</label>
        <input v-model.number="form.recent_window_chars" type="number" />
      </div>
      <div class="field">
        <label class="field-label">price_input_per_1m（元/百万输入 token，本地可 0）</label>
        <input v-model.number="form.price_input_per_1m" type="number" step="0.01" min="0" />
      </div>
      <div class="field">
        <label class="field-label">price_output_per_1m（元/百万输出 token）</label>
        <input v-model.number="form.price_output_per_1m" type="number" step="0.01" min="0" />
      </div>
      <div class="field">
        <label class="field-label">writing_pro_model（长续写强模型，如 deepseek-v4-pro）</label>
        <input
          v-model="form.writing_pro_model"
          type="text"
          placeholder="空则 DeepSeek flash 自动推断 pro"
          @focus="pickTarget = 'writing_pro_model'"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_retry_on_loop"
          label="writing_retry_on_loop（复读截断后自动重试）"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_model_fallback"
          label="writing_model_fallback（指定模型失败回退默认写作模型）"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_route_pro_on_continue"
          label="writing_route_pro_on_continue（续写自动走强模型）"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_auto_digest"
          label="writing_auto_digest（生成写入后自动提炼块记忆，供下轮续写）"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_auto_cast"
          label="writing_auto_cast（生成写入后自动把新人物加入本篇角色，默认开）"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_strip_rhetoric"
          label="writing_strip_rhetoric（定稿清洗「不是A是B」否定对照口癖，默认开）"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.skip_delete_confirm"
          label="skip_delete_confirm（删除不需确认，默认开；关掉后所有删除会弹窗）"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.disable_thinking"
          label="disable_thinking（商汤 / DeepSeek 等推理模建议开，避免 content 为空）"
        />
      </div>
    </div>

    <div class="actions">
      <button type="button" class="app-btn app-btn-primary" @click="onSave">保存</button>
      <button type="button" class="app-btn app-btn-info" @click="onHealth">检测连接 / 刷新模型</button>
      <button type="button" class="app-btn app-btn-warning" @click="onRebuildRag">重建 RAG 索引</button>
    </div>

    <p class="muted">点击模型填入：{{ pickTarget === "model" ? "写作" : pickTarget === "analysis_model" ? "分析" : pickTarget === "writing_pro_model" ? "续写强模型" : "Embedding" }} 槽</p>
    <p class="muted">{{ message }}</p>
    <div v-if="models.length" class="model-list">
      <button
        v-for="m in models"
        :key="m.id"
        type="button"
        class="app-btn"
        @click="pickModel(m.id)"
      >
        {{ m.id }}
      </button>
    </div>
    <pre v-if="error" class="out error">{{ error }}</pre>
  </section>
</template>

<style scoped>
.panel {
  min-height: calc(100% - 8px);
}
.panel-sub {
  margin: 16px 0 8px;
  font-size: 14px;
  font-weight: 700;
  color: var(--text);
}
.font-preview {
  margin: 0 0 14px;
  padding: 12px 14px;
  border-radius: var(--radius-md);
  background: var(--surface-solid);
  box-shadow: var(--shadow-sm);
  line-height: 1.7;
}
.api-key-hint {
  margin: 6px 0 0;
  font-size: 11px;
  line-height: 1.4;
}
.readonly-num {
  opacity: 0.85;
  cursor: default;
}
.grid2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}
@media (max-width: 720px) {
  .grid2 {
    grid-template-columns: 1fr;
  }
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
  flex-wrap: wrap;
}
.model-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
  padding: 14px;
  border-radius: var(--radius-lg);
  background: var(--panel-2);
  box-shadow: var(--shadow-sm);
}
.error {
  color: var(--error);
}
</style>
