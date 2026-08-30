<!--
  角色信息浮窗
  代码路径: kk_novel_ai/src/components/CharacterHoverCard.vue
-->
<script setup>
import { computed } from "vue";
import { summaryForCard } from "../utils/characterNameIndex.js";

const props = defineProps({
  entry: { type: Object, default: null },
  term: { type: String, default: "" },
  x: { type: Number, default: 0 },
  y: { type: Number, default: 0 },
  visible: { type: Boolean, default: false },
});

const emit = defineEmits(["enter", "leave"]);

const attrsList = computed(() => {
  const a = (props.entry && props.entry.attrs) || {};
  return Object.entries(a)
    .filter(([k]) => !String(k).startsWith("_") && k !== "unique")
    .slice(0, 8)
    .map(([k, v]) => `${k}：${v}`);
});

const summary = computed(() => summaryForCard(props.entry));

const style = computed(() => {
  const pad = 12;
  const left = Math.min(props.x + 12, (typeof window !== "undefined" ? window.innerWidth : 800) - 280);
  const top = Math.min(props.y + 14, (typeof window !== "undefined" ? window.innerHeight : 600) - 160);
  return {
    left: `${Math.max(pad, left)}px`,
    top: `${Math.max(pad, top)}px`,
  };
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible && entry"
      class="char-hover-card"
      :style="style"
      @mouseenter="emit('enter')"
      @mouseleave="emit('leave')"
    >
      <div class="card-title">
        <strong>{{ entry.title }}</strong>
        <span v-if="term && term !== entry.title" class="term muted">「{{ term }}」</span>
        <span class="scope muted">{{ entry.scope === "global" ? "全局" : "本篇" }}</span>
      </div>
      <ul v-if="attrsList.length" class="attrs">
        <li v-for="(line, i) in attrsList" :key="i">{{ line }}</li>
      </ul>
      <p class="summary">{{ summary }}</p>
    </div>
  </Teleport>
</template>

<style scoped>
.char-hover-card {
  position: fixed;
  z-index: 10000;
  width: min(280px, calc(100vw - 24px));
  padding: 12px 14px;
  background: var(--panel, #fff);
  color: var(--text, #222);
  border-radius: var(--radius-md, 10px);
  box-shadow: var(--shadow, 0 8px 28px rgba(0, 0, 0, 0.16));
  pointer-events: auto;
  font-size: 12px;
  line-height: 1.5;
}
.card-title {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 6px;
  margin-bottom: 8px;
  font-size: 14px;
}
.term {
  font-size: 12px;
}
.scope {
  margin-left: auto;
  font-size: 11px;
}
.attrs {
  margin: 0 0 8px;
  padding: 0;
  list-style: none;
  color: var(--muted, #888);
  font-size: 11px;
}
.attrs li {
  margin: 0 0 2px;
}
.summary {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 9em;
  overflow: auto;
}
</style>
