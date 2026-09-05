/**
 * 关系网力导向布局（对照 Obsidian 图谱：d3-force）
 * 代码路径: kk_novel_ai/src/utils/relationForceLayout.js
 */
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  forceX,
  forceY,
} from "d3-force";

export const GRAPH_WORLD = { width: 900, height: 560 };

export const DEFAULT_FORCES = {
  center: 0.12,
  charge: -160,
  linkDistance: 72,
  collidePad: 10,
};

export function estimateLabelWidth(label) {
  const s = String(label || "");
  let w = 0;
  for (const ch of s) {
    w += /[\u4e00-\u9fff]/.test(ch) ? 11 : 7;
  }
  return Math.min(Math.max(w, 16), 132);
}

export function nodeRadius(degree) {
  return 8 + Math.min(14, Math.sqrt(Math.max(degree, 1)) * 4);
}

export function graphSignature(edges) {
  return (edges || [])
    .map((e) => `${e.id}:${e.from_id}:${e.to_id}:${e.strength ?? ""}`)
    .join("|");
}

export function buildRelationGraph(edges, loreItems) {
  const loreById = new Map((loreItems || []).map((x) => [x.id, x]));
  const degree = new Map();
  const ids = new Set();
  const links = [];
  for (const e of edges || []) {
    if (!e || !e.from_id || !e.to_id) continue;
    ids.add(e.from_id);
    ids.add(e.to_id);
    degree.set(e.from_id, (degree.get(e.from_id) || 0) + 1);
    degree.set(e.to_id, (degree.get(e.to_id) || 0) + 1);
    const str = Number(e.strength);
    links.push({
      id: e.id,
      source: e.from_id,
      target: e.to_id,
      kind: e.kind || "",
      label: e.label || e.kind || "",
      strength: Number.isFinite(str) ? str : 3,
    });
  }
  const nodes = [...ids].map((id) => {
    const lore = loreById.get(id);
    const label = (lore && lore.title) || String(id).slice(0, 8);
    const deg = degree.get(id) || 0;
    const r = nodeRadius(deg);
    return {
      id,
      label,
      degree: deg,
      r,
      collideR: r + estimateLabelWidth(label) / 2 + 6,
    };
  });
  return { nodes, links };
}

export function placeOnCircle(nodes, cx, cy, radius) {
  const n = Math.max(nodes.length, 1);
  const r = radius || Math.max(90, 26 * Math.sqrt(n));
  nodes.forEach((node, i) => {
    if (Number.isFinite(node.x) && Number.isFinite(node.y)) return;
    const ang = (Math.PI * 2 * i) / n - Math.PI / 2;
    node.x = cx + r * Math.cos(ang);
    node.y = cy + r * Math.sin(ang);
  });
}

export function mergeNodePositions(prevNodes, nextNodes) {
  const prev = new Map((prevNodes || []).map((n) => [n.id, n]));
  for (const n of nextNodes) {
    const p = prev.get(n.id);
    if (!p) continue;
    n.x = p.x;
    n.y = p.y;
    n.vx = p.vx;
    n.vy = p.vy;
    if (p.fx != null) n.fx = p.fx;
    if (p.fy != null) n.fy = p.fy;
  }
}

export function chargeForCount(baseCharge, n) {
  if (n <= 3) return Math.max(baseCharge * 0.28, -48);
  if (n <= 8) return baseCharge * 0.55;
  if (n >= 40) return baseCharge * 1.2;
  return baseCharge;
}

export function createRelationSimulation(nodes, links, opts = {}) {
  const width = opts.width || GRAPH_WORLD.width;
  const height = opts.height || GRAPH_WORLD.height;
  const forces = { ...DEFAULT_FORCES, ...(opts.forces || {}) };
  const cx = width / 2;
  const cy = height / 2;
  const charge = chargeForCount(forces.charge, nodes.length);
  placeOnCircle(nodes, cx, cy, opts.circleRadius);

  const sim = forceSimulation(nodes)
    .force("charge", forceManyBody().strength(charge).distanceMax(480))
    .force(
      "link",
      forceLink(links)
        .id((d) => d.id)
        .distance(forces.linkDistance)
        .strength((l) => 0.12 + (Number(l.strength) || 3) * 0.07)
    )
    .force("center", forceCenter(cx, cy).strength(forces.center))
    .force("x", forceX(cx).strength(forces.center * 0.35))
    .force("y", forceY(cy).strength(forces.center * 0.35))
    .force(
      "collide",
      forceCollide()
        .radius((d) => (d.collideR || d.r || 12) + (forces.collidePad || 0))
        .iterations(2)
    )
    .alpha(1)
    .alphaDecay(0.022)
    .velocityDecay(0.36);

  if (opts.onTick) sim.on("tick", opts.onTick);
  return sim;
}

export function applyForces(sim, forces, width, height) {
  if (!sim) return;
  const cx = (width || GRAPH_WORLD.width) / 2;
  const cy = (height || GRAPH_WORLD.height) / 2;
  const f = { ...DEFAULT_FORCES, ...forces };
  const nodes = sim.nodes() || [];
  const charge = chargeForCount(f.charge, nodes.length);
  const many = sim.force("charge");
  const link = sim.force("link");
  const center = sim.force("center");
  const fx = sim.force("x");
  const fy = sim.force("y");
  const collide = sim.force("collide");
  if (many) many.strength(charge);
  if (link) link.distance(f.linkDistance);
  if (center) center.x(cx).y(cy).strength(f.center);
  if (fx) fx.x(cx).strength(f.center * 0.35);
  if (fy) fy.y(cy).strength(f.center * 0.35);
  if (collide) {
    collide.radius((d) => (d.collideR || d.r || 12) + (f.collidePad || 0));
  }
  sim.alpha(0.5).restart();
}

export function clearPins(nodes) {
  for (const n of nodes || []) {
    n.fx = null;
    n.fy = null;
  }
}

export function endpoint(ref) {
  if (ref && typeof ref === "object") return ref;
  return null;
}
