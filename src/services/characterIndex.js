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

export async function refreshCharacterNameIndex() {
  if (!appState.projectRoot) {
    appState.characterList = [];
    appState.characterNameTerms = [];
    appState.characterById = {};
    return null;
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
      return { list, terms, byId };
    } catch (e) {
      appState.characterList = [];
      appState.characterNameTerms = [];
      appState.characterById = {};
      throw e;
    } finally {
      loading = null;
    }
  })();
  return loading;
}
