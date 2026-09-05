/**
 * 对话页状态（切页不丢输入草稿）
 * 代码路径: kk_novel_ai/src/stores/chatState.js
 */
import { reactive } from "vue";

export const chatState = reactive({
  /** novel | free */
  mode: "free",
  includeChapterBody: false,
  draft: "",
  /** @type {Array<{role:string, content:string}>} */
  messages: [],
  busy: false,
  requestId: "",
  error: "",
  loadedKey: "",
});
