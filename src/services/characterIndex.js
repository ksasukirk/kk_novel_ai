/**
 * 加载本篇+全局角色名索引到 appState
 * 代码路径: kk_novel_ai/src/services/characterIndex.js
 */
import { appState } from "../stores/appState.js";
import * as project from "./projectClient.js";
import {
  buildCharacterNameIndex,
  coalesceCharacters,
} from "../utils/characterNameIndex.js";

let loading = null;
/** @type {{ root: string, revision: number } | null} */
let lastLoaded = null;

export async function refreshCharacterNameIndex() {
  if (!appState.projectRoot) {
    appState.characterList = [];
    appState.characterNameTerms = [];
    appState.characterById = {};
    lastLoaded = null;
    return null;
  }
  const root = appState.projectRoot;
  const revision = Number(appState.castRevision) || 0;
  if (
    lastLoaded &&
    lastLoaded.root === root &&
    lastLoaded.revision === revision
  ) {
    return {
      list: appState.characterList,
      terms: appState.characterNameTerms,
      byId: appState.characterById,
    };
  }
  if (loading) return loading;
  loading = (async () => {
    try {
      await project.ensureCharactersLink();
      const r = await project.listLoreScoped();
      const list = coalesceCharacters(r || {});
      const { terms, byId } = buildCharacterNameIndex(list);
      appState.characterList = list;
      appState.characterNameTerms = terms;
      appState.characterById = Object.fromEntries(byId);
      lastLoaded = { root, revision };
      return { list, terms, byId };
    } catch (e) {
      appState.characterList = [];
      appState.characterNameTerms = [];
      appState.characterById = {};
      lastLoaded = null;
      throw e;
    } finally {
      loading = null;
    }
  })();
  return loading;
}
