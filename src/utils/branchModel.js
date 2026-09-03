/**
 * 章内分支图：节点 + 多变体；编辑器只投影激活路径
 * 代码路径: kk_novel_ai/src/utils/branchModel.js
 */
import {
  contentFromBlocks,
  createGenBlock,
  createPlainBlock,
  cryptoRandomId,
  normalizeBlocks,
  normalizeSources,
  blockTocLabel,
} from "./genBlock.js";

function nextPlainKey() {
  return `b-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

export function isBranchDoc(raw) {
  return !!(raw && typeof raw === "object" && !Array.isArray(raw) && Number(raw.format) >= 2);
}

export function emptyBranchDoc() {
  return { format: 2, nodes: [], plains: [] };
}

/**
 * 旧 sidecar 数组 / 内存块列表 → format2
 * 顺序生成块记为同级根节点（非父子链），避免目录楼梯缩进
 * @param {unknown} raw
 */
export function migrateBlocksToBranchDoc(raw) {
  if (isBranchDoc(raw)) return normalizeBranchDoc(raw);
  const list = normalizeBlocks(Array.isArray(raw) ? raw : []);
  const doc = emptyBranchDoc();
  /** @type {string|null} */
  let prevNodeId = null;
  /** @type {Array} */
  let pendingPlains = [];

  const flushPlainsToPrev = () => {
    if (!pendingPlains.length) return;
    if (!prevNodeId) {
      doc.plains.push(...pendingPlains);
    } else {
      const node = doc.nodes.find((n) => n.id === prevNodeId);
      if (node) {
        node.trailingPlains = [...(node.trailingPlains || []), ...pendingPlains];
      } else {
        doc.plains.push(...pendingPlains);
      }
    }
    pendingPlains = [];
  };

  for (const b of list) {
    if (!b || b.type !== "gen") {
      pendingPlains.push({
        key: b?.key || nextPlainKey(),
        text: String(b?.text ?? ""),
      });
      continue;
    }
    flushPlainsToPrev();
    const variantId = cryptoRandomId();
    const nodeId = cryptoRandomId();
    const variant = variantFromGenBlock(b, {
      id: variantId,
      label: "变体1",
    });
    doc.nodes.push({
      id: nodeId,
      parentId: null,
      fromVariantId: null,
      activeVariantId: variantId,
      variants: [variant],
      trailingPlains: [],
    });
    prevNodeId = nodeId;
  }
  flushPlainsToPrev();
  return normalizeBranchDoc(doc);
}

/**
 * @param {unknown} raw
 */
export function normalizeBranchDoc(raw) {
  if (Array.isArray(raw)) return migrateBlocksToBranchDoc(raw);
  if (!raw || typeof raw !== "object") return emptyBranchDoc();
  if (!isBranchDoc(raw) && Array.isArray(raw.nodes)) {
    raw = { ...raw, format: 2 };
  }
  if (!isBranchDoc(raw)) return migrateBlocksToBranchDoc(raw);

  const plains = Array.isArray(raw.plains)
    ? raw.plains.map((p) => ({
        key: String(p?.key || nextPlainKey()),
        text: String(p?.text ?? ""),
      }))
    : [];

  const nodes = (Array.isArray(raw.nodes) ? raw.nodes : []).map((n) => {
    const variants = (Array.isArray(n?.variants) ? n.variants : [])
      .map((v, i) => normalizeVariant(v, i))
      .filter(Boolean);
    if (!variants.length) return null;
    dedupeVariantLabels(variants);
    let activeVariantId = String(n.activeVariantId || "");
    if (!variants.some((v) => v.id === activeVariantId)) {
      activeVariantId = variants[0].id;
    }
    const trailingPlains = Array.isArray(n.trailingPlains)
      ? n.trailingPlains.map((p) => ({
          key: String(p?.key || nextPlainKey()),
          text: String(p?.text ?? ""),
        }))
      : [];
    return {
      id: String(n.id || cryptoRandomId()),
      parentId: n.parentId != null && n.parentId !== "" ? String(n.parentId) : null,
      fromVariantId:
        n.fromVariantId != null && n.fromVariantId !== ""
          ? String(n.fromVariantId)
          : null,
      activeVariantId,
      variants,
      trailingPlains,
    };
  }).filter(Boolean);

  return { format: 2, nodes, plains };
}

function normalizeVariant(v, index = 0) {
  if (!v || typeof v !== "object") return null;
  const text = String(v.text ?? "");
  const meta = v.meta && typeof v.meta === "object" ? v.meta : {};
  const rawLabel = v.label != null ? String(v.label).trim() : "";
  return {
    id: String(v.id || cryptoRandomId()),
    key: String(v.key || nextPlainKey()),
    label: rawLabel || `变体${index + 1}`,
    text,
    instruction: String(v.instruction || ""),
    task: String(v.task || ""),
    digest: String(v.digest || ""),
    meta: {
      id: meta.id || v.genId || "",
      ts: meta.ts || v.ts || "",
      model: meta.model || v.model || "",
      chars: meta.chars != null ? Number(meta.chars) : [...text].length,
      tokens: meta.tokens != null && meta.tokens !== "" ? Number(meta.tokens) : null,
      cost: meta.cost != null && meta.cost !== "" ? Number(meta.cost) : null,
      usageSource: meta.usageSource || v.usageSource || "",
      sources: normalizeSources(meta.sources || v.sources),
    },
  };
}

/**
 * 同节点内下一个不重复的「变体N」标签
 * @param {Array<{label?:string}>} variants
 * @param {string} [preferred] 若未占用则优先用
 */
export function nextUniqueVariantLabel(variants, preferred = "") {
  const list = Array.isArray(variants) ? variants : [];
  const used = new Set(
    list.map((v) => String(v?.label || "").trim()).filter(Boolean)
  );
  const pref = String(preferred || "").trim();
  if (pref && !used.has(pref)) return pref;

  let max = 0;
  for (const label of used) {
    const m = /^变体\s*(\d+)$/u.exec(label);
    if (m) max = Math.max(max, Number(m[1]) || 0);
  }
  let n = Math.max(max + 1, list.length + 1, 1);
  let label = `变体${n}`;
  while (used.has(label)) {
    n += 1;
    label = `变体${n}`;
  }
  return label;
}

/** 修正同节点内重复的变体名（后者递增） */
export function dedupeVariantLabels(variants) {
  const list = Array.isArray(variants) ? variants : [];
  const used = new Set();
  for (const v of list) {
    let label = String(v?.label || "").trim() || "变体1";
    if (!used.has(label)) {
      v.label = label;
      used.add(label);
      continue;
    }
    const m = /^变体\s*(\d+)$/u.exec(label);
    let n = m ? Number(m[1]) + 1 : list.indexOf(v) + 1;
    let next = `变体${n}`;
    while (used.has(next)) {
      n += 1;
      next = `变体${n}`;
    }
    v.label = next;
    used.add(next);
  }
  return list;
}

/**
 * @param {object} block gen block
 * @param {{ id?: string, label?: string, key?: string }} [opts]
 */
export function variantFromGenBlock(block, opts = {}) {
  const text = String(block?.text ?? "");
  const label =
    opts.label != null && String(opts.label).trim()
      ? String(opts.label).trim()
      : "";
  return normalizeVariant(
    {
      id: opts.id,
      key: opts.key || block?.key,
      label,
      text,
      instruction: block?.instruction || "",
      task: block?.task || "",
      digest: block?.digest || "",
      meta: {
        id: block?.id,
        ts: block?.ts,
        model: block?.model,
        chars: block?.chars,
        tokens: block?.tokens,
        cost: block?.cost,
        usageSource: block?.usageSource,
        sources: block?.sources,
      },
    },
    0
  );
}

function genBlockFromVariant(variant, nodeId) {
  const m = variant.meta || {};
  const block = createGenBlock(
    {
      id: m.id,
      ts: m.ts,
      task: variant.task,
      model: m.model,
      chars: m.chars,
      tokens: m.tokens,
      cost: m.cost,
      usageSource: m.usageSource,
      instruction: variant.instruction,
      sources: m.sources,
      digest: variant.digest,
    },
    variant.text
  );
  block.key = variant.key;
  block._nodeId = nodeId;
  block._variantId = variant.id;
  block._variantLabel = variant.label;
  return block;
}

/**
 * 根节点顺序（parentId 为空），按 nodes 数组序
 * @param {{nodes: Array}} doc
 */
export function rootNodes(doc) {
  return (doc?.nodes || []).filter((n) => !n.parentId);
}

/**
 * 某变体下的直接子节点（按数组序）
 */
export function childNodesOf(doc, parentId, fromVariantId) {
  return (doc?.nodes || []).filter(
    (n) => n.parentId === parentId && n.fromVariantId === fromVariantId
  );
}

/**
 * 激活路径上的节点（深度优先：根按序，每个根跟 active 子链）
 * @returns {Array}
 */
export function activePathNodes(doc) {
  const d = normalizeBranchDoc(doc);
  const out = [];
  const visitChain = (node) => {
    out.push(node);
    const active = node.activeVariantId;
    const kids = childNodesOf(d, node.id, active);
    for (const kid of kids) visitChain(kid);
  };
  for (const root of rootNodes(d)) visitChain(root);
  return out;
}

/**
 * 激活路径 → 编辑器块列表（带 _nodeId / _variantId）
 */
export function activePathBlocks(doc) {
  const d = normalizeBranchDoc(doc);
  /** @type {Array} */
  const blocks = [];
  for (const p of d.plains) {
    blocks.push({ key: p.key, type: "plain", text: p.text });
  }
  for (const node of activePathNodes(d)) {
    const v = node.variants.find((x) => x.id === node.activeVariantId) || node.variants[0];
    if (!v) continue;
    blocks.push(genBlockFromVariant(v, node.id));
    for (const p of node.trailingPlains || []) {
      blocks.push({ key: p.key, type: "plain", text: p.text });
    }
  }
  if (!blocks.length) return [createPlainBlock("")];
  return blocks;
}

export function contentFromActivePath(doc) {
  return contentFromBlocks(activePathBlocks(doc));
}

/**
 * 把编辑器块的正文/digest 写回 doc（按 key / _variantId）
 */
export function syncDocFromBlocks(doc, blocks) {
  const d = normalizeBranchDoc(doc);
  const list = Array.isArray(blocks) ? blocks : [];
  const byKey = new Map();
  for (const b of list) {
    if (b?.key) byKey.set(b.key, b);
  }

  for (const p of d.plains) {
    const b = byKey.get(p.key);
    if (b && b.type === "plain") p.text = String(b.text ?? "");
  }

  for (const node of d.nodes) {
    for (const v of node.variants) {
      const b = byKey.get(v.key);
      if (!b || b.type !== "gen") continue;
      v.text = String(b.text ?? "");
      v.digest = String(b.digest ?? "");
      v.instruction = String(b.instruction ?? v.instruction);
      v.task = String(b.task ?? v.task);
      if (!v.meta) v.meta = {};
      v.meta.chars = [...v.text].length;
      if (b.sources) v.meta.sources = normalizeSources(b.sources);
    }
    for (const p of node.trailingPlains || []) {
      const b = byKey.get(p.key);
      if (b && b.type === "plain") p.text = String(b.text ?? "");
    }
  }
  return d;
}

export function switchVariant(doc, nodeId, variantId) {
  const d = normalizeBranchDoc(doc);
  const node = d.nodes.find((n) => n.id === nodeId);
  if (!node) return d;
  if (!node.variants.some((v) => v.id === variantId)) return d;
  node.activeVariantId = variantId;
  return d;
}

/**
 * 激活祖先链，使 targetNodeId 落在激活路径上
 */
export function activatePathToNode(doc, targetNodeId) {
  let d = normalizeBranchDoc(doc);
  const byId = new Map(d.nodes.map((n) => [n.id, n]));
  const target = byId.get(targetNodeId);
  if (!target) return d;

  /** @type {Array} */
  const chain = [];
  let cur = target;
  while (cur) {
    chain.push(cur);
    cur = cur.parentId ? byId.get(cur.parentId) : null;
  }
  chain.reverse();
  for (let i = 1; i < chain.length; i++) {
    const child = chain[i];
    const parent = chain[i - 1];
    if (child.fromVariantId && parent.activeVariantId !== child.fromVariantId) {
      d = switchVariant(d, parent.id, child.fromVariantId);
    }
  }
  return d;
}

export function addVariant(doc, nodeId, variant, { activate = true } = {}) {
  const d = normalizeBranchDoc(doc);
  const node = d.nodes.find((n) => n.id === nodeId);
  if (!node) return { doc: d, variant: null };
  const label = nextUniqueVariantLabel(node.variants, variant?.label);
  const v = normalizeVariant(
    {
      ...variant,
      label,
    },
    node.variants.length
  );
  v.label = label;
  node.variants.push(v);
  if (activate) node.activeVariantId = v.id;
  return { doc: d, variant: v };
}

export function forkChild(doc, parentNodeId, fromVariantId, firstVariant) {
  const d = normalizeBranchDoc(doc);
  const parent = d.nodes.find((n) => n.id === parentNodeId);
  if (!parent) return { doc: d, node: null, variant: null };
  const fromId =
    fromVariantId ||
    parent.activeVariantId ||
    (parent.variants[0] && parent.variants[0].id);
  if (!fromId || !parent.variants.some((v) => v.id === fromId)) {
    return { doc: d, node: null, variant: null };
  }
  const v = normalizeVariant(
    { ...firstVariant, label: firstVariant?.label || "变体1" },
    0
  );
  const node = {
    id: cryptoRandomId(),
    parentId: parentNodeId,
    fromVariantId: fromId,
    activeVariantId: v.id,
    variants: [v],
    trailingPlains: [],
  };
  d.nodes.push(node);
  // 父节点切到该变体，保证子链可见
  parent.activeVariantId = fromId;
  return { doc: d, node, variant: v };
}

/**
 * 在激活路径末尾追加新节点（普通续写）
 * 记为同级根节点，避免线性续写变成父子链导致目录楼梯缩进；
 * 真分叉仍用 forkChild。
 */
export function appendOnActivePath(doc, firstVariant) {
  const d = normalizeBranchDoc(doc);
  const v = normalizeVariant(
    { ...firstVariant, label: firstVariant?.label || "变体1" },
    0
  );
  const node = {
    id: cryptoRandomId(),
    parentId: null,
    fromVariantId: null,
    activeVariantId: v.id,
    variants: [v],
    trailingPlains: [],
  };
  d.nodes.push(node);
  return { doc: d, node, variant: v };
}

/**
 * 更新某变体正文（重写）
 */
export function replaceVariantText(doc, nodeId, variantId, genLike) {
  const d = normalizeBranchDoc(doc);
  const node = d.nodes.find((n) => n.id === nodeId);
  if (!node) return d;
  const idx = node.variants.findIndex((v) => v.id === variantId);
  if (idx < 0) return d;
  const prev = node.variants[idx];
  const next = variantFromGenBlock(
    {
      ...genLike,
      key: prev.key,
      digest: genLike.digest != null ? genLike.digest : "",
    },
    { id: prev.id, label: prev.label, key: prev.key }
  );
  node.variants[idx] = next;
  node.activeVariantId = next.id;
  return d;
}

function collectDescendantIds(doc, nodeId) {
  const ids = new Set([nodeId]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const n of doc.nodes) {
      if (n.parentId && ids.has(n.parentId) && !ids.has(n.id)) {
        ids.add(n.id);
        grew = true;
      }
    }
  }
  return ids;
}

/**
 * 删除变体；若是最后变体则删节点并级联子树
 * @returns {{ doc, removedKeys: string[] }}
 */
export function deleteVariantOrNode(doc, nodeId, variantId) {
  const d = normalizeBranchDoc(doc);
  const node = d.nodes.find((n) => n.id === nodeId);
  if (!node) return { doc: d, removedKeys: [] };
  /** @type {string[]} */
  const removedKeys = [];

  if (node.variants.length > 1 && variantId) {
    const v = node.variants.find((x) => x.id === variantId);
    if (!v) return { doc: d, removedKeys: [] };
    removedKeys.push(v.key);
    // 挂在该变体下的子树一并删
    const childIds = d.nodes
      .filter((n) => n.parentId === nodeId && n.fromVariantId === variantId)
      .map((n) => n.id);
    const drop = new Set();
    for (const cid of childIds) {
      for (const id of collectDescendantIds(d, cid)) drop.add(id);
    }
    for (const n of d.nodes) {
      if (drop.has(n.id)) {
        for (const vv of n.variants) removedKeys.push(vv.key);
      }
    }
    d.nodes = d.nodes.filter((n) => !drop.has(n.id));
    node.variants = node.variants.filter((x) => x.id !== variantId);
    if (node.activeVariantId === variantId) {
      node.activeVariantId = node.variants[0].id;
    }
    return { doc: d, removedKeys };
  }

  // 删整个节点 + 级联
  const drop = collectDescendantIds(d, nodeId);
  for (const n of d.nodes) {
    if (drop.has(n.id)) {
      for (const vv of n.variants) removedKeys.push(vv.key);
    }
  }
  d.nodes = d.nodes.filter((n) => !drop.has(n.id));
  return { doc: d, removedKeys };
}

export function findNodeByBlockKey(doc, blockKey) {
  const d = normalizeBranchDoc(doc);
  for (const n of d.nodes) {
    for (const v of n.variants) {
      if (v.key === blockKey) return { node: n, variant: v };
    }
  }
  return null;
}

export function findNodeById(doc, nodeId) {
  return normalizeBranchDoc(doc).nodes.find((n) => n.id === nodeId) || null;
}

/**
 * 生成上下文前缀
 * @param {"variant"|"fork"|"continue"|string} mode
 * @param {string} [nodeId]
 */
export function branchContextText(doc, mode, nodeId) {
  const d = normalizeBranchDoc(doc);
  const parts = [];
  for (const p of d.plains) {
    if (String(p.text || "").trim()) parts.push(String(p.text).replace(/\s+$/g, ""));
  }

  const path = activePathNodes(d);
  if (mode === "variant" && nodeId) {
    for (const node of path) {
      if (node.id === nodeId) break;
      const v = node.variants.find((x) => x.id === node.activeVariantId);
      if (v && String(v.text || "").trim()) parts.push(String(v.text).replace(/\s+$/g, ""));
      for (const p of node.trailingPlains || []) {
        if (String(p.text || "").trim()) parts.push(String(p.text).replace(/\s+$/g, ""));
      }
    }
  } else if (mode === "fork" && nodeId) {
    for (const node of path) {
      const v = node.variants.find((x) => x.id === node.activeVariantId);
      if (v && String(v.text || "").trim()) parts.push(String(v.text).replace(/\s+$/g, ""));
      for (const p of node.trailingPlains || []) {
        if (String(p.text || "").trim()) parts.push(String(p.text).replace(/\s+$/g, ""));
      }
      if (node.id === nodeId) break;
    }
  } else {
    // continue / 整条激活路径
    for (const node of path) {
      const v = node.variants.find((x) => x.id === node.activeVariantId);
      if (v && String(v.text || "").trim()) parts.push(String(v.text).replace(/\s+$/g, ""));
      for (const p of node.trailingPlains || []) {
        if (String(p.text || "").trim()) parts.push(String(p.text).replace(/\s+$/g, ""));
      }
    }
  }

  while (parts.length && !parts[parts.length - 1].trim()) parts.pop();
  return parts.join("\n\n").replace(/^\n+/, "") + (parts.some((p) => p.length) ? "\n" : "");
}

/**
 * 激活路径上、目标节点之前一节的 digest（本段记忆）
 * @param {object} doc
 * @param {string} nodeId
 * @returns {string}
 */
export function previousSectionDigest(doc, nodeId) {
  if (!nodeId) return "";
  const path = activePathNodes(doc);
  let prev = null;
  for (const node of path) {
    if (node.id === nodeId) break;
    prev = node;
  }
  if (!prev) return "";
  const v =
    prev.variants.find((x) => x.id === prev.activeVariantId) || prev.variants[0];
  return String(v?.digest || "").trim();
}

/**
 * 目录：激活路径上的小节 + 各变体 + 子岔开预览
 * 线性单子链展平为同级（depth 不变）；仅多子真分叉时加深缩进
 * @returns {Array<{kind:string,key?:string,nodeId?:string,variantId?:string,label:string,genIndex:number,active?:boolean,depth:number}>}
 */
export function branchTocTree(doc) {
  const d = normalizeBranchDoc(doc);
  /** @type {Array} */
  const out = [];
  let genIndex = 0;

  const walk = (node, depth) => {
    const activeV =
      node.variants.find((x) => x.id === node.activeVariantId) || node.variants[0];
    const sectionLabel = blockTocLabel(
      {
        type: "gen",
        instruction: activeV?.instruction,
        text: activeV?.text,
      },
      genIndex
    );
    out.push({
      kind: "section",
      key: activeV?.key,
      nodeId: node.id,
      variantId: activeV?.id,
      label: sectionLabel,
      genIndex,
      active: true,
      depth,
    });
    const sectionGen = genIndex;
    genIndex += 1;

    node.variants.forEach((v, i) => {
      out.push({
        kind: "variant",
        key: v.key,
        nodeId: node.id,
        variantId: v.id,
        label: v.label || `变体${i + 1}`,
        genIndex: sectionGen,
        active: v.id === node.activeVariantId,
        depth: depth + 1,
      });
    });

    // 当前激活变体下的子节点：单子=线性续写展平；多子=真分叉加深
    const kids = childNodesOf(d, node.id, node.activeVariantId);
    const childDepth = kids.length > 1 ? depth + 1 : depth;
    for (const kid of kids) walk(kid, childDepth);

    // 非激活变体若有子岔开，列一行提示（点选会先切变体）
    for (const v of node.variants) {
      if (v.id === node.activeVariantId) continue;
      const otherKids = childNodesOf(d, node.id, v.id);
      for (const kid of otherKids) {
        const kv =
          kid.variants.find((x) => x.id === kid.activeVariantId) || kid.variants[0];
        out.push({
          kind: "branchHint",
          key: kv?.key,
          nodeId: kid.id,
          variantId: kv?.id,
          parentVariantId: v.id,
          parentNodeId: node.id,
          label: `${v.label || "变体"} → ${blockTocLabel(
            { type: "gen", instruction: kv?.instruction, text: kv?.text },
            genIndex
          )}`,
          genIndex,
          active: false,
          depth: depth + 1,
        });
      }
    }
  };

  for (const root of rootNodes(d)) walk(root, 0);
  return out;
}

/**
 * MindMapBoard 用树
 * @param {string} [chapterTitle]
 */
export function buildBranchMindTree(doc, chapterTitle = "本章") {
  const d = normalizeBranchDoc(doc);

  const variantNode = (node, v, i) => {
    const kids = childNodesOf(d, node.id, v.id).map((child) => nodeMind(child));
    return {
      id: `var:${v.id}`,
      label: v.label || `变体${i + 1}`,
      kind: v.id === node.activeVariantId ? "activeVariant" : "variant",
      meta: v.id === node.activeVariantId ? "当前" : "",
      children: kids,
      _nodeId: node.id,
      _variantId: v.id,
      _blockKey: v.key,
    };
  };

  const nodeMind = (node) => {
    const active =
      node.variants.find((x) => x.id === node.activeVariantId) || node.variants[0];
    return {
      id: `node:${node.id}`,
      label: blockTocLabel(
        { type: "gen", instruction: active?.instruction, text: active?.text },
        0
      ),
      kind: "section",
      meta: `${node.variants.length} 变体`,
      children: node.variants.map((v, i) => variantNode(node, v, i)),
      _nodeId: node.id,
      _variantId: active?.id,
      _blockKey: active?.key,
    };
  };

  return {
    id: "branch-root",
    label: chapterTitle || "本章",
    kind: "root",
    children: rootNodes(d).map((n) => nodeMind(n)),
  };
}

/** 落盘 format2（完整树，含非激活） */
export function branchDocForPersist(doc) {
  const d = normalizeBranchDoc(doc);
  return {
    format: 2,
    plains: d.plains.map((p) => ({ key: p.key, text: p.text })),
    nodes: d.nodes.map((n) => ({
      id: n.id,
      parentId: n.parentId,
      fromVariantId: n.fromVariantId,
      activeVariantId: n.activeVariantId,
      trailingPlains: (n.trailingPlains || []).map((p) => ({
        key: p.key,
        text: p.text,
      })),
      variants: n.variants.map((v) => ({
        id: v.id,
        key: v.key,
        label: v.label,
        text: v.text,
        instruction: v.instruction,
        task: v.task,
        digest: v.digest,
        meta: {
          id: v.meta?.id || "",
          ts: v.meta?.ts || "",
          model: v.meta?.model || "",
          chars: v.meta?.chars,
          tokens: v.meta?.tokens,
          cost: v.meta?.cost,
          usageSource: v.meta?.usageSource || "",
          sources: normalizeSources(v.meta?.sources),
        },
      })),
    })),
  };
}

/**
 * 从 sidecar 原始 JSON 解析为 doc（数组或 format2）
 */
export function parseSidecarToBranchDoc(sidecar) {
  if (sidecar == null) return emptyBranchDoc();
  if (Array.isArray(sidecar)) return migrateBlocksToBranchDoc(sidecar);
  if (isBranchDoc(sidecar)) return normalizeBranchDoc(sidecar);
  // 偶发包了一层
  if (Array.isArray(sidecar.blocks)) return migrateBlocksToBranchDoc(sidecar.blocks);
  return migrateBlocksToBranchDoc([]);
}

/**
 * 旧版分节/多生成块：激活路径上存在多个小节
 * @param {unknown} doc
 */
export function chapterNeedsSectionCollapse(doc) {
  const d = normalizeBranchDoc(
    isBranchDoc(doc) ? doc : migrateBlocksToBranchDoc(doc || [])
  );
  if (activePathNodes(d).length > 1) return true;
  if (rootNodes(d).length > 1) return true;
  return activePathBlocks(d).filter((b) => b.type === "gen").length > 1;
}

/**
 * 将一章内多个残留小节合并为整章一块（保留最后一块元数据 + 合并摘要）
 * @param {unknown} doc
 * @returns {{ doc: object, changed: boolean }}
 */
export function collapseChapterSectionsToWholeChapter(doc) {
  const d = normalizeBranchDoc(
    isBranchDoc(doc) ? doc : migrateBlocksToBranchDoc(doc || [])
  );
  if (!chapterNeedsSectionCollapse(d)) {
    return { doc: d, changed: false };
  }

  const blocks = activePathBlocks(d);
  /** @type {string[]} */
  const textParts = [];
  /** @type {string[]} */
  const digestParts = [];
  /** @type {object|null} */
  let lastGen = null;
  for (const b of blocks) {
    const t = String(b.text ?? "").trim();
    if (t) textParts.push(t);
    if (b.type === "gen") {
      lastGen = b;
      const dg = String(b.digest ?? "").trim();
      if (dg) digestParts.push(dg);
    }
  }
  const mergedText = textParts.join("\n\n");
  const mergedDigest = digestParts.join("\n\n");

  if (!lastGen) {
    return {
      doc: migrateBlocksToBranchDoc([createPlainBlock(mergedText)]),
      changed: true,
    };
  }

  const mergedBlock = createGenBlock(
    {
      id: lastGen.id,
      ts: lastGen.ts,
      task: lastGen.task,
      model: lastGen.model,
      chars: [...mergedText].length,
      tokens: lastGen.tokens,
      cost: lastGen.cost,
      usageSource: lastGen.usageSource,
      instruction: lastGen.instruction,
      sources: lastGen.sources,
      digest: mergedDigest,
    },
    mergedText
  );
  if (lastGen.key) mergedBlock.key = lastGen.key;

  const newDoc = emptyBranchDoc();
  const variant = variantFromGenBlock(mergedBlock, { label: "变体1" });
  const nodeId = cryptoRandomId();
  newDoc.nodes.push({
    id: nodeId,
    parentId: null,
    fromVariantId: null,
    activeVariantId: variant.id,
    variants: [variant],
    trailingPlains: [],
  });
  return { doc: normalizeBranchDoc(newDoc), changed: true };
}
