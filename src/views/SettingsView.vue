<!--
  设置：LM Studio 多模型槽（修改即自动保存）
  代码路径: kk_novel_ai/src/views/SettingsView.vue
-->
<script setup>
import { onMounted, onUnmounted, ref, watch, computed } from "vue";
import { appState } from "../stores/appState.js";
import { loadSettings, saveSettings, refreshHealth, listModels } from "../services/llmClient.js";
import { invoke } from "../services/tauri.js";
import CapsuleSwitch from "../components/CapsuleSwitch.vue";
import { applyUiLocale, t, UI_LOCALES } from "../i18n/index.js";
import {
  EDITOR_FONT_PRESETS,
  EDITOR_FONT_SIZES,
  DEFAULT_EDITOR_FONT_SIZE,
  presetIdFromSettings,
  applyEditorTypography,
  fontPresetLabel,
} from "../utils/editorTypography.js";
import { isMobileUx } from "../utils/platform.js";
import { useToastError } from "../services/toast.js";
import {
  DEEPSEEK_OFFICIAL_PRICES,
  resolveDeepseekPeak as resolveDeepseekPeakUtil,
} from "../utils/deepseekPricing.js";
import {
  checkAppUpdate,
  downloadAppUpdate,
  getAppAbout,
  GITHUB_REPO_URL,
  launchDownloadedUpdate,
  openExternalUrl,
  revealDownloadedUpdate,
} from "../services/appUpdate.js";
import { formatUpdateSpeedMbs } from "../services/updateFlow.js";

const form = ref(null);
const models = ref([]);
const message = ref("");
const error = useToastError();
const pickTarget = ref("model");
const fontPresets = EDITOR_FONT_PRESETS;
const fontSizes = EDITOR_FONT_SIZES;
const appVersion = ref("");
const githubUrl = ref(GITHUB_REPO_URL);
const updateInfo = ref(null);
const updateChecking = ref(false);
const updateDownloading = ref(false);
const updateProgress = ref({ received: 0, total: 0 });
const updateStartedAt = ref(0);
const downloadedPath = ref("");
/** 仅用于重新输入；不回填已保存明文 */
const apiKeyDraft = ref("");
const apiKeyConfigured = ref(false);
const imageApiKeyDraft = ref("");
const imageApiKeyConfigured = ref(false);
const mobileUx = ref(isMobileUx());
/** DeepSeek 官方单价（与 src/utils/deepseekPricing.js 同步） */
const DEEPSEEK_PRICE_REF = DEEPSEEK_OFFICIAL_PRICES;
/** 内存中保留已保存 key，供未改时原样写回（不进输入框） */
let savedApiKey = "";
let savedImageApiKey = "";
/** 初始化灌表期间不触发自动保存 */
let hydrating = true;
/** 保存写回表单时抑制 watch 再入 */
let suppressing = false;
let saveTimer = null;
let saveSeq = 0;

onMounted(async () => {
  hydrating = true;
  try {
    const s = await loadSettings();
    savedApiKey = String((s && s.api_key) || "");
    apiKeyConfigured.value = !!savedApiKey;
    apiKeyDraft.value = "";
    savedImageApiKey = String((s && s.image_api_key) || "");
    imageApiKeyConfigured.value = !!savedImageApiKey;
    imageApiKeyDraft.value = "";
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
      writing_auto_story_sync: true,
      writing_strip_rhetoric: true,
      skip_delete_confirm: true,
      disable_thinking: null,
      editor_font_family: "heiti",
      editor_font_size: DEFAULT_EDITOR_FONT_SIZE,
      analytics_page_size: 10,
      price_input_per_1m: 0,
      price_output_per_1m: 0,
      price_cache_hit_per_1m: 0,
      api_provider: "local",
      deepseek_pricing_tier: "auto",
      writing_cache_friendly_prompt: true,
      writing_target_chars: 1800,
      image_provider: "openai_compat",
      image_base_url: "",
      image_model: "",
      image_size: "1024x1024",
      ui_locale: "zh-CN",
      writing_locale: "zh-CN",
      ...s,
      api_key: "",
      image_api_key: "",
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
    {
      const n = Number(form.value.analytics_page_size);
      if (!Number.isFinite(n) || n < 1) form.value.analytics_page_size = 10;
      else form.value.analytics_page_size = Math.min(200, Math.max(1, Math.floor(n)));
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
    if (form.value.writing_auto_story_sync == null) {
      form.value.writing_auto_story_sync = true;
    }
    if (form.value.writing_strip_rhetoric == null) {
      form.value.writing_strip_rhetoric = true;
    }
    if (form.value.writing_cache_friendly_prompt == null) {
      form.value.writing_cache_friendly_prompt = true;
    }
    if (!form.value.api_provider) {
      form.value.api_provider = String(form.value.base_url || "").toLowerCase().includes("deepseek.com")
        ? "deepseek_flash"
        : "local";
    }
    if (!form.value.deepseek_pricing_tier) {
      form.value.deepseek_pricing_tier = "auto";
    }
    if (!form.value.image_provider) {
      form.value.image_provider = "openai_compat";
    }
    if (!form.value.image_size) {
      form.value.image_size = "1024x1024";
    }
    if (form.value.ui_locale !== "en" && form.value.ui_locale !== "ja") {
      form.value.ui_locale = "zh-CN";
    }
    if (form.value.writing_locale !== "en" && form.value.writing_locale !== "ja") {
      form.value.writing_locale = "zh-CN";
    }
    applyUiLocale(form.value.ui_locale);
    form.value.base_url = normalizeBaseUrl(form.value.base_url);
    applyEditorTypography(form.value);
    try {
      const about = await getAppAbout();
      appVersion.value = about.version;
      githubUrl.value = about.githubUrl || GITHUB_REPO_URL;
    } catch {
      appVersion.value = "";
      githubUrl.value = GITHUB_REPO_URL;
    }
    await onHealth();
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    // 等下一拍，避免灌表/默认值回填误触发保存
    queueMicrotask(() => {
      hydrating = false;
    });
  }
});

onUnmounted(() => {
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  if (!hydrating && form.value) {
    void persistSettings({ silent: true });
  }
});

watch(
  form,
  () => {
    if (hydrating || suppressing || !form.value) return;
    scheduleSave();
  },
  { deep: true }
);

watch(apiKeyDraft, () => {
  if (hydrating || suppressing) return;
  scheduleSave();
});

watch(imageApiKeyDraft, () => {
  if (hydrating || suppressing) return;
  scheduleSave();
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

function scheduleSave() {
  if (saveTimer) clearTimeout(saveTimer);
  message.value = t("settings.saving");
  saveTimer = setTimeout(() => {
    saveTimer = null;
    void persistSettings();
  }, 450);
}

async function persistSettings(opts = {}) {
  if (!form.value) return;
  const silent = !!opts.silent;
  const seq = ++saveSeq;
  error.value = "";
  try {
    suppressing = true;
    syncMaxFromTarget();
    const nextKey = apiKeyDraft.value.trim();
    const nextImageKey = imageApiKeyDraft.value.trim();
    const payload = {
      ...form.value,
      api_key: nextKey || savedApiKey,
      image_api_key: nextImageKey || savedImageApiKey,
    };
    await saveSettings(payload);
    if (seq !== saveSeq) return;
    applyEditorTypography(form.value);
    if (nextKey) {
      savedApiKey = nextKey;
      apiKeyConfigured.value = true;
      apiKeyDraft.value = "";
    }
    if (nextImageKey) {
      savedImageApiKey = nextImageKey;
      imageApiKeyConfigured.value = true;
      imageApiKeyDraft.value = "";
    }
    form.value.api_key = "";
    form.value.image_api_key = "";
    if (!silent) message.value = t("settings.saved");
  } catch (e) {
    if (seq !== saveSeq) return;
    error.value = String(e.message || e);
    if (!silent) message.value = "";
  } finally {
    queueMicrotask(() => {
      suppressing = false;
    });
  }
}

function onFontPreview() {
  if (!form.value) return;
  applyEditorTypography(form.value);
}

function pickModel(id) {
  form.value[pickTarget.value] = id;
}

function normalizeBaseUrl(url) {
  const u = String(url || "").trim().replace(/\/+$/, "");
  if (u.includes("deepseek.com") && u.endsWith("/v1")) {
    return u.slice(0, -3);
  }
  return u;
}

function resolveDeepseekPeak() {
  return resolveDeepseekPeakUtil(form.value || {});
}

function applyDeepseekPreset(variant) {
  if (!form.value) return;
  const peak = resolveDeepseekPeak();
  const isPro = variant === "deepseek_pro";
  const ref = DEEPSEEK_PRICE_REF[isPro ? "pro" : "flash"][peak ? "peak" : "idle"];
  form.value.api_provider = isPro ? "deepseek_pro" : "deepseek_flash";
  form.value.base_url = "https://api.deepseek.com";
  form.value.model = isPro ? "deepseek-v4-pro" : "deepseek-v4-flash";
  form.value.analysis_model = "deepseek-v4-flash";
  form.value.writing_pro_model = isPro ? "deepseek-v4-pro" : "";
  form.value.writing_route_pro_on_continue = isPro;
  form.value.disable_thinking = true;
  form.value.writing_cache_friendly_prompt = true;
  form.value.price_cache_hit_per_1m = ref.hit;
  form.value.price_input_per_1m = ref.miss;
  form.value.price_output_per_1m = ref.out;
  form.value.llm_timeout_secs = Math.max(600, Number(form.value.llm_timeout_secs) || 600);
  form.value.context_budget = Math.max(12000, Number(form.value.context_budget) || 24000);
  message.value = t("settings.appliedDeepseek", {
    name: isPro ? "Pro" : "Flash",
    period: peak ? t("settings.peakWord") : t("settings.idleWord"),
  });
}

function applyLocalPreset() {
  if (!form.value) return;
  form.value.api_provider = "local";
  form.value.base_url = mobileUx.value ? "" : "http://127.0.0.1:1234/v1";
  form.value.price_cache_hit_per_1m = 0;
  form.value.price_input_per_1m = 0;
  form.value.price_output_per_1m = 0;
  form.value.disable_thinking = null;
  message.value = t("settings.appliedLocal");
}

function refreshDeepseekPrices() {
  if (!form.value) return;
  const u = String(form.value.base_url || "").toLowerCase();
  if (!u.includes("deepseek.com") && !String(form.value.api_provider || "").startsWith("deepseek")) {
    message.value = t("settings.notDeepseek");
    return;
  }
  const isPro =
    String(form.value.model || "").toLowerCase().includes("pro") ||
    form.value.api_provider === "deepseek_pro";
  const peak = resolveDeepseekPeak();
  const ref = DEEPSEEK_PRICE_REF[isPro ? "pro" : "flash"][peak ? "peak" : "idle"];
  form.value.price_cache_hit_per_1m = ref.hit;
  form.value.price_input_per_1m = ref.miss;
  form.value.price_output_per_1m = ref.out;
  message.value = t("settings.refreshedPrices", {
    period: peak ? t("settings.peakWord") : t("settings.idleWord"),
  });
}

const deepseekHintPeak = computed(() => resolveDeepseekPeak());

const pickSlotLabel = computed(() => {
  if (pickTarget.value === "model") return t("settings.slotWriting");
  if (pickTarget.value === "analysis_model") return t("settings.slotAnalysis");
  if (pickTarget.value === "writing_pro_model") return t("settings.slotPro");
  return t("settings.slotEmbedding");
});

const updatePct = computed(() => {
  const t = Number(updateProgress.value.total) || 0;
  const r = Number(updateProgress.value.received) || 0;
  if (t <= 0) return 0;
  return Math.min(100, Math.round((r / t) * 100));
});

function formatMb(bytes) {
  const n = Number(bytes) || 0;
  return (n / (1024 * 1024)).toFixed(2);
}

const updateReceivedMb = computed(() => formatMb(updateProgress.value.received));
const updateTotalMb = computed(() => {
  const t = Number(updateProgress.value.total) || 0;
  return t > 0 ? formatMb(t) : "?";
});
const updateSpeedMbs = computed(() =>
  formatUpdateSpeedMbs(updateProgress.value.received, updateStartedAt.value),
);

async function onCheckUpdate() {
  error.value = "";
  message.value = "";
  downloadedPath.value = "";
  updateChecking.value = true;
  try {
    const r = await checkAppUpdate();
    updateInfo.value = r;
    if (r && r.current) appVersion.value = String(r.current);
    if (r && r.has_update) {
      message.value = t("settings.foundVersion", { latest: r.latest });
    } else {
      message.value = t("settings.upToDate");
    }
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    updateChecking.value = false;
  }
}

async function onDownloadUpdate() {
  const info = updateInfo.value;
  if (!info || !info.download_url) {
    error.value = t("settings.noWinPkg");
    return;
  }
  error.value = "";
  message.value = t("settings.downloading");
  updateDownloading.value = true;
  updateProgress.value = { received: 0, total: 0 };
  updateStartedAt.value = 0;
  try {
    const r = await downloadAppUpdate(info, (p) => {
      updateProgress.value = {
        received: Number(p.received) || 0,
        total: Number(p.total) || 0,
      };
      if (!updateStartedAt.value && Number(p.received) > 0) {
        updateStartedAt.value = Date.now();
      }
    });
    downloadedPath.value = String((r && r.path) || "");
    if (!downloadedPath.value) {
      message.value = t("settings.downloadDone");
      return;
    }
    message.value = t("settings.launching");
    await launchDownloadedUpdate(downloadedPath.value);
  } catch (e) {
    error.value = String(e.message || e);
  } finally {
    updateDownloading.value = false;
  }
}

async function onRevealUpdate() {
  if (!downloadedPath.value) return;
  try {
    await revealDownloadedUpdate(downloadedPath.value);
  } catch (e) {
    error.value = String(e.message || e);
  }
}

function onOpenGithub(e) {
  if (mobileUx.value) return;
  e.preventDefault();
  void openExternalUrl(githubUrl.value);
}

async function onRebuildRag() {
  error.value = "";
  message.value = "";
  if (!appState.projectRoot) {
    error.value = t("settings.needProjectRag");
    return;
  }
  try {
    message.value = t("settings.rebuildingRag");
    const r = await invoke("rag_rebuild", { root: appState.projectRoot });
    message.value = t("settings.ragDone", { n: r.indexed || 0 });
  } catch (e) {
    error.value = String(e.message || e);
  }
}
</script>

<template>
  <section class="panel" v-if="form">
    <h1 class="panel-heading">{{ $t("settings.title") }}</h1>
    <p class="muted">
      <template v-if="mobileUx">
        {{ $t("settings.introMobile") }}
      </template>
      <template v-else>
        {{ $t("settings.introDesktop") }}
      </template>
      {{ $t("settings.autoSave") }}
    </p>

    <h2 class="panel-sub">{{ $t("locale.ui") }} / {{ $t("locale.writing") }}</h2>
    <p class="muted">{{ $t("locale.uiHint") }} {{ $t("locale.writingHint") }}</p>
    <div class="grid2">
      <div class="field">
        <label class="field-label">{{ $t("locale.ui") }}</label>
        <select v-model="form.ui_locale" @change="applyUiLocale(form.ui_locale)">
          <option v-for="loc in UI_LOCALES" :key="'ui-' + loc.id" :value="loc.id">{{ loc.native }}</option>
        </select>
      </div>
      <div class="field">
        <label class="field-label">{{ $t("locale.writing") }}</label>
        <select v-model="form.writing_locale">
          <option v-for="loc in UI_LOCALES" :key="'w-' + loc.id" :value="loc.id">{{ loc.native }}</option>
        </select>
      </div>
    </div>

    <h2 class="panel-sub">{{ $t("settings.about") }}</h2>
    <p>{{ $t("settings.version", { version: appVersion || $t("common.unknown") }) }}</p>
    <p class="muted about-github">
      {{ $t("settings.githubLabel") }}
      <a :href="githubUrl" target="_blank" rel="noopener" @click="onOpenGithub">{{ githubUrl }}</a>
    </p>
    <p class="muted">{{ $t("settings.updateHint") }}</p>
    <div class="actions version-actions">
      <button type="button" class="app-btn" :disabled="updateChecking || updateDownloading" @click="onCheckUpdate">
        {{ updateChecking ? $t("settings.checking") : $t("settings.checkUpdate") }}
      </button>
      <button
        v-if="updateInfo && updateInfo.has_update && updateInfo.download_url && !mobileUx"
        type="button"
        class="app-btn app-btn-primary"
        :disabled="updateDownloading"
        @click="onDownloadUpdate"
      >
        {{
          updateDownloading
            ? $t("settings.downloadingPct", { pct: updatePct })
            : $t("settings.downloadLaunch", { latest: updateInfo.latest })
        }}
      </button>
      <button
        v-if="downloadedPath"
        type="button"
        class="app-btn"
        @click="onRevealUpdate"
      >
        {{ $t("settings.openFolder") }}
      </button>
      <a
        v-if="updateInfo && updateInfo.has_update && updateInfo.html_url"
        class="app-btn"
        :href="updateInfo.html_url"
        target="_blank"
        rel="noopener"
      >{{ $t("settings.openRelease") }}</a>
    </div>
    <p v-if="updateInfo && updateInfo.has_update && updateInfo.download_url && !mobileUx" class="muted">
      {{ $t("settings.githubFallback", { url: updateInfo.download_url }) }}
    </p>
    <p v-if="updateDownloading" class="muted">
      {{
        $t("settings.downloadProgress", {
          a: updateReceivedMb,
          b: updateTotalMb,
          pct: updatePct,
          speed: updateSpeedMbs,
        })
      }}
    </p>
    <p v-if="updateInfo && updateInfo.has_update && updateInfo.notes" class="muted version-notes">
      {{ String(updateInfo.notes).slice(0, 400) }}
    </p>

    <h2 class="panel-sub">{{ $t("settings.presetTitle") }}</h2>
    <p class="muted preset-hint">
      {{ $t("settings.presetHintUrl") }}
      <code>https://api.deepseek.com</code>
      {{ $t("settings.presetHintNoV1") }}
      {{ $t("settings.presetHintCache") }}
      {{ $t("settings.presetHintSee") }}
      <a href="https://api-docs.deepseek.com/zh-cn/guides/kv_cache/" target="_blank" rel="noopener">{{ $t("settings.cacheDoc") }}</a>
      {{ $t("settings.presetHintAnd") }}
      <a href="https://api-docs.deepseek.com/zh-cn/quick_start/pricing" target="_blank" rel="noopener">{{ $t("settings.priceDoc") }}</a>
      {{ $t("settings.presetHintHours") }}
      {{ $t("settings.currentTier") }}<strong>{{ deepseekHintPeak ? $t("settings.peak") : $t("settings.idle") }}</strong>{{ $t("settings.currentTierEnd") }}
    </p>
    <div class="preset-actions">
      <button type="button" class="app-btn app-btn-primary" @click="applyDeepseekPreset('deepseek_flash')">
        {{ $t("settings.flash") }}
      </button>
      <button type="button" class="app-btn" @click="applyDeepseekPreset('deepseek_pro')">
        {{ $t("settings.pro") }}
      </button>
      <button type="button" class="app-btn" @click="applyLocalPreset">
        {{ $t("settings.localLm") }}
      </button>
      <button type="button" class="app-btn app-btn-info" @click="refreshDeepseekPrices">
        {{ $t("settings.refreshPrices") }}
      </button>
    </div>
    <div class="grid2">
      <div class="field">
        <label class="field-label">{{ $t("settings.apiProvider") }}</label>
        <select v-model="form.api_provider">
          <option value="local">local</option>
          <option value="deepseek_flash">deepseek_flash</option>
          <option value="deepseek_pro">deepseek_pro</option>
          <option value="custom">custom</option>
        </select>
      </div>
      <div class="field">
        <label class="field-label">deepseek_pricing_tier</label>
        <select v-model="form.deepseek_pricing_tier">
          <option value="auto">{{ $t("settings.tierAuto") }}</option>
          <option value="idle">{{ $t("settings.tierIdle") }}</option>
          <option value="peak">{{ $t("settings.tierPeak") }}</option>
        </select>
      </div>
    </div>

    <h2 class="panel-sub">{{ $t("settings.appearance") }}</h2>
    <div class="grid2">
      <div class="field">
        <label class="field-label">{{ $t("settings.font") }}</label>
        <select v-model="form.editor_font_family" @change="onFontPreview">
          <option v-for="p in fontPresets" :key="p.id" :value="p.id">{{ fontPresetLabel(p) }}</option>
        </select>
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.pageSize") }}</label>
        <input
          v-model.number="form.analytics_page_size"
          type="number"
          min="1"
          max="200"
          step="1"
        />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.fontSize") }}</label>
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
      {{ $t("settings.preview") }}
    </p>

    <div class="field">
      <label class="field-label">Base URL</label>
      <input
        v-model="form.base_url"
        type="text"
        :placeholder="mobileUx ? $t('settings.baseUrlPhMobile') : $t('settings.baseUrlPhDesktop')"
      />
    </div>
    <div class="field">
      <label class="field-label">API Key</label>
      <input
        v-model="apiKeyDraft"
        type="password"
        autocomplete="new-password"
        spellcheck="false"
        :placeholder="apiKeyConfigured ? $t('settings.apiKeyPhSaved') : $t('settings.apiKeyPhNew')"
      />
      <p class="muted api-key-hint">
        {{
          apiKeyConfigured
            ? $t("settings.apiKeyHintSaved")
            : $t("settings.apiKeyHintNew")
        }}
      </p>
    </div>
    <div class="field">
      <label class="field-label">{{ $t("settings.writingModel") }}</label>
      <input v-model="form.model" type="text" :placeholder="$t('settings.modelPh')" @focus="pickTarget = 'model'" />
    </div>
    <div class="field">
      <label class="field-label">{{ $t("settings.analysisModel") }}</label>
      <input
        v-model="form.analysis_model"
        type="text"
        :placeholder="$t('settings.optional')"
        @focus="pickTarget = 'analysis_model'"
      />
    </div>
    <div class="field">
      <label class="field-label">{{ $t("settings.embeddingModel") }}</label>
      <input
        v-model="form.embedding_model"
        type="text"
        :placeholder="$t('settings.embeddingPh')"
        @focus="pickTarget = 'embedding_model'"
      />
    </div>

    <h2 class="panel-sub">{{ $t("settings.imageTitle") }}</h2>
    <p class="muted">{{ $t("settings.imageIntro", { base: "{base}" }) }}</p>
    <div class="field">
      <label class="field-label">{{ $t("settings.imageProvider") }}</label>
      <select v-model="form.image_provider">
        <option value="openai_compat">{{ $t("settings.imageProviderOpenai") }}</option>
      </select>
    </div>
    <div class="field">
      <label class="field-label">{{ $t("settings.imageBaseUrl") }}</label>
      <input
        v-model="form.image_base_url"
        type="text"
        :placeholder="$t('settings.imageBaseUrlPh')"
      />
    </div>
    <div class="field">
      <label class="field-label">{{ $t("settings.imageApiKey") }}</label>
      <input
        v-model="imageApiKeyDraft"
        type="password"
        autocomplete="new-password"
        spellcheck="false"
        :placeholder="imageApiKeyConfigured ? $t('settings.apiKeyPhSaved') : $t('settings.imageApiKeyPhNew')"
      />
    </div>
    <div class="grid2">
      <div class="field">
        <label class="field-label">{{ $t("settings.imageModel") }}</label>
        <input v-model="form.image_model" type="text" :placeholder="$t('settings.imageModelPh')" />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.imageSize") }}</label>
        <input v-model="form.image_size" type="text" placeholder="1024x1024" />
      </div>
    </div>
    <div class="grid2">
      <div class="field">
        <label class="field-label">{{ $t("settings.temperature") }}</label>
        <input v-model.number="form.temperature" type="number" step="0.1" />
      </div>
      <div class="field">
        <label class="field-label">analysis_temperature</label>
        <input v-model.number="form.analysis_temperature" type="number" step="0.1" />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.targetChars") }}</label>
        <input
          v-model.number="form.writing_target_chars"
          type="number"
          min="200"
          step="100"
          @change="syncMaxFromTarget"
        />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.maxTokens") }}</label>
        <input v-model.number="form.max_tokens" type="number" min="256" readonly class="readonly-num" />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.frequencyPenalty") }}</label>
        <input v-model.number="form.frequency_penalty" type="number" step="0.05" min="0" max="2" />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.presencePenalty") }}</label>
        <input v-model.number="form.presence_penalty" type="number" step="0.05" min="0" max="2" />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.llmTimeout") }}</label>
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
        <label class="field-label">{{ $t("settings.priceInput") }}</label>
        <input v-model.number="form.price_input_per_1m" type="number" step="0.01" min="0" />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.priceCacheHit") }}</label>
        <input v-model.number="form.price_cache_hit_per_1m" type="number" step="0.01" min="0" />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.priceOutput") }}</label>
        <input v-model.number="form.price_output_per_1m" type="number" step="0.01" min="0" />
      </div>
      <div class="field">
        <label class="field-label">{{ $t("settings.writingProModel") }}</label>
        <input
          v-model="form.writing_pro_model"
          type="text"
          :placeholder="$t('settings.writingProPh')"
          @focus="pickTarget = 'writing_pro_model'"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_retry_on_loop"
          :label="$t('settings.retryOnLoop')"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_model_fallback"
          :label="$t('settings.modelFallback')"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_route_pro_on_continue"
          :label="$t('settings.routePro')"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_auto_digest"
          :label="$t('settings.autoDigest')"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_auto_cast"
          :label="$t('settings.autoCast')"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_auto_story_sync"
          :label="$t('settings.autoStorySync')"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_strip_rhetoric"
          :label="$t('settings.stripRhetoric')"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.skip_delete_confirm"
          :label="$t('settings.skipDeleteConfirm')"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.writing_cache_friendly_prompt"
          :label="$t('settings.cacheFriendly')"
        />
      </div>
      <div class="field capsule-switch-row">
        <CapsuleSwitch
          v-model="form.disable_thinking"
          :label="$t('settings.disableThinking')"
        />
      </div>
    </div>

    <div class="actions">
      <button type="button" class="app-btn app-btn-info" @click="onHealth">{{ $t("settings.detectRefresh") }}</button>
      <button type="button" class="app-btn app-btn-warning" @click="onRebuildRag">{{ $t("settings.rebuildRag") }}</button>
    </div>

    <p class="muted">{{ $t("settings.pickHint", { slot: pickSlotLabel }) }}</p>
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
  </section>
</template>

<style scoped>
.panel {
  min-height: calc(100% - 8px);
}
.preset-hint {
  margin: 0 0 10px;
  line-height: 1.55;
  font-size: 12px;
}
.preset-hint code {
  font-size: 11px;
}
.preset-hint a {
  color: var(--accent, #3b82f6);
}
.preset-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 12px;
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
.version-actions {
  margin-top: 8px;
}
.about-github a {
  word-break: break-all;
}
.version-notes {
  white-space: pre-wrap;
  font-size: 12px;
  max-height: 120px;
  overflow: auto;
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
