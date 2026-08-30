<!--
  跨章连续阅读：邻章只读正文（与编辑器块结构对齐，供 TOC spy）
  代码路径: kk_novel_ai/src/components/ContinuousChapterRead.vue
-->
<script setup>
import { computed } from "vue";

const props = defineProps({
  chapterId: { type: String, required: true },
  title: { type: String, default: "" },
  blocks: { type: Array, default: () => [] },
});

const emit = defineEmits(["activate"]);

const list = computed(() => (Array.isArray(props.blocks) ? props.blocks : []));

function blockLabel(block, index) {
  if (!block || block.type !== "gen") return "";
  const dig = String(block.digest || block.summary || "").trim();
  if (dig) return dig.length > 48 ? `${dig.slice(0, 48)}…` : dig;
  const instr = String(block.instruction || "").trim();
  if (instr) return instr.length > 48 ? `${instr.slice(0, 48)}…` : instr;
  return `小节 ${index + 1}`;
}

function onActivate() {
  emit("activate", props.chapterId);
}
</script>

<template>
  <div
    class="continuous-read"
    title="点击本章可进入编辑"
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
      <div class="continuous-read-text">{{ block.text || "" }}</div>
    </div>
    <p v-if="!list.length" class="continuous-empty muted">（本章暂无正文）</p>
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
.continuous-empty {
  margin: 8px 0 0;
  font-size: 13px;
}
</style>
