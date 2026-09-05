/**
 * 旧作品无 storyboard / 无 illustration 时，块模型与现在一致
 * 代码路径: kk_novel_ai/scripts/check-illustration-compat.mjs
 */
import {
  contentFromBlocks,
  createGenBlock,
  createIllustrationBlock,
  createPlainBlock,
  normalizeBlocks,
} from "../src/utils/genBlock.js";
import {
  branchContextText,
  collapseChapterSectionsToWholeChapter,
  chapterNeedsSectionCollapse,
  insertIllustrationAfterGen,
  migrateBlocksToBranchDoc,
  activePathBlocks,
} from "../src/utils/branchModel.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg);
}

const oldBlocks = [
  createPlainBlock("开场。"),
  createGenBlock({ id: "g1", instruction: "续写" }, "她走进雨里。"),
];
const oldNorm = normalizeBlocks(oldBlocks);
assert(
  contentFromBlocks(oldNorm).includes("她走进雨里"),
  "旧块正文应保留"
);
assert(
  !contentFromBlocks(oldNorm).includes("![]("),
  "旧块不应写出 markdown 图"
);

const illus = createIllustrationBlock({
  caption: "雨中",
  rel: "assets/illustrations/ch1/a.png",
  prompt: "rain",
});
const mixed = normalizeBlocks([...oldNorm, illus]);
assert(
  mixed.some((b) => b.type === "illustration" && b.rel.includes("a.png")),
  "normalize 必须识别 illustration，不能降成 plain"
);
assert(!contentFromBlocks(mixed).includes("rain"), "contentFromBlocks 应跳过插图");
assert(contentFromBlocks(mixed).includes("她走进雨里"), "插图不得吃掉正文");

const oldDoc = migrateBlocksToBranchDoc(oldBlocks);
const oldText = branchContextText(oldDoc, "continue", "");
assert(oldText.includes("她走进雨里"), "无插图时上下文与现在一致");

const withIllus = insertIllustrationAfterGen(oldDoc, oldBlocks[1].key, illus);
const ctx = branchContextText(withIllus, "continue", "");
assert(!ctx.includes("rain"), "上下文拼字应跳过插图");
assert(ctx.includes("她走进雨里"), "上下文仍含正文");

const path = activePathBlocks(withIllus);
assert(path.some((b) => b.type === "illustration"), "激活路径应含插图块");

const twoGens = [
  createGenBlock({ id: "g1" }, "第一段。"),
  createIllustrationBlock({
    caption: "图题勿并入",
    rel: "assets/illustrations/x/y.png",
    prompt: "secret-prompt",
  }),
  createGenBlock({ id: "g2" }, "第二段。"),
];
const doc2 = migrateBlocksToBranchDoc(twoGens);
assert(chapterNeedsSectionCollapse(doc2), "两段 gen 应触发合并");
const c2 = collapseChapterSectionsToWholeChapter(doc2);
assert(c2.changed, "应发生合并");
const c2path = activePathBlocks(c2.doc);
assert(c2path.filter((b) => b.type === "gen").length === 1, "合并后只剩一块 gen");
assert(
  c2path.some((b) => b.type === "illustration" && b.rel.includes("y.png")),
  "合并后插图仍在"
);
assert(
  !c2path
    .filter((b) => b.type !== "illustration")
    .some((b) => String(b.text || "").includes("图题勿并入")),
  "caption 不得并进小说"
);
assert(
  contentFromBlocks(c2path).includes("第一段") && contentFromBlocks(c2path).includes("第二段"),
  "合并正文仍在"
);
assert(!contentFromBlocks(c2path).includes("secret-prompt"), "prompt 不得进章节文件");

console.log("illustration-compat ok");
