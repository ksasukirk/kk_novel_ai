# LM Studio 与 CLI 接入说明

代码路径索引见文末。

## 1. 启动 LM Studio

1. 打开 LM Studio，加载模型
2. Developer / Local Server → Start Server
3. 默认地址：`http://127.0.0.1:1234/v1`
4. 在本应用「设置」页检测连接并选择模型

## 2. 作品目录结构

```text
MyNovel/
  project.json
  memory.json             # rolling_summary / chapter_snapshots / block_notes（块级蒸馏）
  stats.json
  embeddings.sqlite   # 配置 embedding_model 后生成
  story/
    plot.json         # 故事弧 + 承诺
    timeline.json     # 故事内时间线
    relations.json    # 角色关系边
    canon.json        # 锁定事实
  chapters/*.md
  chapters/.history/  # AI 写入可选快照（最近 20）
  lore/characters/*.json
  lore/world/*.json
  import/jobs/{job_id}/   # 蒸馏任务：per_chapter / pending_* / report.json
```

## 3. GUI 能力

| 视图 | 说明 | 前端路径 |
|---|---|---|
| 作品 | 新建/打开写作作品、叙事仪表盘、码字热力、导出 EPUB；导入请用「知识库」 | `src/views/ProjectHome.vue` |
| **知识库** | **页内**子导航（库列表/实体/关系总谱/语料）；一书一库 + 通用库；不替换全局侧栏 | `src/views/KnowledgeHome.vue` |
| 总谱 | **思维导图**（大纲+角色+故事线+时间线+Canon+关系）+ **右侧角色栏**添加/删除 + 表单编辑 | `src/views/StoryView.vue`, `CastSidePanel.vue`, `MindMapBoard.vue` |
| 大纲 | 结构导图 / 整理成导图 + 卷弧/章纲 + 已写总结 + **右侧角色栏** | `src/views/OutlineView.vue`, `CastSidePanel.vue`, `MindMapBoard.vue` |
| 写作 | 章节树 + 编辑器 + Ctrl+K 行内 + AI 面板 | `src/views/EditorView.vue`, `AiPanel.vue` |
| 设定 | 角色/世界观 + links/attrs | `src/views/LoreView.vue` |
| 日志 | 生成记录 / 提示词 / token·费用累计 / TXT·EPUB | `src/views/GenLogView.vue` |
| 设置 | 写作 / 分析 / Embedding / 单价；重建 RAG | `src/views/SettingsView.vue` |

AI 续写会注入总谱块（plot/timeline/relations/canon/focus/beats/volume_arc）。「同步总谱」任务输出 JSON patch，需在 AI 面板点确认后落盘。

### 3.0.1 按纲生成（全书拆章 + 章内节拍 + 跨章）

- **入口**：
  - 写作页 AI 面板 → **按纲生成**
  - 写作页左侧**目录**即为待写章队列（状态徽章 + 单章「写」/「纲」）
- **正确流程（章节提示词队列）**：
  1. 在「创作提示 / 全书大纲」或底部指令写提示（如「乐乐与表哥的探索，分几章」）
  2. 点 **生成章节队列**（红按钮同义）→ 弹出「确认拆章写入」
  3. 点 **开始写** → 目录写入这些章，并立刻按弹窗里的章节依次写正文
  4. 也可事后在目录改章名 / 点「纲」改章纲，再点目录「写」或「全部按纲写」补写
- **全书大纲**：`project.json` 字段 `book_outline`；写作页与大纲页均可编辑，互相同步；短提示也会落盘为大纲种子
- **章纲 vs 全书大纲**：
  - **章纲** = 单章 `summary`（冲突/推进/钩子）；AI 任务「章纲」预览默认 **写入本章纲**
  - **全书大纲 / 创作提示** = 整书种子；勿把单场章纲误写成 `book_outline`
- **拆章**（`outline_to_chapters`，`split_mode=full`）：拆出 JSON → 确认「开始写」后写入目录为 `status=pending` 并立刻按这些章开写正文；空首章可更新，其余追加
- **章标题**：须为「第N章 + 具体事件」；禁止「隐秘的延续 / 暗流 / 诱惑」等气氛套话；一句纲只按句内动作拆章，禁止为凑三幕编「隐秘循环」段（见 `outline_to_chapters.md`）
- **续拆后续**（`split_mode=append`）：根据已有章标题+摘要衔接，**只追加**新待写章；目录头与面板均有「续拆后续」
- **单章生成**：目录行「写」→ `runOutlineQueue({ stopAfterOneChapter: true })`
- **整队生成**：目录「全部按纲写」或面板「开始按纲生成」→ 跨章队列
- **跨章硬门槛**：每章正文写入后先等块蒸馏，再阻塞跑 `chapter_summary`；成功才写入 `memory.json` 的 `chapter_snapshots` 并标 `outline_complete`，然后才写下章。总结失败则停队列、保留已写正文（重跑只补总结，不清空重写）
- **章纲 vs 写后总结**：`chapters[].summary` 是写前计划；写后总结只进快照 / `rolling_summary`，**禁止覆盖章纲**。下章「上章收束」优先用快照
- **空章裸续写**：本章无 summary 且存在全书大纲时，续写前软拦截，提示先拆章/填章纲
- **正文生成**：每章整章一次 `continue` 写入（不再自动拆拍写）
- **同步记忆**：默认 `writing_outline_run_sync_digest=true`，章内块蒸馏仍保留，但不能替代章末总结
- **思维导图**：大纲页「整理成导图」→ `outline_to_mindmap`，结果写入 `project.json` 的 `outline_mindmap`；已拆章时结构树把章纲要点 / 已写总结挂成子节点
- **代码**：
  - [`src/utils/chapterStatus.js`](../src/utils/chapterStatus.js)（待写/写作中/已完成）
  - [`src/services/bookOutlineQueue.js`](../src/services/bookOutlineQueue.js)（保存大纲 / 拆章 / 续拆 / 建章）
  - [`src/services/outlineQueue.js`](../src/services/outlineQueue.js)（按章生成；写完→章总结→再下章）
  - [`src/services/outlineMindmap.js`](../src/services/outlineMindmap.js)（整理成导图）
  - [`src/utils/outlineMindTree.js`](../src/utils/outlineMindTree.js)（本地拆树 / 选树）
  - [`src/utils/mindmapLayout.js`](../src/utils/mindmapLayout.js)
  - [`src/views/EditorView.vue`](../src/views/EditorView.vue)（目录队列 UI）
  - [`src/components/AiPanel.vue`](../src/components/AiPanel.vue)
  - [`src-tauri/prompts/outline_to_chapters.md`](../src-tauri/prompts/outline_to_chapters.md)
  - [`src-tauri/prompts/outline_to_mindmap.md`](../src-tauri/prompts/outline_to_mindmap.md)
  - [`src-tauri/prompts/chapter_summary.md`](../src-tauri/prompts/chapter_summary.md)
  - [`src-tauri/src/writing/mod.rs`](../src-tauri/src/writing/mod.rs)
  - [`src/views/StoryView.vue`](../src/views/StoryView.vue)
  - [`src/views/OutlineView.vue`](../src/views/OutlineView.vue)

推荐路径：写/贴全书大纲 → 拆成章节 → 确认「开始写」→ 自动按这些章写正文 → 已写若干章后「续拆后续」再确认开写。

蒸馏（知识库）使用 **`analysis_model`**（空则回退 `model`），按章调用 `lore_extract` + `story_sync`（摘要取自 extract.summary），产物在 `import/jobs/`。`story_sync` 失败不阻断落盘。

CLI：`kk_novel_cli story plot get <root>` / `timeline` / `canon` / `relations` / `dashboard` / `apply-patch`。

### 3.0 TXT 导入与知识库蒸馏

导入创建 **`kind=knowledge_base`**（一书一库），**不是**写作工程。侧栏「知识库」入口；通用库在 `%APPDATA%/kk_novel_ai/universal_kb/`。

切章规则：优先行首 `===标题===`；若无匹配则回退「第N章」。

```bash
# 导入为知识库
kk_novel_cli kb import-txt <kb_root> --file "test_files/《问道红尘》.txt" --title "问道红尘"

# 蒸馏 + 自动同步通用库（apply auto）
kk_novel_cli kb distill <kb_root> --from 1 --to 20 --apply auto --resume

kk_novel_cli kb list
kk_novel_cli kb sync <kb_root>
kk_novel_cli kb sync-all
kk_novel_cli kb migrate <old_root> --sync
kk_novel_cli kb universal-dashboard
```

写作作品可在 `project.json` 的 `linked_kb_roots` 挂接路径、`"@universal"` 或 **`"@characters"`（全局角色仓）**。新建小说默认挂接 `@characters`。角色 lore 可设 `unique: true`（或 attrs.unique）：同名跨来源只保留一条，**本篇覆盖全局**。

全局角色仓目录：`%APPDATA%/kk_novel_ai/character_roster/`（人物 `lore/characters` + 背景/世界观 `lore/world`）。**侧栏「设定」在无任何写作工程时也可直接编辑全局仓。**

验收脚本：[`scripts/test_import_wendao.ps1`](../scripts/test_import_wendao.ps1)

实现：[`src-tauri/src/import/mod.rs`](../src-tauri/src/import/mod.rs)、[`src-tauri/src/kb/mod.rs`](../src-tauri/src/kb/mod.rs)，提示词：[`src-tauri/prompts/lore_extract.md`](../src-tauri/prompts/lore_extract.md)

### 3.1 多模型槽

| 字段 | 用途 |
|---|---|
| `model` | 续写 / 润色 / 章纲 |
| `analysis_model` + `analysis_temperature` | 摘要 / 一致性（空则回退 `model`） |
| `embedding_model` | RAG（空则仅关键词召回） |

LM Studio 需加载对应模型；Embedding 走 `/v1/embeddings`。

## 4. 外部大模型 CLI 控制（已整合进主程序）

CLI 与 GUI **同一可执行文件** `kk_novel_ai`（另有控制台入口 `kk_novel_cli`）：

- **无参数** → 启动图形界面（并开启本机 IPC）
- **带子命令** 或 `--cli` → CLI 模式（JSON / RPC / GUI IPC）

实现路径：[`src-tauri/src/cli.rs`](../src-tauri/src/cli.rs)，IPC：[`src-tauri/src/ipc/mod.rs`](../src-tauri/src/ipc/mod.rs)，入口：[`src-tauri/src/main.rs`](../src-tauri/src/main.rs)

构建：

```bash
cd src-tauri
cargo build
# 产物: target/debug/kk_novel_ai.exe 、 target/debug/kk_novel_cli.exe
```

### 4.1 与 GUI 联动（默认）

1. 先启动 GUI（无参数运行主程序）
2. GUI 会在 `%APPDATA%/kk_novel_ai/ipc.json` 写入 `host/port/token`
3. CLI `writing run` **默认走 IPC**：预览区流式更新，行为对齐 AI 面板

```bash
# 推荐：控制台二进制
kk_novel_cli writing run D:/novels/demo <chapter_id> continue --instruction "写一场对决" --stream-stderr

# 生成后写入章末（等同插入章末并保存）
kk_novel_cli writing run D:/novels/demo <chapter_id> continue --apply append --stream-stderr

# GUI 未启动时会报错；旁路直调加 --offline（界面不会动）
kk_novel_cli writing run D:/novels/demo <chapter_id> continue --offline --stream-stderr

# 单次覆盖采样（抑复读）：短续写 + penalty；指定模型失败会自动回退默认写作模型
kk_novel_cli writing run D:/novels/demo <chapter_id> continue --offline --apply append \
  --model huihui-qwen3-vl-30b-a3b-instruct-abliterated \
  --fallback-model qwen3-vl-8b-instruct-abliterated-v2.0 \
  --temperature 0.55 --max-tokens 700 --frequency-penalty 0.7 --presence-penalty 0.3 \
  --instruction "只写下一拍，写完即停，禁止复述前文"
```

全局默认（设置页 / `settings patch`）：`frequency_penalty` 默认 0.55，`presence_penalty` 默认 0.25，`llm_timeout_secs` 默认 600。续写会：流式失败回退非流式、模型失败回退、段落复读截断、截断过短时降参重试一次。返回 JSON 含 `model_used` / `fallback_from` / `truncated` / `loop_retried` / `usage` / `log_id` / `cost_cny`。

**生成区块与计费**：插入章末为编辑器 **UI 分块**（左侧色条 + 块下方显示 task/model/字数/tokens/费用）；吸顶标题条提供 **复制指令 / 润色 / 重新生成 / 删除**（润色与重写会弹出浮动指令框，可取消）；定稿默认清洗「不是A是B」否定对照口癖（`writing_strip_rhetoric`）。正文 `.md` 不写 HTML 注释；分块元数据在 `chapters/.genblocks/{chapter_id}.json`（含可选 `digest` 本段记忆）。旧 `<!-- kk-gen -->` 加载时迁移，导出仍剥壳。提示词与 usage 进 `gen_log.jsonl`，累计 `usage_ledger.json`；设置单价 `price_input_per_1m` / `price_output_per_1m`；RPC `usage_summary`。

**块级蒸馏记忆（M40 / M41）**：`continue` 写入后若设置 `writing_auto_digest`（默认开），后台跑 `task=block_digest`（分析模型，约 200～400 字），写入 `memory.json` 的 `block_notes` 并重建 `rolling_summary`。摘要生成时会去掉婴儿相关词（prompt + `digest_sanitize` 后处理）；块卡片「本段记忆」可编辑，失焦经 `memory_upsert_block_note` 写回。下轮续写以记忆摘要为主、近期原文窗口缩短至约 800 字（无记忆时仍用 `recent_window_chars`）。

**本篇新角色自动添加（M49）**：生成块写入后若 `writing_auto_cast`（默认开），后台跑 `task=cast_extract`，对照本篇+挂接角色名单，把新人物 upsert 到本篇 `lore/characters`（不写全局仓）。CLI 示例：

```bash
kk_novel_cli writing run <root> <chapter_id> cast_extract --offline \
  --selection "……正文……"
```

块记忆 CLI 示例：

```bash
kk_novel_cli writing run <root> <chapter_id> block_digest --offline \
  --instruction "" --stream-stderr
# selection/块正文可通过 JSON request；GUI 会带 block_key
```

IPC 命令：`ping` / `gui_status` / `writing_run` / `llm_cancel` / `preview_apply` / `project_focus`。

### 4.2 其它子命令（stdout 默认 JSON）

```bash
kk_novel_ai tools
kk_novel_ai llm health
kk_novel_ai llm models
kk_novel_ai settings get
kk_novel_ai settings patch --model <model-id> --base-url http://127.0.0.1:1234/v1
kk_novel_ai project create D:/novels/demo --title 演示
kk_novel_ai chapter list D:/novels/demo
kk_novel_ai export txt D:/novels/demo D:/novels/demo/out.txt
kk_novel_ai export epub D:/novels/demo D:/novels/demo/out.epub
kk_novel_ai stats get D:/novels/demo
kk_novel_ai stats set-goal D:/novels/demo 3000
kk_novel_ai rag rebuild D:/novels/demo
kk_novel_ai settings patch --model <chat-id> --analysis-model <chat-id> --embedding-model <embed-id>
kk_novel_ai gen-log --limit 20

# 等价显式前缀
kk_novel_ai --cli llm health
```

加 `--human` 可得到美化 JSON。

### 4.3 外部写入冲突

GUI 打开某章且 `dirty` 时，若 CLI `--apply` 触发 `chapter-external-update`：

- 弹窗二选一：保留本地编辑 / 接受外部覆盖
- 不 dirty：直接重载外部内容

实现：`src/services/guiBridge.js`、`src/App.vue`。

### 4.3 RPC 模式（推荐给外部 Agent）

```bash
kk_novel_ai rpc
```

stdin 每行一个 NDJSON 请求，stdout 每行一个 JSON 响应：

```json
{"cmd":"llm_health"}
{"cmd":"project_open","root":"D:/novels/demo"}
{"cmd":"project_suggest_title","root":"D:/novels/demo"}
{"cmd":"project_apply_title","root":"D:/novels/demo","title":"建议书名"}
{"cmd":"writing_run","request":{"project_root":"D:/novels/demo","chapter_id":"...","task":"continue","instruction":"推进冲突"}}
```

注意：RPC 里的 `writing_run` 仍是进程内旁路（不驱动 GUI）。要驱动界面请用 CLI 子命令（默认 IPC）。

先执行 `kk_novel_ai tools` 获取完整 cmd 清单。

### 4.4 设计说明

- GUI 启动后开启 loopback TCP IPC；CLI 默认转发到 GUI，复用 `llm-chunk` 预览链路
- `--offline` 时 CLI **不依赖 GUI**，直接读写作品目录并调用 LM Studio
- 与 Tauri 命令共用 `src-tauri/src/api.rs` / `gui_writing.rs`
- Release 下 GUI 使用 Windows 子系统；CLI 调试优先用 `kk_novel_cli`
- 适合 Cursor / 其它大模型用 shell tool 调试写作流水线

## 5. 关键代码路径

| 模块 | 路径 |
|---|---|
| 共享 API / RPC | `src-tauri/src/api.rs` |
| Tauri 命令 | `src-tauri/src/commands.rs` |
| LM Studio 客户端 | `src-tauri/src/llm/mod.rs` |
| 作品存储 | `src-tauri/src/project/mod.rs` |
| 写作上下文 | `src-tauri/src/writing/mod.rs` |
| 混合召回 | `src-tauri/src/writing/retrieve.rs` |
| Embedding RAG | `src-tauri/src/rag/mod.rs` |
| TXT / EPUB 导出 | `src-tauri/src/export/mod.rs` |
| Prompt 模板 | `src-tauri/prompts/*.md` |
| CLI（主程序内） | `src-tauri/src/cli.rs` |
| GUI IPC | `src-tauri/src/ipc/mod.rs` |
| 流式写作 emit | `src-tauri/src/gui_writing.rs` |
| 前端 IPC 桥 | `src/services/guiBridge.js` |
| AI Undo | `src/services/aiUndo.js` |
| 主入口 GUI/CLI 分发 | `src-tauri/src/main.rs` |
| 前端壳 | `src/App.vue` |
| 大纲导图 | `src/utils/outlineMindTree.js` / `src/services/outlineMindmap.js` / `src/components/MindMapBoard.vue` |
| TODO 清单 | `docs/todo.md` |
