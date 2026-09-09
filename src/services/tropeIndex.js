/**
 * 加载本篇+全局情节/性癖到 appState.tropeList
 * 代码路径: kk_novel_ai/src/services/tropeIndex.js
 */
import { appState } from "../stores/appState.js";
import * as project from "./projectClient.js";
import { isTropeKind } from "../utils/tropeKinds.js";
import { compareLocale } from "../i18n/index.js";

let loading = null;

function normalizeTitle(s) {
  return String(s || "")
    .trim()
    .replace(/^\[.*?\]\s*/, "")
    .toLowerCase();
}

/**
 * 合并本篇优先的情节/性癖列表
 * @param {{ local?: any[], global?: any[] }} scoped
 */
export function coalesceTropes(scoped) {
  const byKey = new Map();
  const push = (row, scope) => {
    const entry = row.entry || row;
    if (!entry || !isTropeKind(entry.kind)) return;
    const key = normalizeTitle(entry.title) || entry.id;
    if (!key) return;
    if (scope === "local" || !byKey.has(key)) {
      byKey.set(key, {
        ...entry,
        scope,
        _root: row.root || entry._root || "",
      });
    }
  };
  for (const row of scoped.local || []) push(row, "local");
  for (const row of scoped.global || []) push(row, "global");
  if (!scoped.local && !scoped.global && Array.isArray(scoped)) {
    for (const row of scoped) push(row, row.scope || "global");
  }
  return [...byKey.values()].sort((a, b) =>
    String(a.title).localeCompare(String(b.title), compareLocale())
  );
}

export async function refreshTropeIndex() {
  if (loading) return loading;
  loading = (async () => {
    try {
      if (!appState.projectRoot) {
        const ens = await project.ensureTropeLibrary();
        const r = await project.listLoreAt(ens.root);
        appState.tropeList = coalesceTropes({
          local: [],
          global: (r.items || []).map((entry) => ({ entry, root: ens.root })),
        });
        return appState.tropeList;
      }
      await project.ensureCharactersLink();
      const r = await project.listLoreScoped();
      appState.tropeList = coalesceTropes(r || {});
      return appState.tropeList;
    } catch (e) {
      appState.tropeList = [];
      throw e;
    } finally {
      loading = null;
    }
  })();
  return loading;
}
