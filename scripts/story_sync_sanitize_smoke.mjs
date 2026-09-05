/**
 * 总谱 patch 洗净冒烟（人名对 id、丢掉无头边、锁定 Canon）
 * 代码路径: kk_novel_ai/scripts/story_sync_sanitize_smoke.mjs
 */
import { sanitizeStoryPatch, resolveEndpoint } from "../src/services/storySync.js";

const index = {
  byId: {
    "id-kk": { id: "id-kk", title: "kk" },
    "id-lele": { id: "id-lele", title: "乐乐" },
  },
  byTitle: new Map([
    ["kk", "id-kk"],
    ["乐乐", "id-lele"],
  ]),
};

if (resolveEndpoint("乐乐", index) !== "id-lele") throw new Error("人名未对上 id");
if (resolveEndpoint("id-kk", index) !== "id-kk") throw new Error("id 应原样保留");
if (resolveEndpoint("路人甲", index) !== "") throw new Error("未知人名应丢弃");

const clean = sanitizeStoryPatch(
  {
    edges: [
      { from_id: "乐乐", to_id: "kk", kind: "related", label: "表妹", strength: 4 },
      { from_id: "幽灵", to_id: "kk", kind: "related" },
    ],
    facts: [
      { id: "lock-1", text: "不该覆盖", locked: false },
      { id: "", text: "新事实" },
    ],
    events: [{ title: "晚饭", summary: "吃过了" }],
  },
  {
    index,
    lockedFactIds: new Set(["lock-1"]),
    chapterId: "ch-1",
  }
);

if (!clean.edges || clean.edges.length !== 1) throw new Error("无头边未过滤");
if (clean.edges[0].from_id !== "id-lele" || clean.edges[0].to_id !== "id-kk") {
  throw new Error("边端点未解析");
}
if ((clean.facts || []).some((f) => f.id === "lock-1")) throw new Error("锁定 Canon 被覆盖");
if (!clean.facts || clean.facts.length !== 1) throw new Error("新事实应保留");
if (!clean.events || clean.events[0].chapter_ids[0] !== "ch-1") {
  throw new Error("事件未带上本章 id");
}

console.log(JSON.stringify({ ok: true, edges: clean.edges.length, facts: clean.facts.length }, null, 2));
