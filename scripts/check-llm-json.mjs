/**
 * 模型 JSON 修复：漏逗号 / 拖尾逗号 / 围栏
 * 代码路径: kk_novel_ai/scripts/check-llm-json.mjs
 */
import { parseLlmJson } from "../src/utils/llmJson.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg);
}

const missingComma = `{
  "shots": [
    {
      "seq": 1,
      "visual": "林岚站在门口"
    }
    {
      "seq": 2,
      "visual": "kk 回头"
    }
  ]
}`;

const parsed = parseLlmJson(missingComma);
assert(Array.isArray(parsed.shots) && parsed.shots.length === 2, "应补上镜之间的逗号");
assert(parsed.shots[1].seq === 2, "第二镜 seq");

const trailing = parseLlmJson(`{ "shots": [ { "seq": 1, "visual": "a", }, ], }`);
assert(trailing.shots.length === 1 && trailing.shots[0].visual === "a", "应去掉拖尾逗号");

const fenced = parseLlmJson("```json\n{\"shots\":[{\"seq\":1,\"visual\":\"x\"}]}\n```");
assert(fenced.shots[0].visual === "x", "应剥围栏");

const many = [];
for (let i = 1; i <= 20; i += 1) {
  many.push(`    {\n      "seq": ${i},\n      "visual": "镜头${i} 林岚"\n    }`);
}
const bigBroken = `{\n  "shots": [\n${many.join("\n")}\n  ]\n}`;
const big = parseLlmJson(bigBroken);
assert(big.shots.length === 20, `大批漏逗号应全部修好，实际 ${big.shots.length}`);

console.log("llm-json ok");
