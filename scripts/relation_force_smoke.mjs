/**
 * 力导向关系网冒烟：多人撑开、少人不出界、加边能热启动
 * 代码路径: kk_novel_ai/scripts/relation_force_smoke.mjs
 */
import {
  buildRelationGraph,
  createRelationSimulation,
  GRAPH_WORLD,
  mergeNodePositions,
} from "../src/utils/relationForceLayout.js";

function minPairDist(nodes) {
  let m = Infinity;
  for (let i = 0; i < nodes.length; i++) {
    for (let j = i + 1; j < nodes.length; j++) {
      m = Math.min(m, Math.hypot(nodes[i].x - nodes[j].x, nodes[i].y - nodes[j].y));
    }
  }
  return m;
}

function inWorld(nodes, pad = 40) {
  return nodes.every(
    (n) =>
      n.x >= -pad &&
      n.y >= -pad &&
      n.x <= GRAPH_WORLD.width + pad &&
      n.y <= GRAPH_WORLD.height + pad
  );
}

function circleMinDist(n, r = 110) {
  if (n < 2) return Infinity;
  return 2 * r * Math.sin(Math.PI / n);
}

function makeEdges(count) {
  const edges = [];
  for (let i = 1; i < count; i++) {
    edges.push({
      id: `e${i}`,
      from_id: `n${Math.max(0, i - 1)}`,
      to_id: `n${i}`,
      strength: 3,
      kind: "related",
      label: "",
    });
  }
  for (let i = 0; i < Math.min(8, count); i++) {
    const j = (i * 7 + 3) % count;
    if (j === i) continue;
    edges.push({
      id: `c${i}`,
      from_id: `n${i}`,
      to_id: `n${j}`,
      strength: 2,
      kind: "related",
      label: "",
    });
  }
  return edges;
}

function loreFor(count) {
  return Array.from({ length: count }, (_, i) => ({ id: `n${i}`, title: `角色${i}` }));
}

function runTicks(sim, n) {
  for (let i = 0; i < n; i++) sim.tick();
}

function main() {
  const dense = 40;
  const { nodes: denseNodes, links: denseLinks } = buildRelationGraph(
    makeEdges(dense),
    loreFor(dense)
  );
  const simDense = createRelationSimulation(denseNodes, denseLinks);
  runTicks(simDense, 180);
  simDense.stop();
  const forceMin = minPairDist(denseNodes);
  const circleMin = circleMinDist(dense);
  if (!(forceMin > circleMin * 1.15)) {
    throw new Error(
      `多人未撑开: forceMin=${forceMin.toFixed(1)} circleChord=${circleMin.toFixed(1)}`
    );
  }

  const { nodes: fewNodes, links: fewLinks } = buildRelationGraph(makeEdges(3), loreFor(3));
  const simFew = createRelationSimulation(fewNodes, fewLinks);
  runTicks(simFew, 120);
  simFew.stop();
  if (!inWorld(fewNodes)) {
    throw new Error(
      `少人飞出画布: ${fewNodes.map((n) => `${n.id}=(${n.x.toFixed(0)},${n.y.toFixed(0)})`).join(" ")}`
    );
  }

  const more = makeEdges(4);
  const built = buildRelationGraph(more, loreFor(4));
  mergeNodePositions(fewNodes, built.nodes);
  const kept = built.nodes.filter((n) => n.id !== "n3");
  if (!kept.every((n) => Number.isFinite(n.x))) {
    throw new Error("加边后旧节点坐标丢失");
  }
  const simMore = createRelationSimulation(built.nodes, built.links);
  runTicks(simMore, 40);
  simMore.stop();
  if (built.nodes.length !== 4) throw new Error("加边后节点数不对");

  console.log(
    JSON.stringify(
      {
        ok: true,
        dense: { n: dense, forceMin: +forceMin.toFixed(1), circleChord: +circleMin.toFixed(1) },
        few: fewNodes.map((n) => ({ id: n.id, x: +n.x.toFixed(1), y: +n.y.toFixed(1) })),
        added: built.nodes.length,
      },
      null,
      2
    )
  );
}

main();
