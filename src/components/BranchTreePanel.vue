<!--
  章内分支图：复用 MindMapBoard，点击变体切换激活路径
  代码路径: kk_novel_ai/src/components/BranchTreePanel.vue
-->
<script setup>
import { computed } from "vue";
import { appState } from "../stores/appState.js";
import { buildBranchMindTree, activatePathToNode, findNodeById } from "../utils/branchModel.js";
import { switchBlockVariant } from "../services/draftAccept.js";
import { applyBranchDoc } from "../services/projectClient.js";
import {
  activePathKey,
  captureScrollerProgress,
  saveChapterProgress,
} from "../services/editorReadingProgress.js";
import MindMapBoard from "./MindMapBoard.vue";

const props = defineProps({
  height: { type: Number, default: 260 },
});

const emit = defineEmits(["select-block"]);

const chapterTitle = computed(() => {
  const list = (appState.project && appState.project.chapters) || [];
  const ch = list.find((c) => c.id === appState.chapterId);
  return (ch && ch.title) || "本章";
});

const tree = computed(() => {
  if (!appState.chapterBranchDoc) {
    return { id: "branch-root", label: chapterTitle.value, kind: "root", children: [] };
  }
  return buildBranchMindTree(appState.chapterBranchDoc, chapterTitle.value);
});

function resolveBlockKey(nodeId, variantId) {
  const doc = appState.chapterBranchDoc;
  if (!doc || !nodeId) return "";
  const node = findNodeById(doc, nodeId);
  if (!node) return "";
  const v = variantId
    ? node.variants.find((x) => x.id === variantId)
    : node.variants.find((x) => x.id === node.activeVariantId) || node.variants[0];
  return v?.key || "";
}

function saveCurrentPathProgress() {
  const root = appState.projectRoot;
  const chapterId = appState.chapterId;
  if (!root || !chapterId) return;
  const scroller = document.querySelector(".editor-scroll");
  if (!scroller) return;
  saveChapterProgress(
    root,
    chapterId,
    captureScrollerProgress(scroller, ""),
    activePathKey(appState.chapterBranchDoc)
  );
}

function onSelect(n) {
  if (!n) return;
  const id = String(n.id || "");
  const nodeId = id.startsWith("node:")
    ? id.slice(5)
    : id.startsWith("var:")
      ? null
      : "";
  let variantId = id.startsWith("var:") ? id.slice(4) : "";
  let resolvedNodeId = nodeId;

  saveCurrentPathProgress();

  if (variantId && appState.chapterBranchDoc) {
    const host = (appState.chapterBranchDoc.nodes || []).find((node) =>
      node.variants.some((v) => v.id === variantId)
    );
    if (host) {
      resolvedNodeId = host.id;
      switchBlockVariant(host.id, variantId);
    }
  } else if (resolvedNodeId && appState.chapterBranchDoc) {
    applyBranchDoc(activatePathToNode(appState.chapterBranchDoc, resolvedNodeId));
    const node = findNodeById(appState.chapterBranchDoc, resolvedNodeId);
    variantId = node?.activeVariantId || "";
  }

  const blockKey = resolveBlockKey(resolvedNodeId, variantId);
  if (blockKey) emit("select-block", blockKey);
}
</script>

<template>
  <div class="branch-panel">
    <div class="branch-panel-head">
      <span class="branch-panel-title">分支图</span>
      <span class="muted tip">点击变体切换 · 仅激活路径写入正文</span>
    </div>
    <MindMapBoard :tree="tree" :height="height" @select="onSelect" />
  </div>
</template>

<style scoped>
.branch-panel {
  border-top: 1px solid var(--border, #e5e5e5);
  background: var(--panel, #fff);
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.branch-panel-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 10px 4px;
}
.branch-panel-title {
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.04em;
}
.tip {
  font-size: 11px;
}
</style>
