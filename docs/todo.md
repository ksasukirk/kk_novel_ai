# 实现 TODO

| # | 里程碑 | 状态 | 关键路径 |
|---|---|---|---|
| M1 | LM Studio settings / health / models / stream | 完成 | `src-tauri/src/llm/*`, `src/views/SettingsView.vue` |
| M2 | 作品目录与编辑器 | 完成 | `src-tauri/src/project/*`, `src/views/EditorView.vue`, `ProjectHome.vue` |
| M3 | 四类写作任务 + AiPanel | 完成 | `src-tauri/src/writing/*`, `prompts/*`, `src/components/AiPanel.vue` |
| M4 | 设定召回 + 章摘要 memory | 完成 | `writing/retrieve.rs`, `project` memory, `LoreView.vue`, `OutlineView.vue` |
| M5 | TXT 导出 + 生成日志 + 文档 | 完成 | `export/*`, `genlog.rs`, `GenLogView.vue`, `docs/*` |
| M6 | 外部大模型 CLI / RPC（已整合进主程序） | 完成 | `src-tauri/src/cli.rs`, `src-tauri/src/main.rs`, `api.rs` `dispatch_rpc` |
| M7 | CLI 驱动运行中 GUI（本机 IPC） | 完成 | `src-tauri/src/ipc/mod.rs`, `gui_writing.rs`, `src/services/guiBridge.js` |
| M8 | 多模型分槽 + 外部脏冲突 | 完成 | `settings.rs`, `SettingsView.vue`, `writing/mod.rs`, `guiBridge.js`, `App.vue` |
| M9 | Diff / AI Undo + Ctrl+K 行内 | 完成 | `AiPanel.vue`, `aiUndo.js`, `lineDiff.js`, `EditorView.vue` |
| M10 | Lore links/attrs + Embedding RAG | 完成 | `project/mod.rs`, `LoreView.vue`, `rag/*`, `retrieve.rs` |
| M11 | 码字统计 + EPUB | 完成 | `stats.json` / `project` stats, `ProjectHome.vue`, `export/mod.rs` |
| M12 | Novel OS：故事线 + 本章焦点 + prompt | 完成 | `story/mod.rs` plot, `ChapterMeta` 焦点, `StoryView.vue`, `continue_chapter.md` |
| M13 | 时间线 + Canon + story_sync | 完成 | `timeline.json` / `canon.json`, `story_sync` 任务, AiPanel 确认 patch |
| M14 | 关系拓扑 + 邻居召回 | 完成 | `relations.json`, SVG 拓扑, lore.links 同步, assemble 邻居 |
| M15 | 节拍 / 卷弧 / 仪表盘 / docs | 完成 | beats, VolumeMeta 弧, `ProjectHome` 仪表盘, CLI `story` |
| M16 | TXT 导入（=== 切章 + 批量建章） | 完成 | `src-tauri/src/import/mod.rs`, CLI `import txt`, `ProjectHome.vue` |
| M17 | 知识库蒸馏（lore_extract + 别名合并 + jobs） | 完成 | `prompts/lore_extract.md`, `import distill`, `apply none/auto` |
| M18 | CLI 流水线 + 问道红尘验收文档 | 完成 | `docs/lmstudio.md`, `scripts/test_import_wendao.ps1` |
| M19 | 知识库 kind + 导入改道 + registry | 完成 | `project.kind`, `import`, `kb_registry.json`, `settings.recent_knowledge_bases` |
| M20 | 侧栏知识库入口 + KnowledgeHome | 完成 | `KnowledgeHome.vue`, `App.vue`, 作品页移除误导入 |
| M21 | 通用知识库聚合 sync | 完成 | `src-tauri/src/kb/mod.rs` |
| M22 | 写作挂接 / CLI kb / migrate / docs | 完成 | `writing` linked_kb, `cli kb *` |
| M23 | 写作抑复读：penalty + CLI 透传 + 段落截断 | 完成 | `llm/mod.rs`, `settings.rs`, `writing/dedupe.rs`, `cli.rs`, `continue_chapter.md` |
| M24 | 流式容错 / 模型回退 / 复读重试 / 主宾规则 | 完成 | `llm/mod.rs` stream fallback, `writing` WritingOutcome, `continue_chapter.md` |
| M25 | SenseNova OpenAI 兼容接入 + disable_thinking | 完成 | `settings.disable_thinking`, `llm` chat_body `thinking.type=disabled` |
| M26 | DeepSeek 官方 API：默认关思考链，避免长写作 content 空 | 完成 | `settings.resolve_disable_thinking` 含 `deepseek.com`；长篇试写 `outputs/deepseek_long_test` |
| M27 | GUI 续写空正文：settings 默认 + content 空强制关 thinking 重试；打包 0.1.9 | 完成 | `SettingsView.vue`, `llm/mod.rs` chat_stream, `dist/kk_novel_ai_0.1.9.exe` |
| M28 | 复读截断误留短台词（预览有字、定稿只剩「张嘴」） | 完成 | `writing/dedupe.rs`, `llmClient.js`, `guiBridge.js`；打包见 0.1.10 |
| M29 | 完整写作优化：禁止清单/节拍状态/方向锚点/pro 路由/raw·定稿双栏/genlog 全文 | 完成 | `writing/advance.rs`, `continue_chapter.md`, `settings`, `genlog`, `AiPanel`, `dist/kk_novel_ai_0.1.11.exe` |
| M30 | 写作区字体/字号选择，默认黑体 | 完成 | `editorTypography.js`, `SettingsView.vue`, `EditorView.vue`, `settings.rs` |
| M31 | 插入章末防半截：生成中禁用、半截确认 | 完成 | `AiPanel.vue`, `previewText.js`（定稿/原始双插已由 M34 统一） |
| M32 | 生成进度条（流式字数估算 + 顶栏/AI 面板） | 完成 | `GenProgressBar.vue`, `genProgress.js`, `guiBridge.js`, `PageHeader.vue` |
| M33 | 全局角色仓 + unique 可改 + 设定分栏 | 完成 | `character_roster`/`@characters`, `LoreEntry.unique`, `LoreView.vue`, `writing` coalesce |
| M34 | 统一插入 + 生成 UI 分块 + usage/计费累计 | 完成 | `ChapterBlockEditor`, `genBlock.js`, `.genblocks` sidecar, `usage.rs` |
| M35 | 写作侧栏树形目录：章根 + 生成块小节索引 | 完成 | `EditorView.vue`, `genBlock.js` (`genBlocksToc`), `projectClient.js` (`peekChapterBlocks`), `ChapterBlockEditor.vue` (`scrollBlockIntoView` force) |
| M36 | 生成写回正文滚动闪跳：冻结 scrollTop + 去掉草稿锚点 | 完成 | `draftAccept.js` `withEditorScrollFrozen`, `appState.editorScrollFreezeTop`, `ChapterBlockEditor` / `EditorView` / `projectClient.saveChapter` |
| M37 | 抑制「不是A是B」否定对照套话 | 完成 | `continue_chapter.md` / `polish.md` 硬规则；`project` 默认 style；`advance.rs` 近期「不是」≥3 动态禁 |
| M38 | 规定字数主控：max_tokens 始终与规定字数对齐 | 完成 | `settings.writing_target_chars`、`resolve_writing_max_tokens`、`continue_chapter` 篇幅、`SettingsView` |
| M39 | 预览阅读中不打断：滚动空闲再写入 + 软冻结 | 完成 | `draftAccept.js` defer/soft-freeze、`EditorView` scroll、`EditorDraftPreview` 提示 |
| M40 | 块级蒸馏记忆：生成后摘要 → memory → 下轮续写 | 完成 | `block_digest`、`MemoryStore.block_notes`、`blockDigest.js`、`writing_auto_digest` |
| M41 | 本段记忆：去婴儿词 + 可编辑写回 | 完成 | `digest_sanitize.rs`、`blockDigestSanitize.js`、`ChapterBlockEditor`、`memory_upsert_block_note` |
| M42 | 块标题条：删除/重新生成 + 复制指令 | 完成 | `ChapterBlockEditor.vue`（`block-sticky-actions`、`onCopyInstruction`） |
| M43 | 侧栏目录生成块条目可删除 | 完成 | `EditorView.vue` → `deleteTocBlock` / `.toc-block-del` |
| M44 | 全局自定义确认弹窗（禁用系统 confirm） | 完成 | `confirmDialog.js`、`ConfirmDialog.vue`、`App.vue` |
| M45 | 取消生成：流式中可取消，进度随停 | 完成 | `gui_writing` `llm-start`、`guiBridge`、`cancelGeneration` |
| M46 | 删除不需确认开关（默认开） | 完成 | `skip_delete_confirm`、`appConfirmDelete`、SettingsView |
| M47 | 目录激活项随浏览块滚动联动 | 完成 | `EditorView.vue` → `syncActiveBlockFromScroll` |
| M48 | 本段记忆完整展开无滚动条 | 完成 | `ChapterBlockEditor.vue` → `autoSizeDigest` |
| M49 | 生成后自动添加本篇新角色 | 完成 | `cast_extract`、`castExtract.js`、`writing_auto_cast` |
| M50 | 修辞口癖清洗 + 块标题润色浮动指令 | 完成 | `rhetoric.rs`、`polishGenBlock`、`ChapterBlockEditor` 浮动框 |
| M51 | 按纲续写引擎：节拍进度 sidecar / beat_engine / 跨章队列 / prompt 加固 | 完成 | `writing/beat_engine.rs`、`outlineQueue.js`、`outline_to_beats.md`、`AiPanel.vue` |
| M52 | 跨章连续阅读 + 目录滚动激活 | 完成 | `EditorView.vue`、`ContinuousChapterRead.vue`、`editorReadingProgress.js` |
| M53 | 作品卡片 AI 生成书名 | 完成 | `project_suggest_title` / `project_apply_title`、`suggest_book_title.md`、`ProjectHome.vue` |

### M53 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 汇总大纲/章纲/记忆/正文摘录起书名 | 完成 | `src-tauri/src/api.rs` → `build_book_title_seed` / `project_suggest_title` |
| 2 | 确认后写入 project.json + 最近列表标题 | 完成 | `api.rs` → `project_apply_title`；`projectClient.js` |
| 3 | 提示词 | 完成 | `src-tauri/prompts/suggest_book_title.md` |
| 4 | 卡片右上角「AI」按钮（不进打开作品） | 完成 | `src/views/ProjectHome.vue` → `onSuggestTitle` |
| 5 | Tauri / CLI RPC 注册 | 完成 | `commands.rs`、`lib.rs`、`api.rs` `dispatch_rpc` |

### M52 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 全书章叠入同一 `.editor-scroll`，邻章只读 / 当前章可编 | 完成 | `src/views/EditorView.vue`、`src/components/ContinuousChapterRead.vue` |
| 2 | 预载 `peekChapterBlocks` 邻章正文缓存 | 完成 | `EditorView.vue` → `preloadChapterBodies` / `chapterBodyCache` |
| 3 | 滚动 spy：焦点章 `tocFocusChapterId` + 小节 `activeBlockKey` | 完成 | `EditorView.vue` → `syncActiveBlockFromScroll` |
| 4 | 目录高亮跟阅读焦点（非仅编辑章） | 完成 | `EditorView.vue` TOC `active` 条件 |
| 5 | 阅读进度改为相对章顶，避免叠章串位 | 完成 | `EditorView.vue` → `captureRelativeChapterProgress` |
| 6 | 目录章节行删除按钮 | 完成 | `EditorView.vue` → `deleteTocChapter` / `.toc-op-del` |
| 7 | 删光章节后仍可「按纲生成」（自动占位章） | 完成 | `bookOutlineQueue.js` → `ensureChapterContext`；`AiPanel.vue` `onRun` |
| 8 | 拆章标题贴事件、禁气氛套话 | 完成 | `src-tauri/prompts/outline_to_chapters.md`、`docs/lmstudio.md` §3.0.1 |
| 9 | 浮条按纲/生成中 UI 压矮 | 完成 | `AiPanel.vue`（去芯片行、脚栏操作、compact 进度、生成中收起扩展） |
| 10 | 目录显示生成中小节 + 旋转图标 | 完成 | `EditorView.vue` → `tocRowsForChapter` / `.toc-gen-spin` |
| 11 | 跨章衔接 + 性别锁 | 完成 | `project/mod.rs` 记忆重建/快照；`writing/mod.rs` 上章收束+人称锁；`continue_chapter.md` / `outline_to_beats.md`；`outlineQueue.js` 章末快照 |

### M51 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | ChapterBeatProgress sidecar + API | 完成 | `src-tauri/src/project/mod.rs`、`commands.rs`、`api.rs` |
| 2 | beat_engine 状态机 | 完成 | `src-tauri/src/writing/beat_engine.rs`、`advance.rs` |
| 3 | WritingRequest.active_beat_id + volume_arc | 完成 | `src-tauri/src/writing/mod.rs`、`continue_chapter.md` |
| 4 | outline_to_beats 任务 | 完成 | `src-tauri/prompts/outline_to_beats.md`、`writing/mod.rs` |
| 5 | 按纲续写跨章队列 | 完成 | `src/services/outlineQueue.js`、`draftAccept.js`、`blockDigest.js` |
| 6 | AI 面板 / 总谱进度 UI | 完成 | `src/components/AiPanel.vue`、`src/views/StoryView.vue` |
| 7 | length_fill / section_plan / context 截断 | 完成 | `length_fill.md`、`section_plan.md`、`writing/mod.rs` |
| 8 | 文档 | 完成 | `docs/lmstudio.md`、`docs/todo.md` |

### M50 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 定稿 sanitize「不是A是B」 | 完成 | `src-tauri/src/writing/rhetoric.rs`、`writing/mod.rs`、`writing_strip_rhetoric` |
| 2 | prompt/动态 ban 改禁止 | 完成 | `continue_chapter.md`、`polish.md`、`advance.rs`、`project` 默认 style |
| 3 | 标题条润色 + 浮动指令框 | 完成 | `ChapterBlockEditor.vue`、`draftAccept.polishGenBlock` |
| 4 | 重写也走浮动指令框 | 完成 | 同上 |

### M49 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | cast_extract 任务与 prompt | 完成 | `src-tauri/prompts/cast_extract.md`、`writing/mod.rs` |
| 2 | 前端抽取并 upsert 本篇 | 完成 | `src/services/castExtract.js`、`draftAccept.js` |
| 3 | 设置开关默认开 | 完成 | `settings.rs`、`SettingsView.vue` |
| 4 | 侧栏刷新 | 完成 | `appState.castRevision`、`CastSidePanel.vue` |

### M48 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | digest textarea 按内容撑高 + overflow:hidden | 完成 | `src/components/ChapterBlockEditor.vue` |

### M47 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 编辑区 scroll-spy 更新 activeBlockKey | 完成 | `src/views/EditorView.vue` |
| 2 | 光标所在生成块同步激活 | 完成 | 同上 `onCaret` |
| 3 | TOC 点击短暂抑制 spy | 完成 | `selectBlock` + `suppressTocSpyUntil` |

### M46 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | settings 字段默认 true | 完成 | `src-tauri/src/settings.rs` |
| 2 | appConfirmDelete 统一入口 | 完成 | `src/services/confirmDialog.js` |
| 3 | 各删除点接入 | 完成 | 块/目录/角色/设定/总谱/作品·知识库最近列表 |
| 4 | 设置页开关 | 完成 | `src/views/SettingsView.vue` |

### M45 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 注册后立即 emit llm-start（含 request_id） | 完成 | `src-tauri/src/gui_writing.rs` |
| 2 | 前端同步 lastRequestId / activeRequestId | 完成 | `src/services/guiBridge.js` |
| 3 | cancelGeneration 用进行中 id | 完成 | `src/services/llmClient.js` |

### M44 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | confirm/alert 服务 | 完成 | `src/services/confirmDialog.js` |
| 2 | 弹窗组件挂载 | 完成 | `src/components/ConfirmDialog.vue`、`src/App.vue` |
| 3 | 替换 window.confirm | 完成 | `EditorView.vue`、`ChapterBlockEditor.vue`、`CastSidePanel.vue` |

### M43 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 目录树每条生成块加删除 | 完成 | `src/views/EditorView.vue`（`deleteTocBlock`，复用 `draftAccept.deleteGenBlock`） |

### M42 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 吸顶标题条加删除 / 重新生成 | 完成 | `src/components/ChapterBlockEditor.vue` → `.block-sticky-actions` |
| 2 | 复制生成指令按钮（标题条 + 底部） | 完成 | 同上 `onCopyInstruction` / `blockInstructionText` |

### M41 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | prompt 禁止婴儿词 | 完成 | `src-tauri/prompts/block_digest.md` |
| 2 | Rust 后处理清洗 + append 统一过滤 | 完成 | `src-tauri/src/project/digest_sanitize.rs`、`project/mod.rs`、`writing/mod.rs` |
| 3 | 前端清洗工具 | 完成 | `src/utils/blockDigestSanitize.js`、`src/services/blockDigest.js` |
| 4 | 本段记忆可编辑失焦写回 memory | 完成 | `ChapterBlockEditor.vue`、`memory_upsert_block_note`（`commands.rs`/`api.rs`/`lib.rs`） |

### M40 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | MemoryStore.block_notes + append/rebuild | 完成 | `src-tauri/src/project/mod.rs` |
| 2 | block_digest 任务与 prompt | 完成 | `src-tauri/prompts/block_digest.md`、`writing/mod.rs` |
| 3 | 续写短 recent + 记忆硬规则 | 完成 | `continue_chapter.md`、`writing/mod.rs` |
| 4 | 写块后异步蒸馏 | 完成 | `src/services/blockDigest.js`、`draftAccept.js` |
| 5 | 开关与块卡片「本段记忆/重提炼」 | 完成 | `settings.rs`、`SettingsView.vue`、`ChapterBlockEditor.vue` |

### M35 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 生成块 TOC 文案 / 列表工具 | 完成 | `src/utils/genBlock.js` → `blockTocLabel`, `genBlocksToc` |
| 2 | 只读 peek 他章块（不切编辑章） | 完成 | `src/services/projectClient.js` → `peekChapterBlocks` |
| 3 | 侧栏章根展开 + 小节树 UI | 完成 | `src/views/EditorView.vue` 模板 `.toc-*` |
| 4 | 点击小节强制滚到对应块 | 完成 | `EditorView.selectBlock` + `ChapterBlockEditor.scrollBlockIntoView({force})` + `appState.pendingScrollBlockKey` |
| 5 | 当前章块变更时同步 TOC | 完成 | `EditorView` watch `chapterBlocks` / `syncCurrentToc` |

### M36 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 写回整段冻结 scrollTop（含落盘） | 完成 | `src/services/draftAccept.js` → `withEditorScrollFrozen` |
| 2 | 取消草稿锚点纠偏（会拽上方正文） | 完成 | 同上；accept 各分支不再传 anchor |
| 3 | 快照改到 pushAiUndo 之后、改 DOM 之前 | 完成 | `acceptDraft` 各分支 |
| 4 | 冻结标志供编辑器/TOC 协作 | 完成 | `src/stores/appState.js` → `editorScrollFreezeTop`；`ChapterBlockEditor.vue`；`EditorView.vue` |
| 5 | saveChapter 避免同引用重赋值 | 完成 | `src/services/projectClient.js` → `saveChapter` |

### M37 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 续写硬规则：否定对照限频 | 完成 | `src-tauri/prompts/continue_chapter.md` |
| 2 | 润色同步少用并删并扎堆 | 完成 | `src-tauri/prompts/polish.md` |
| 3 | 新建作品默认文风一句 | 完成 | `src-tauri/src/project/mod.rs` 默认 `style` |
| 4 | 前文已堆「不是」时写入动态禁止清单 | 完成 | `src-tauri/src/writing/advance.rs` → `build_dynamic_ban_list` |

### M38 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 设置字段 writing_target_chars + 保存时同步 max_tokens | 完成 | `src-tauri/src/settings.rs` |
| 2 | 写作请求 max_tokens 按规定字数解析 | 完成 | `src-tauri/src/writing/mod.rs` → `resolve_writing_options` |
| 3 | 续写 prompt 注入约写字数 | 完成 | `src-tauri/prompts/continue_chapter.md` `{{target_chars}}` |
| 4 | 设置页主控「规定字数」 | 完成 | `src/views/SettingsView.vue` |
| 5 | 进度条用规定字数 | 完成 | `src/utils/genProgress.js`、`guiBridge.js` |

### M39 明细 TODO

| # | 项 | 状态 | 路径 |
|---|---|---|---|
| 1 | 用户滚动后 SCROLL_IDLE_MS 再 autoAccept | 完成 | `src/services/draftAccept.js` |
| 2 | 写回冻结不硬拽用户滚动 | 完成 | 同上 `withEditorScrollFrozen` |
| 3 | 编辑器滚动上报 | 完成 | `src/views/EditorView.vue` |
| 4 | 草稿条提示「停滚后写入」 | 完成 | `src/components/EditorDraftPreview.vue` |

## 后续可增强

| # | 项 | 说明 |
|---|---|---|
| E4 | 行内幽灵文本分层渲染 | 换 CodeMirror 等 |
| E5 | sqlite-vec 原生扩展 | 现为 blob + cosine |
| E6 | 关系图拖拽布局 | 现为圆形固定布局 |
| E7 | story_sync 自动建议 diff UI | 现为 JSON 预览确认 |
| E8 | EPUB 导入 | 本期仅 TXT |
| E9 | 蒸馏任务进度条 UI | 现 GenLog + stderr |
| E10 | 跨书同名实体手动合并 | 通用库现按来源分条；角色可用 unique+全局仓 |
| E11 | 复读检测后自动降参重试 | M24 已轻量实现 |
