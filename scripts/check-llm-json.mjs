/**
 * 模型 JSON 修复：漏逗号 / 拖尾逗号 / 围栏
 * 代码路径: kk_novel_ai/scripts/check-llm-json.mjs
 */
import { parseLlmJson, salvageChapterObjects } from "../src/utils/llmJson.js";
import { parseOutlineToChapters, estimateSplitMaxTokens } from "../src/utils/outlineChapters.js";

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

const nl = parseLlmJson(`{
  "reason": "按天拆",
  "chapters": [
    {
      "title": "第1章 客厅",
      "summary": "第一天。
妈妈让kk跪下。"
    }
  ]
}`);
assert(nl.chapters[0].title === "第1章 客厅", "字符串内未转义换行应修好");
assert(String(nl.chapters[0].summary).includes("第一天"), "summary 应保留");

const truncated = `{
  "reason": "七天",
  "chapters": [
    { "title": "第1章 扇打", "summary": "客厅扇打并口交" },
    { "title": "第2章 后穴", "summary": "插入后穴内射"
`;
const salvaged = salvageChapterObjects(truncated);
assert(salvaged.length >= 1, `截断 JSON 应捞出至少一章，实际 ${salvaged.length}`);
assert(salvaged[0].title === "第1章 扇打", "捞出的第一章标题");

const missingCommaCh = parseOutlineToChapters(`{
  "reason": "ok"
  "chapters": [
    { "title": "第1章 A", "summary": "冲突A" }
    { "title": "第2章 B", "summary": "冲突B" }
  ]
}`);
assert(missingCommaCh.chapters.length === 2, `拆章漏逗号应修好，实际 ${missingCommaCh.chapters.length}`);
assert(missingCommaCh.chapters[1].title === "第2章 B", "第二章标题");

assert(estimateSplitMaxTokens("短") >= 4096, "短大纲也要有拆章下限");
assert(estimateSplitMaxTokens("字".repeat(8000)) >= 4800, "长大纲应抬高 max_tokens");

const emptyPlan = parseOutlineToChapters("");
assert(emptyPlan.errorKind === "empty", "空回包应标记 empty");

console.log("llm-json ok");
