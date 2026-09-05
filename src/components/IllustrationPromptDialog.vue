<!--
  确认/修改绘图提示词后再出图
  代码路径: kk_novel_ai/src/components/IllustrationPromptDialog.vue
-->
<script setup>
import { createBackdropDismiss } from "../utils/backdropDismiss.js";
import {
  closeImagePromptDialog,
  imagePromptDialog,
} from "../services/illustration.js";

const backdrop = createBackdropDismiss(() => {
  if (imagePromptDialog.busy) return;
  closeImagePromptDialog(null);
});

function onConfirm() {
  if (imagePromptDialog.busy) return;
  const prompt = String(imagePromptDialog.prompt || "").trim();
  if (!prompt) {
    imagePromptDialog.error = "请填写提示词";
    return;
  }
  closeImagePromptDialog({
    prompt,
    negative: String(imagePromptDialog.negative || "").trim(),
    caption: String(imagePromptDialog.caption || "").trim(),
  });
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="imagePromptDialog.open"
      class="illus-mask"
      role="dialog"
      aria-modal="true"
      :aria-label="imagePromptDialog.title"
      @mousedown="!imagePromptDialog.busy && backdrop.onMouseDown($event)"
      @click="!imagePromptDialog.busy && backdrop.onClick($event)"
    >
      <div class="illus-card">
        <h2 class="illus-title">{{ imagePromptDialog.title }}</h2>
        <p class="muted">确认提示词后再出图。可改词；取消则不生成。</p>
        <label class="field-label">提示词</label>
        <textarea v-model="imagePromptDialog.prompt" rows="6" />
        <label class="field-label">负向提示</label>
        <textarea v-model="imagePromptDialog.negative" rows="2" />
        <label class="field-label">图题</label>
        <input v-model="imagePromptDialog.caption" type="text" />
        <p v-if="imagePromptDialog.error" class="err">{{ imagePromptDialog.error }}</p>
        <div class="illus-actions">
          <button
            type="button"
            class="app-btn"
            :disabled="imagePromptDialog.busy"
            @click="closeImagePromptDialog(null)"
          >
            取消
          </button>
          <button
            type="button"
            class="app-btn app-btn-primary"
            :disabled="imagePromptDialog.busy"
            @click="onConfirm"
          >
            生成图像
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.illus-mask {
  position: fixed;
  inset: 0;
  z-index: 80;
  background: color-mix(in srgb, #000 45%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}
.illus-card {
  width: min(560px, 100%);
  max-height: 90vh;
  overflow: auto;
  background: var(--panel, #1e1e1e);
  color: var(--text);
  border-radius: 12px;
  padding: 18px 18px 14px;
  box-shadow: var(--shadow);
}
.illus-title {
  margin: 0 0 8px;
  font-size: 1.1rem;
}
.field-label {
  display: block;
  margin: 10px 0 4px;
  font-size: 0.85rem;
}
.illus-card textarea,
.illus-card input {
  width: 100%;
  box-sizing: border-box;
}
.illus-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 14px;
}
.err {
  color: var(--error, #c44);
  margin: 8px 0 0;
}
.muted {
  color: var(--muted);
  font-size: 0.9rem;
}
</style>
