/**
 * 按纲快照可用性
 * 代码路径: kk_novel_ai/scripts/check-outline-snapshot.mjs
 */
import {
  bodyHasSubstantialProse,
  isDumpSnapshot,
  snapshotLooksValid,
} from "../src/utils/outlineSnapshot.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg);
}

const dump =
  "（写后总结过长或复读正文，已丢弃。下一章以正文时间地点为准，勿回拨用餐或改亲属。）";
assert(isDumpSnapshot(dump), "应识别占位句");
assert(!snapshotLooksValid(dump), "占位句不得当合格快照");

const ok =
  "第三天早晨，kk在客厅被鞋跟插入尿道，妈妈跨坐令其舔后穴。结束后令其洗澡，答明天再说。主导与顺从延续。";
assert(snapshotLooksValid(ok), "正常总结应合格");

assert(!bodyHasSubstantialProse("", 80), "空正文不合格");
assert(!bodyHasSubstantialProse("短", 80), "过短不合格");
assert(bodyHasSubstantialProse("字".repeat(80), 80), "80 字应合格");

console.log("outline snapshot checks ok");
