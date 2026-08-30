/**
 * 思维导图树形布局（水平展开）
 * 代码路径: kk_novel_ai/src/utils/mindmapLayout.js
 */

/**
 * @typedef {{ id: string, label: string, kind?: string, meta?: string, children?: object[] }} MindNode
 */

/**
 * @param {MindNode} root
 * @param {{ nodeWidth?: number, nodeHeight?: number, gapX?: number, gapY?: number }} [opts]
 * @returns {{ nodes: Array, edges: Array, width: number, height: number }}
 */
export function layoutMindMap(root, opts = {}) {
  const nodeW = opts.nodeWidth ?? 140;
  const nodeH = opts.nodeHeight ?? 36;
  const gapX = opts.gapX ?? 56;
  const gapY = opts.gapY ?? 14;

  function measure(node) {
    const kids = node.children || [];
    if (!kids.length) {
      node._leaf = 1;
      node._span = nodeH;
      return node._span;
    }
    let span = 0;
    for (const c of kids) {
      span += measure(c);
    }
    span += gapY * (kids.length - 1);
    node._span = Math.max(span, nodeH);
    return node._span;
  }

  function place(node, depth, top) {
    const kids = node.children || [];
    const x = 24 + depth * (nodeW + gapX);
    let y;
    if (!kids.length) {
      y = top;
    } else {
      let cursor = top;
      const childYs = [];
      for (const c of kids) {
        place(c, depth + 1, cursor);
        childYs.push(c._y);
        cursor += c._span + gapY;
      }
      y = (childYs[0] + childYs[childYs.length - 1]) / 2;
    }
    node._x = x;
    node._y = y;
    node._depth = depth;
  }

  const tree = structuredClone(root);
  measure(tree);
  place(tree, 0, 24);

  const nodes = [];
  const edges = [];
  let maxX = 0;
  let maxY = 0;

  function walk(node, parent) {
    nodes.push({
      id: node.id,
      label: node.label,
      kind: node.kind || "default",
      meta: node.meta || "",
      x: node._x,
      y: node._y,
      w: nodeW,
      h: nodeH,
    });
    maxX = Math.max(maxX, node._x + nodeW + 40);
    maxY = Math.max(maxY, node._y + nodeH + 40);
    if (parent) {
      edges.push({
        id: `${parent.id}->${node.id}`,
        x1: parent._x + nodeW,
        y1: parent._y + nodeH / 2,
        x2: node._x,
        y2: node._y + nodeH / 2,
      });
    }
    for (const c of node.children || []) {
      walk(c, node);
    }
  }
  walk(tree, null);

  return {
    nodes,
    edges,
    width: Math.max(maxX, 480),
    height: Math.max(maxY, 280),
  };
}

/**
 * 从作品 + 总谱数据组装统一导图根节点
 */
export function buildNovelMindTree({
  title,
  volumes = [],
  chapters = [],
  plot = { arcs: [], promises: [] },
  timeline = { events: [] },
  canon = { facts: [] },
  relations = { edges: [] },
  loreItems = [],
}) {
  const chapterById = Object.fromEntries(chapters.map((c) => [c.id, c]));
  const loreTitle = (id) => {
    const l = loreItems.find((x) => x.id === id);
    return (l && l.title) || (id ? id.slice(0, 8) : "?");
  };

  const outlineChildren = [];
  if (volumes.length) {
    for (const vol of volumes) {
      const chIds = vol.chapter_ids && vol.chapter_ids.length
        ? vol.chapter_ids
        : chapters.map((c) => c.id);
      const volNode = {
        id: `vol:${vol.id}`,
        label: vol.title || "卷",
        kind: "volume",
        meta: vol.arc_goal || vol.arc_summary || "",
        children: chIds
          .map((id) => chapterById[id])
          .filter(Boolean)
          .map((ch) => ({
            id: `ch:${ch.id}`,
            label: ch.title,
            kind: "chapter",
            meta: ch.summary || ch.must_do || "",
            children: (ch.beats || []).map((b, i) => ({
              id: `beat:${ch.id}:${b.id || i}`,
              label: b.title || `节拍${i + 1}`,
              kind: "beat",
              meta: b.purpose || b.conflict || "",
              children: [],
            })),
          })),
      };
      outlineChildren.push(volNode);
    }
  } else {
    outlineChildren.push(
      ...chapters.map((ch) => ({
        id: `ch:${ch.id}`,
        label: ch.title,
        kind: "chapter",
        meta: ch.summary || "",
        children: (ch.beats || []).map((b, i) => ({
          id: `beat:${ch.id}:${b.id || i}`,
          label: b.title || `节拍${i + 1}`,
          kind: "beat",
          meta: "",
          children: [],
        })),
      }))
    );
  }

  const arcNodes = (plot.arcs || []).map((a) => {
    const promises = (plot.promises || [])
      .filter((p) => !p.arc_id || p.arc_id === a.id)
      .map((p) => ({
        id: `promise:${p.id}`,
        label: (p.text || "承诺").slice(0, 28),
        kind: p.status === "open" ? "promise-open" : "promise",
        meta: p.status,
        children: [],
      }));
    return {
      id: `arc:${a.id}`,
      label: `[${a.kind}] ${a.title}`,
      kind: a.kind === "main" ? "arc-main" : "arc",
      meta: `${a.status} ${a.goal || ""}`.trim(),
      children: promises,
    };
  });
  // 未绑定弧的承诺
  const orphanPromises = (plot.promises || [])
    .filter((p) => !p.arc_id || !(plot.arcs || []).some((a) => a.id === p.arc_id))
    .map((p) => ({
      id: `promise:${p.id}`,
      label: (p.text || "承诺").slice(0, 28),
      kind: p.status === "open" ? "promise-open" : "promise",
      meta: p.status,
      children: [],
    }));

  const sortedEvents = [...(timeline.events || [])].sort((a, b) =>
    String(a.story_time).localeCompare(String(b.story_time))
  );
  const timelineNodes = sortedEvents.map((e) => ({
    id: `ev:${e.id}`,
    label: `${e.story_time || "?"} ${e.title}`,
    kind: "event",
    meta: e.summary || e.location || "",
    children: [],
  }));

  const canonNodes = (canon.facts || []).map((f) => ({
    id: `fact:${f.id}`,
    label: (f.text || "事实").slice(0, 32),
    kind: f.locked ? "canon-locked" : "canon",
    meta: f.locked ? "LOCKED" : "",
    children: [],
  }));

  const relNodes = (relations.edges || []).slice(0, 24).map((e) => ({
    id: `edge:${e.id}`,
    label: `${loreTitle(e.from_id)} → ${loreTitle(e.to_id)}`,
    kind: "relation",
    meta: e.label || e.kind,
    children: [],
  }));

  // 角色枝：按 title 去重（本篇优先由调用方 coalesce 后传入亦可）
  const seenChar = new Set();
  const charNodes = [];
  for (const l of loreItems || []) {
    if ((l.kind || "") !== "character") continue;
    const key = (l.title || "").trim() || l.id;
    if (seenChar.has(key)) continue;
    seenChar.add(key);
    charNodes.push({
      id: `char:${l.id}`,
      label: l.title || "角色",
      kind: "character",
      meta: (l.content || "").slice(0, 40),
      children: [],
    });
  }

  return {
    id: "root",
    label: title || "作品",
    kind: "root",
    meta: "Novel OS",
    children: [
      {
        id: "branch:outline",
        label: "大纲",
        kind: "branch",
        meta: `${chapters.length} 章`,
        children: outlineChildren,
      },
      {
        id: "branch:characters",
        label: "角色",
        kind: "branch",
        meta: `${charNodes.length} 人`,
        children: charNodes,
      },
      {
        id: "branch:plot",
        label: "故事线",
        kind: "branch",
        meta: `${(plot.arcs || []).length} 弧`,
        children: [...arcNodes, ...orphanPromises],
      },
      {
        id: "branch:timeline",
        label: "时间线",
        kind: "branch",
        meta: timeline.calendar_note || `${sortedEvents.length} 事`,
        children: timelineNodes,
      },
      {
        id: "branch:canon",
        label: "Canon",
        kind: "branch",
        meta: `${(canon.facts || []).filter((f) => f.locked).length} 锁定`,
        children: canonNodes,
      },
      {
        id: "branch:relations",
        label: "关系",
        kind: "branch",
        meta: `${(relations.edges || []).length} 边`,
        children: relNodes,
      },
    ],
  };
}
