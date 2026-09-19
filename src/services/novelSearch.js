/**
 * 作品页智能搜索：目录卡缓存 + 本地/AI 检索
 * 代码路径: kk_novel_ai/src/services/novelSearch.js
 */
import { invoke } from "./tauri.js";
import { scoreCatalog } from "../utils/novelSearchScore.js";

let catalogItems = [];
let catalogInflight = null;

export function getCachedCatalog() {
  return catalogItems;
}

export async function ensureWorkCatalog(force = false) {
  if (catalogInflight) return catalogInflight;
  catalogInflight = (async () => {
    try {
      const r = await invoke("work_catalog_ensure", { force: !!force });
      catalogItems = Array.isArray(r && r.items) ? r.items : [];
      return catalogItems;
    } finally {
      catalogInflight = null;
    }
  })();
  return catalogInflight;
}

export function localSearchCatalog(query, cards) {
  const list = cards || catalogItems;
  return scoreCatalog(list, query);
}

export async function novelsSearchAi(query) {
  return await invoke("novels_search", { query: String(query || ""), mode: "ai" });
}
