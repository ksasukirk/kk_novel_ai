/**
 * 应用状态
 * 代码路径: kk_novel_ai/src/stores/appState.js
 */
import { reactive } from "vue";

export const appState = reactive({
  activeNav: "project",
  /** 知识库页内子页：home | entities | story | corpus */
  kbSubNav: "home",
  settings: null,
  llmOnline: false,
  llmModel: "",
  projectRoot: "",
  project: null,
  chapterId: "",
  chapterContent: "",
  /** @type {Array<{key?:string,type:string,text:string,id?:string,ts?:string,task?:string,model?:string,chars?:number,tokens?:number|null,cost?:number|null,usageSource?:string}>} */
  chapterBlocks: [],
  /**
   * 章内分支文档 format2；null 表示尚未加载
   * @type {null|{format:number,nodes:Array,plains:Array}}
   */
  chapterBranchDoc: null,
  /**
   * 草稿落点：variant=同节点新变体；fork=从当前变体岔开子节点；空=普通续写/重写
   * @type {""|"variant"|"fork"}
   */
  draftBranchMode: "",
  /** 变体/岔开目标节点 id */
  draftBranchNodeId: "",
  /** 岔开所基于的变体 id */
  draftForkFromVariantId: "",
  dirty: false,
  generating: false,
  /** 本轮已流式字数 */
  genStreamChars: 0,
  /** 估算目标字数（由 max_tokens 推导） */
  genTargetChars: 800,
  /** 0–100；生成中为估算，结束瞬为 100 */
  genProgressPct: 0,
  lastRequestId: "",
  previewText: "",
  /** 模型原始全文（截断前，仅日志对照） */
  previewRawText: "",
  lastTruncated: false,
  /** 疑似半截（token 上限或中途打断） */
  lastIncomplete: false,
  lastModelUsed: "",
  /** 最近一次 usage {prompt_tokens,completion_tokens,total_tokens,source} */
  lastUsage: null,
  lastLogId: "",
  lastCostCny: 0,
  /** 最近一次写作注入的设定来源 {items:[{kind,id,title,detail}]} */
  lastContextSources: null,
  /** 插入后滚动到该块 key；编辑器消费后清空 */
  pendingScrollBlockKey: "",
  /**
   * 非 null 时正文滚动区锁在该 scrollTop（生成写入结算中）
   * @type {number|null}
   */
  editorScrollFreezeTop: null,
  /**
   * 生成预览放置：editor=正文区接受/不接；panel=侧栏预览；空=无
   * @type {""|"editor"|"panel"}
   */
  draftPlacement: "",
  /** 本轮草稿对应任务 id */
  draftTask: "",
  /** 润色替换用选区快照 */
  draftSelection: "",
  /** 本轮写入用的指令快照（发给模型，可变体包装） */
  draftInstruction: "",
  /** 落盘到块上的指令（变体时保留原创作指令，不含包装话） */
  draftPersistInstruction: "",
  /** 按纲续写：当前节拍 id */
  draftActiveBeatId: "",
  /** 重写目标生成块 key；有值时 accept 替换该块而非追加 */
  draftRewriteBlockKey: "",
  /**
   * 正文区草稿锚点块 key：有值时草稿嵌在该块内前台流式显示，而非章末
   * （生成变体 / 重写 / 润色 / 岔开）
   */
  draftAnchorBlockKey: "",
  /** 本篇+全局角色（已合并） */
  characterList: [],
  /** @type {Array<{term:string,id:string,entry:any}>} */
  characterNameTerms: [],
  /** id -> entry */
  characterById: {},
  /** 自动抽角色等变更时 +1，侧栏可 watch 刷新 */
  castRevision: 0,
  /** 自动同步总谱后 +1，总谱页 / 大纲导图可 watch 刷新 */
  storyRevision: 0,
  statusMessage: "就绪",
  /** DeepSeek 高峰时段（由 settings_get / 生成启动时更新） */
  deepseekPeakNow: false,
  deepseekPeakNotice: "",
  genLogs: [],
  /** 当前作品目录内生成/保存履历（优先于全局 genLogs 做分析） */
  projectGenLogs: [],
  usageSummary: null,
  /** DeepSeek 余额等；{ ok, total, granted, topped_up, reason? } */
  providerBalance: null,
  /** 分析页作品目录清单（novels 扫描 + 最近列表） */
  analyticsProjects: [],
  aiUndoStack: [],
  /** 外部写入冲突：{ content, saved, chapter_id } | null */
  externalConflict: null,
  /**
   * 打开知识库前暂存的写作作品，离开知识库时可恢复
   * { root, project, chapterId, chapterContent, chapterBlocks } | null
   */
  writingSnapshot: null,
});

export function isKbProject(project) {
  if (!project) return false;
  return project.kind === "knowledge_base" || project.kind === "universal" || project.kind === "character_roster";
}

export function isKnowledgeOpen() {
  return isKbProject(appState.project);
}

/** 打开知识库前若当前是写作作品，先快照 */
export function snapshotWritingIfNeeded() {
  if (appState.project && !isKbProject(appState.project) && appState.projectRoot) {
    appState.writingSnapshot = {
      root: appState.projectRoot,
      project: appState.project,
      chapterId: appState.chapterId,
      chapterContent: appState.chapterContent,
      chapterBlocks: Array.isArray(appState.chapterBlocks)
        ? appState.chapterBlocks.map((b) => ({ ...b }))
        : [],
      chapterBranchDoc: appState.chapterBranchDoc
        ? JSON.parse(JSON.stringify(appState.chapterBranchDoc))
        : null,
    };
  }
}

/** 恢复写作作品快照（若有） */
export function restoreWritingSnapshot() {
  const snap = appState.writingSnapshot;
  if (!snap || !snap.root) return false;
  appState.projectRoot = snap.root;
  appState.project = snap.project;
  appState.chapterId = snap.chapterId || "";
  appState.chapterContent = snap.chapterContent || "";
  appState.chapterBlocks = Array.isArray(snap.chapterBlocks)
    ? snap.chapterBlocks.map((b) => ({ ...b }))
    : [];
  appState.chapterBranchDoc = snap.chapterBranchDoc
    ? JSON.parse(JSON.stringify(snap.chapterBranchDoc))
    : null;
  appState.dirty = false;
  appState.writingSnapshot = null;
  return true;
}
