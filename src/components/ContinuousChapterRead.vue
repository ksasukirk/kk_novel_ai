<!--
  跨章连续阅读：邻章只读正文（与编辑器块结构对齐，供 TOC spy）
  代码路径: kk_novel_ai/src/components/ContinuousChapterRead.vue
-->
<script setup>
import { computed } from "vue";
import { t, normalizeLocale } from "../i18n/index.js";
import { appState } from "../stores/appState.js";
import {
  getBlockLocaleText,
  isReadingTranslation,
} from "../utils/blockLocales.js";
import { isIllustrationBlock } from "../utils/genBlock.js";

const props = defineProps({
  chapterId: { type: String, required: true },
  title: { type: String, default: "" },
  blocks: { type: Array, default: () => [] },
  readingLocale: { type: String, default: "" },
  bilingualView: { type: Boolean, default: false },
});

const emit = defineEmits(["activate"]);

const list = computed(() => (Array.isArray(props.blocks) ? props.blocks : []));

const writingLocale = computed(() =>
  normalizeLocale(appState.settings?.writing_locale || "zh-CN")
);

function blockSourceLocale(block) {
  const s = String(block?.sourceLocale || "").trim();
  return s ? normalizeLocale(s) : writingLocale.value;
}

function blockIsReadingTranslation(block) {
  return isReadingTranslation(block, props.readingLocale, writingLocale.value);
}

function translationText(block) {
  const loc = String(props.readingLocale || "").trim();
  if (!loc) return "";
  return getBlockLocaleText(
    { ...block, sourceLocale: blockSourceLocale(block) },
    loc
  );
}

function primaryText(block) {
  if (!props.bilingualView && blockIsReadingTranslation(block)) {
    const tr = translationText(block);
    return String(tr || "").trim() ? tr : "";
  }
  return block.text || "";
}

function showTranslationPane(block) {
  if (isIllustrationBlock(block)) return false;
  if (!props.bilingualView) return false;
  return blockIsReadingTranslation(block);
}

function blockLabel(block, index) {
  if (!block || block.type !== "gen") return "";
  const dig = String(block.digest || block.summary || "").trim();
  if (dig) return dig.length > 48 ? `${dig.slice(0, 48)}…` : dig;
  const instr = String(block.instruction || "").trim();
  if (instr) return instr.length > 48 ? `${instr.slice(0, 48)}…` : instr;
  return t("editor.paragraphN", { n: index + 1 });
}

function onActivate() {
  emit("activate", props.chapterId);
}
</script>

<template>
  <div
    class="continuous-read"
    :title="$t('editor.clickToEdit')"
    @click="onActivate"
  >
    <div
      v-for="(block, index) in list"
      :key="block.key || `b-${index}`"
      class="chapter-block"
      :class="block.type === 'gen' ? 'is-gen' : 'is-plain'"
      :data-block-key="block.key || ''"
    >
      <div
        v-if="block.type === 'gen'"
        class="block-sticky-bar continuous-read-bar"
        :title="blockLabel(block, index)"
      >
        <span class="block-sum-label">{{ blockLabel(block, index) }}</span>
      </div>
      <div class="continuous-read-text">
        <template v-if="primaryText(block)">{{ primaryText(block) }}</template>
        <span
          v-else-if="!bilingualView && blockIsReadingTranslation(block)"
          class="muted"
        >{{ $t("editor.translationMissing") }}</span>
      </div>
      <div v-if="showTranslationPane(block)" class="continuous-translation">
        <div class="continuous-translation-label muted">{{ $t("editor.translationLabel") }}</div>
        <div
          v-if="String(translationText(block) || '').trim()"
          class="continuous-translation-body"
        >{{ translationText(block) }}</div>
        <p v-else class="muted continuous-translation-empty">{{ $t("editor.translationMissing") }}</p>
      </div>
    </div>
    <p v-if="!list.length" class="continuous-empty muted">{{ $t("editor.noBody") }}</p>
  </div>
</template>

<style scoped>
.continuous-read {
  padding: 4px 2px 20px;
  cursor: pointer;
}
.continuous-read:hover {
  background: color-mix(in srgb, var(--accent-soft, #fde8ee) 35%, transparent);
  border-radius: var(--radius-md, 8px);
}
.chapter-block {
  display: block;
  width: 100%;
  margin: 0 0 14px;
  position: relative;
  overflow-anchor: none;
  box-sizing: border-box;
}
.chapter-block.is-gen {
  padding: 10px 12px 12px;
  background: var(--surface-solid);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-sm);
  border-left: 3px solid color-mix(in srgb, var(--accent) 55%, var(--muted));
}
.chapter-block.is-plain {
  padding-bottom: 4px;
}
.continuous-read-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: -2px 0 8px;
  padding: 4px 0;
  font-size: 12px;
  color: var(--muted);
}
.block-sum-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}
.continuous-read-text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.75;
  font-family: var(--editor-font-family, inherit);
  font-size: var(--editor-font-size, inherit);
  color: var(--text);
}
.continuous-translation {
  margin-top: 8px;
  padding: 8px 10px;
  border-radius: var(--radius-md, 8px);
  background: color-mix(in srgb, var(--muted, #888) 10%, transparent);
  border: 1px dashed color-mix(in srgb, var(--border, #888) 55%, transparent);
}
.continuous-translation-label {
  font-size: 11px;
  margin-bottom: 4px;
}
.continuous-translation-body {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.75;
  font-family: var(--editor-font-family, inherit);
  font-size: var(--editor-font-size, inherit);
  color: var(--text);
}
.continuous-translation-empty {
  margin: 0;
  font-size: 13px;
}
.continuous-empty {
  margin: 8px 0 0;
  font-size: 13px;
}
</style>
