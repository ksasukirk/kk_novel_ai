# Kk Novel Ai

**Kk Novel Ai** 是面向长篇连载的本地小说创作台：大纲、章节、设定与记忆都落在你自己的磁盘目录里，不绑云端作品库；桌面 GUI 负责写与改，命令行 / 脚本也能驱动同一套写作流水线。

写作、拆章、润色与知识蒸馏推荐接 **DeepSeek** 官方 API（OpenAI 兼容）；也支持本机或其他兼容端点。插图链路可另配 OpenAI 兼容文生图端点。项目会持续听取反馈并迭代体验。

| 项 | 值 |
|---|---|
| 当前版本 | `0.2.31`（`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 同步） |
| 标识符 | `com.kk.kk-novel-ai` |
| 仓库 | [https://github.com/ksasukirk/kk_novel_ai](https://github.com/ksasukirk/kk_novel_ai) |
| 作者 | kk |

文档：模块分析 [`docs/project-analysis.md`](docs/project-analysis.md) · 里程碑 [`docs/todo.md`](docs/todo.md) · 模型与 CLI [`docs/lmstudio.md`](docs/lmstudio.md)

**持续优化**：欢迎在 [Issues](https://github.com/ksasukirk/kk_novel_ai/issues) 提建议，或直接向作者反馈。合理诉求会尽量排进后续迭代（见 [`docs/todo.md`](docs/todo.md) 与本文第 11 节）。

**模型建议**：长篇续写、拆章、润色与蒸馏，优先使用 **DeepSeek**。设置页提供 Flash / Pro / 本机 LM Studio 预设、空闲/高峰单价与缓存友好续写；应用对 `deepseek.com` 默认关闭思考链，避免长写作 `content` 为空（见 [`src-tauri/src/settings.rs`](src-tauri/src/settings.rs)、[`src/views/SettingsView.vue`](src/views/SettingsView.vue)、[`src/utils/deepseekPricing.js`](src/utils/deepseekPricing.js)）。本机 [LM Studio](https://lmstudio.ai/) 等 OpenAI 兼容服务仍可用，配置说明见 [`docs/lmstudio.md`](docs/lmstudio.md)。

**界面与写作语言**：设置页可分别选择 **界面语言** 与 **写作语言**（`zh-CN` / `en` / `ja`）。界面文案走 vue-i18n；写作 Prompt、导出语言与部分用户可见错误按对应 locale 加载（见第 1 节）。

**用量与分析**：侧栏「分析」页展示 DeepSeek 官方余额、本应用累计花费 / token、近 14 天趋势与按模型柱状图；无履历时按当前单价与写作参数做约算（不写假账）。业务 AI 调用会记入全局 `gen_log.jsonl` 与作品目录 `gen_activity.jsonl`（见第 8 节）。

**情节 / 性癖库**：侧栏「情节库」可维护可复用的情节套路与性癖写法卡；本章可在 AI 面板与总谱「本章焦点」多选，续写时强制注入并要求兑现过程；写后可自动抽回全局库（见第 1 节）。

---

## 0. 本文编写 TODO

| # | 项 | 状态 | 主要代码 / 文档路径 |
|---|---|---|---|
| R1 | 产品定位与开篇说明 | 完成 | 本文；[`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) |
| R2 | 技术栈与进程入口 | 完成 | [`package.json`](package.json)、[`src-tauri/Cargo.toml`](src-tauri/Cargo.toml)、[`src-tauri/src/main.rs`](src-tauri/src/main.rs) |
| R3 | 目录结构与前端视图 | 完成 | [`src/App.vue`](src/App.vue)、[`src/views/`](src/views/) |
| R4 | 后端模块与写作流水线 | 完成 | [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs)、[`src-tauri/src/writing/`](src-tauri/src/writing/) |
| R5 | 磁盘数据模型 | 完成 | [`src-tauri/src/project/mod.rs`](src-tauri/src/project/mod.rs)、[`src-tauri/src/paths.rs`](src-tauri/src/paths.rs) |
| R6 | 开发 / 打包 / Android / 发版 | 完成 | [`build.py`](build.py)、[`build_android.py`](build_android.py)、[`docs/android-setup.md`](docs/android-setup.md) |
| R7 | 文档索引与后续建议 | 完成 | [`docs/`](docs/) |
| R8 | 写明持续吸取建议并优化软件 | 完成 | 本文开篇；[`docs/todo.md`](docs/todo.md) |
| R9 | 写明推荐使用 DeepSeek | 完成 | 本文开篇；[`src-tauri/src/settings.rs`](src-tauri/src/settings.rs)、[`src/views/SettingsView.vue`](src/views/SettingsView.vue) |
| R10 | 用量分析 / 作品履历 / 余额 | 完成 | [`UsageAnalyticsView.vue`](src/views/UsageAnalyticsView.vue)、[`project_genlog.rs`](src-tauri/src/project_genlog.rs)、[`llm/balance.rs`](src-tauri/src/llm/balance.rs) |
| R11 | v0.2.20 插图 / 分镜 / 应用内更新 / 发版流水线 | 完成 | [`illustration.js`](src/services/illustration.js)、[`image.rs`](src-tauri/src/image.rs)、[`update.rs`](src-tauri/src/update.rs)、[`build.py`](build.py) |
| R12 | v0.2.21 分镜 JSON 容错与提示词约束 | 完成 | [`llmJson.js`](src/utils/llmJson.js)、[`illustration.js`](src/services/illustration.js)、[`beats_to_storyboard.md`](src-tauri/prompts/beats_to_storyboard.md) |
| R13 | v0.2.22 独立对话页 / 切页保草稿 / KeepAlive | 完成 | 本文第 1 节；[`ChatView.vue`](src/views/ChatView.vue)、[`chat.rs`](src-tauri/src/chat.rs)、[`App.vue`](src/App.vue) |
| R14 | v0.2.23–0.2.27 版本对齐与发版落盘 | 完成 | [`package.json`](package.json)、[`src-tauri/Cargo.toml`](src-tauri/Cargo.toml)、[`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json)、本文 |
| R15 | v0.2.24 角色/设定落盘后跨页刷新同步 | 完成 | [`appState.js`](src/stores/appState.js)、[`CastSidePanel.vue`](src/components/CastSidePanel.vue)、[`LoreView.vue`](src/views/LoreView.vue) |
| R16 | v0.2.28 对话助手人设 + 大纲导图/编辑分栏 | 完成 | [`ChatView.vue`](src/views/ChatView.vue)、[`chatClient.js`](src/services/chatClient.js)、[`OutlineView.vue`](src/views/OutlineView.vue)、[`MindMapBoard.vue`](src/components/MindMapBoard.vue) |
| R17 | v0.2.29 界面/写作多语言 + 拆章 JSON 加固 | 完成 | [`src/i18n/`](src/i18n/)、[`src-tauri/src/i18n.rs`](src-tauri/src/i18n.rs)、[`prompt_i18n.rs`](src-tauri/src/prompt_i18n.rs)、[`SettingsView.vue`](src/views/SettingsView.vue)、[`outlineChapters.js`](src/utils/outlineChapters.js)、[`llmJson.js`](src/utils/llmJson.js) |
| R18 | v0.2.30 按纲写完收束 + 半句补全 + 记忆去脏 | 完成 | [`writing/mod.rs`](src-tauri/src/writing/mod.rs)、[`continuity.rs`](src-tauri/src/writing/continuity.rs)、[`outlineQueue.js`](src/services/outlineQueue.js)、[`outlineSnapshot.js`](src/utils/outlineSnapshot.js)、[`scene_complete.md`](src-tauri/prompts/scene_complete.md) |
| R19 | v0.2.31 情节/性癖库 + 本章勾选注入 + 写后抽取 | 完成 | [`TropeLibraryView.vue`](src/views/TropeLibraryView.vue)、[`tropeExtract.js`](src/services/tropeExtract.js)、[`writing/mod.rs`](src-tauri/src/writing/mod.rs)、[`AiPanel.vue`](src/components/AiPanel.vue)、[`StoryView.vue`](src/views/StoryView.vue)、[`trope_extract.md`](src-tauri/prompts/trope_extract.md) |

---

## 1. 本版（0.2.31）做了什么

相对 `0.2.30`，本版补上「情节 / 性癖库」：可复用的写法卡进全局目录，本章多选后强制注入续写 / 补字 / 拆节拍等 Prompt，要求按卡兑现过程与感官；写完可自动抽回库，蒸馏也能落地同类条目。

- **全局情节库页**：侧栏新增「情节库」，维护 `kind=trope` / `kind=kink` 条目（写法说明、关键词等）；设定页与角色仓不再混入这两类。见 [`src/views/TropeLibraryView.vue`](src/views/TropeLibraryView.vue)、[`src/App.vue`](src/App.vue)、[`src/components/shell/AppSidebar.vue`](src/components/shell/AppSidebar.vue)、[`src/views/LoreView.vue`](src/views/LoreView.vue)、[`src/views/CharacterRosterView.vue`](src/views/CharacterRosterView.vue)。
- **本章双层多选**：AI 面板与总谱「本章焦点」均可勾选情节 / 性癖；勾选立刻写入章节 `trope_ids`，打开章时会 hydrate。见 [`src/components/AiPanel.vue`](src/components/AiPanel.vue)、[`src/views/StoryView.vue`](src/views/StoryView.vue)、[`src/stores/aiPanelState.js`](src/stores/aiPanelState.js)、[`src/stores/appState.js`](src/stores/appState.js)、[`src/utils/tropeKinds.js`](src/utils/tropeKinds.js)、[`src/services/tropeIndex.js`](src/services/tropeIndex.js)、[`src/services/projectClient.js`](src/services/projectClient.js)。
- **生成强制注入**：写作引擎把勾选项填进 `{{tropes}}`，RAG 排除未勾选库条目；续写、同位置变体、补字、场景收束、扩纲、拆节拍等 Prompt（中 / 英 / 日）要求兑现过程，禁止只点名标签。见 [`src-tauri/src/writing/mod.rs`](src-tauri/src/writing/mod.rs)、[`src/utils/genBlock.js`](src/utils/genBlock.js)、[`src-tauri/prompts/`](src-tauri/prompts/)（含 `en/`、`ja/`）。
- **写后自动抽取**：定稿后可按设置 `writing_auto_trope`（默认开）调用 [`trope_extract.md`](src-tauri/prompts/trope_extract.md) 抽情节 / 性癖并入全局库。见 [`src/services/tropeExtract.js`](src/services/tropeExtract.js)、[`src/services/draftAccept.js`](src/services/draftAccept.js)、[`src/views/SettingsView.vue`](src/views/SettingsView.vue)、[`src-tauri/src/settings.rs`](src-tauri/src/settings.rs)。
- **蒸馏与摘要对齐**：知识库蒸馏可落地 trope / kink；块摘要 / 章摘要对照已知库名，只记正文里实际出现的套路。见 [`src-tauri/src/import/mod.rs`](src-tauri/src/import/mod.rs)、[`lore_extract.md`](src-tauri/prompts/lore_extract.md)、[`block_digest.md`](src-tauri/prompts/block_digest.md)、[`chapter_summary.md`](src-tauri/prompts/chapter_summary.md)。
- **存储与 IPC**：`ChapterMeta.trope_ids`、lore 按 kind 分目录、`chapter_update_meta` 可补丁更新。见 [`src-tauri/src/project/mod.rs`](src-tauri/src/project/mod.rs)、[`src-tauri/src/api.rs`](src-tauri/src/api.rs)、[`src-tauri/src/kb/mod.rs`](src-tauri/src/kb/mod.rs)。
- **版本号同步**：[`package.json`](package.json)、[`src-tauri/Cargo.toml`](src-tauri/Cargo.toml)（及 [`Cargo.lock`](src-tauri/Cargo.lock)）、[`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) 均为 `0.2.31`。

**上一版（0.2.30）及更早已交付、本版继续可用的体验**：

- **按纲写完收束**：字数是下限；半句或按纲钩子未出现时用 `scene_complete` 补写；按纲队列拒空章冒充写完、记忆拒脏占位。
- **界面 / 写作多语言**：`zh-CN` / `en` / `ja`；Prompt 与后端用户文案分 locale 加载。
- **拆章 JSON 加固**、对话助手人设、大纲导图 / 编辑分栏、角色 / 设定跨页刷新、侧栏「对话」页与 `KeepAlive`。
- 章内插图与分镜、文生图设置、导出带图、应用内检查更新，以及 `python build.py` 默认发版流水线（见第 9 节）。

---

## 2. 它做什么

- 以**本地作品目录**为真相源：`project.json`、章节 Markdown、lore / memory / 总谱 / 分镜 JSON；对话会话另存（不混进章节）；情节 / 性癖条目与角色 / 世界观分目录存放。
- 续写、润色、拆章、按纲节拍、设定召回、知识库蒸馏，都走同一套 Rust 写作引擎；按纲整章可一次写完本章纲，并自动补半句 / 收束；本章勾选的情节 / 性癖会注入生成。
- 桌面 GUI（Tauri 2 + Vue 3）负责编辑与预览；界面可切中 / 英 / 日；写作 Prompt 可按写作语言独立选择；独立「对话」页做本作 / 自由聊（可配助手人设）；独立「情节库」页维护套路卡；CLI / NDJSON RPC 给脚本编排；GUI 在线时 CLI 默认可经本机 IPC 驱动同一套预览（见 [`src-tauri/src/ipc/mod.rs`](src-tauri/src/ipc/mod.rs)、[`src/services/guiBridge.js`](src/services/guiBridge.js)）。
- Windows 桌面为主，Android APK 由 `build_android.py` 引导工具链后打包（见 [`docs/android-setup.md`](docs/android-setup.md)）。
- **持续迭代**：会吸取更多建议来优化本软件；写作模型**最好使用 DeepSeek**（设置页配置端点与模型槽）。
- **用量可追溯**：续写 / 润色 / 书名建议 / 导入蒸馏等业务 AI 调用记 token 与花费；侧栏「分析」可看余额、KPI 与趋势。
- **插图可选**：分镜与章内插图依赖你配置的文生图端点；不配也能正常写作与导出纯文本。分镜表 JSON 解析已对常见模型瑕疵做容错。
- **角色 / 设定跨页一致**：落盘或自动抽角后，大纲侧栏与总谱 lore 会随 `castRevision` 与页面激活刷新。
- **大纲导图独立浏览**：大纲页可在结构导图与编辑之间切换，导图区可全高浏览。

---

## 3. 技术栈

| 层 | 选型 | 路径 |
|---|---|---|
| 桌面壳 | Tauri 2.11 | [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) |
| 前端 | Vue 3 + Vite 5 | [`package.json`](package.json)、[`vite.config.js`](vite.config.js)、[`src/main.js`](src/main.js) |
| 界面 i18n | vue-i18n 9 | [`src/i18n/index.js`](src/i18n/index.js)、[`src/i18n/locales/`](src/i18n/locales/) |
| 后端 | Rust 2021 | [`src-tauri/src/`](src-tauri/src/) |
| 后端 / Prompt i18n | 嵌入 JSON + 分语言 Prompt | [`src-tauri/src/i18n.rs`](src-tauri/src/i18n.rs)、[`prompt_i18n.rs`](src-tauri/src/prompt_i18n.rs)、[`src-tauri/locales/`](src-tauri/locales/)、[`src-tauri/prompts/`](src-tauri/prompts/) |
| HTTP / 流式 | reqwest + rustls | [`src-tauri/src/llm/mod.rs`](src-tauri/src/llm/mod.rs) |
| 文生图 | OpenAI 兼容 images API | [`src-tauri/src/image.rs`](src-tauri/src/image.rs) |
| CLI | clap 4 | [`src-tauri/src/cli.rs`](src-tauri/src/cli.rs) |
| RAG | rusqlite + 关键词回退 | [`src-tauri/src/rag/mod.rs`](src-tauri/src/rag/mod.rs) |
| 导出 | TXT / EPUB / PDF（zip、krilla，可嵌插图） | [`src-tauri/src/export/mod.rs`](src-tauri/src/export/mod.rs) |
| 应用更新 | GitHub Release 检查 / 下载 | [`src-tauri/src/update.rs`](src-tauri/src/update.rs) |
| LLM JSON 容错 | 前端近似 JSON 修复 + 拆章抢救 | [`src/utils/llmJson.js`](src/utils/llmJson.js)、[`src/utils/outlineChapters.js`](src/utils/outlineChapters.js) |
| 按纲快照校验 | 正文门槛 + 拒脏占位 | [`src/utils/outlineSnapshot.js`](src/utils/outlineSnapshot.js)、[`scripts/check-outline-snapshot.mjs`](scripts/check-outline-snapshot.mjs) |
| 情节 / 性癖库 | lore kind + 本章 `trope_ids` + 写后抽取 | [`TropeLibraryView.vue`](src/views/TropeLibraryView.vue)、[`tropeExtract.js`](src/services/tropeExtract.js)、[`tropeIndex.js`](src/services/tropeIndex.js)、[`writing/mod.rs`](src-tauri/src/writing/mod.rs) |
| 对话会话 | 本地 JSON 落盘（含人设字段） | [`src-tauri/src/chat.rs`](src-tauri/src/chat.rs) |

窗口无系统边框（`decorations: false`），自定义标题栏在 [`src/App.vue`](src/App.vue)；主题 Token 在 [`src/style.css`](src/style.css)。

---

## 4. 架构

```text
Vue (src/)  --invoke / listen-->  Tauri commands.rs  -->  api.rs
                                                      -->  writing/*  -->  llm/*  -->  LM Studio / OpenAI 兼容
                                                      -->  chat.rs    -->  作品 chat/ 或 APPDATA/chat/
                                                      -->  image.rs   -->  文生图端点
                                                      -->  update.rs  -->  GitHub Release
                                                      -->  project/* / story/*  -->  作品目录（含 lore tropes/kinks）
CLI (cli.rs)  --默认 IPC-->  运行中 GUI（ipc/mod.rs + guiBridge.js）
              --offline / rpc-->  同一套 api.rs（不驱动界面）
```

| 二进制 | 路径 | 行为 |
|---|---|---|
| `kk_novel_ai` | [`src-tauri/src/main.rs`](src-tauri/src/main.rs) | 无参启动 GUI；有子命令或 `--cli` 走 CLI |
| `kk_novel_cli` | [`src-tauri/src/bin/kk_novel_cli.rs`](src-tauri/src/bin/kk_novel_cli.rs) | 纯控制台入口，调试 stdout 更稳 |

GUI 装配：[`src-tauri/src/lib.rs`](src-tauri/src/lib.rs)（注册命令、`CancelRegistry` / `PrepareRegistry`，桌面端启动 IPC；含 `chat_session_*`；挂载 `i18n` / `prompt_i18n`）。

设置字段：`ui_locale` / `writing_locale` / `writing_auto_trope`（[`settings.rs`](src-tauri/src/settings.rs)）；前端保存后 `applyUiLocale`（[`src/i18n/index.js`](src/i18n/index.js)）。

写作请求可带 `outline_run` 与本章 `selected_trope_ids`（[`writing/mod.rs`](src-tauri/src/writing/mod.rs)）；按纲队列在 [`outlineQueue.js`](src/services/outlineQueue.js) 置位。前端跨页同步：角色 / 设定用 [`bumpCastRevision()`](src/stores/appState.js)；情节库用 `tropeRevision` / [`refreshTropeIndex`](src/services/tropeIndex.js)。大纲侧栏、总谱、设定页在 watch / `onActivated` 时重载。

---

## 5. 仓库目录

```text
kk_novel_ai/
├── README.md                 # 本文件
├── package.json              # npm 脚本与前端依赖（含 vue-i18n）
├── vite.config.js
├── index.html
├── build.py                  # Windows / 可选 Android 总打包 + 默认发版
├── build_android.py          # Android 工具链引导 + APK
├── build-frontend.mjs        # 前端产物 frontend-dist/
├── src/                      # Vue 3
│   ├── App.vue               # 侧栏 / KeepAlive 导航 / 跟随 ui_locale
│   ├── i18n/                 # vue-i18n 入口与 zh-CN / en / ja 文案
│   ├── views/                # 作品 / 知识库 / 写作 / 对话 / 大纲 / 总谱 / 情节库 / 设定 / 分析 / 日志 / 设置
│   ├── components/           # AiPanel、编辑块、插图对话框、更新对话框、壳、思维导图、CastSidePanel 等
│   ├── services/             # Tauri / LLM / 作品 / 对话 / 插图 / 更新 / GUI 桥 / 抽角色 / 情节抽取 / 按纲队列
│   ├── stores/               # appState（castRevision / tropeRevision）/ aiPanelState / chatState / genJobs 等
│   └── utils/                # 用量 / DeepSeek 单价 / lore 视觉 / LLM JSON / 拆章 / 按纲快照 / 生成块 / tropeKinds
├── src-tauri/                # Tauri + Rust
│   ├── src/                  # 后端模块（含 writing、i18n、prompt_i18n、chat、image、update、import distill）
│   ├── locales/              # 后端用户文案 zh-CN / en / ja
│   ├── prompts/              # 写作 Prompt（含 trope_extract / scene_complete / length_fill；根=zh-CN；en/、ja/）
│   ├── tauri.conf.json
│   └── tauri.android.conf.json
├── scripts/                  # android-setup、发版 README 提示词、LLM JSON / 按纲快照验收等
├── docs/                     # 分析、TODO、LM Studio、Android、移动 QA
└── test_files/               # 导入语料（如《问道红尘》）
```

构建产物不入库：`dist/`、`frontend-dist/`、`outputs/`、`node_modules/`、`src-tauri/target/`（见 [`.gitignore`](.gitignore)）。人设规则目录 `.cursor/rules/` 同样不提交。

---

## 6. 前端视图与关键路径

侧栏定义：[`src/App.vue`](src/App.vue)（作品、知识库、角色定义、总谱、大纲、写作、**对话**、**情节库**、设定、分析、日志、设置；主内容区 `KeepAlive`；文案随 `ui_locale`）。

| 界面 | 路径 | 职责 |
|---|---|---|
| 壳 / 侧栏 / 页眉 | [`src/components/shell/AppSidebar.vue`](src/components/shell/AppSidebar.vue)、[`PageHeader.vue`](src/components/shell/PageHeader.vue)、[`PageBackground.vue`](src/components/shell/PageBackground.vue) | 布局、主题、移动抽屉；i18n 键 |
| 作品 | [`src/views/ProjectHome.vue`](src/views/ProjectHome.vue) | 新建/打开/最近、仪表盘、书名建议 |
| 知识库 | [`src/views/KnowledgeHome.vue`](src/views/KnowledgeHome.vue) | 一书一库、通用库；导入走此页 |
| 写作 | [`src/views/EditorView.vue`](src/views/EditorView.vue)、[`src/components/AiPanel.vue`](src/components/AiPanel.vue)、[`ChapterBlockEditor.vue`](src/components/ChapterBlockEditor.vue) | 章树、块编辑（含插图块）、按纲队列、流式预览；情节多选；指令按任务分槽 |
| 对话 | [`src/views/ChatView.vue`](src/views/ChatView.vue)、[`chatClient.js`](src/services/chatClient.js)、[`chatState.js`](src/stores/chatState.js) | 本作 / 自由聊；助手人设落盘；气泡显示名；不写章节 |
| 大纲 | [`src/views/OutlineView.vue`](src/views/OutlineView.vue)、[`CastSidePanel.vue`](src/components/CastSidePanel.vue)、[`MindMapBoard.vue`](src/components/MindMapBoard.vue) | 结构导图 / 大纲编辑分栏；导图 `fill` 全高；拆章走 `outlineChapters` |
| 总谱 | [`src/views/StoryView.vue`](src/views/StoryView.vue)、[`MindMapBoard.vue`](src/components/MindMapBoard.vue) | 故事线 / 时间线 / 关系 / Canon / 分镜；本章焦点含情节多选；lore 随 `castRevision` 重载 |
| 情节库 | [`src/views/TropeLibraryView.vue`](src/views/TropeLibraryView.vue)、[`tropeIndex.js`](src/services/tropeIndex.js)、[`tropeKinds.js`](src/utils/tropeKinds.js) | 全局 trope / kink 卡；与设定 / 角色仓分离 |
| 设定 / 角色仓 | [`src/views/LoreView.vue`](src/views/LoreView.vue)、[`CharacterRosterView.vue`](src/views/CharacterRosterView.vue) | lore 与全局角色（排除 trope/kink）；保存/删除后 `bumpCastRevision` |
| 分析 | [`src/views/UsageAnalyticsView.vue`](src/views/UsageAnalyticsView.vue)、[`src/components/analytics/`](src/components/analytics/) | 余额、KPI、折线/柱状、履历详情 |
| 日志 | [`src/views/GenLogView.vue`](src/views/GenLogView.vue) | 轻量历史与导出 |
| 设置 | [`src/views/SettingsView.vue`](src/views/SettingsView.vue) | **界面/写作语言**、端点、DeepSeek 预设、`writing_auto_trope`、文生图、检查更新、多模型槽 |
| 插图 / 更新 UI | [`IllustrationPromptDialog.vue`](src/components/IllustrationPromptDialog.vue)、[`UpdateDialog.vue`](src/components/UpdateDialog.vue) | 提示词确认出图；更新说明与进度 |
| i18n | [`src/i18n/index.js`](src/i18n/index.js)、[`src/i18n/locales/zh-CN.json`](src/i18n/locales/zh-CN.json)、[`en.json`](src/i18n/locales/en.json)、[`ja.json`](src/i18n/locales/ja.json) | 文案目录、`applyUiLocale`、跨语言消息匹配 |
| 用量工具 | [`usageFormat.js`](src/utils/usageFormat.js)、[`usageSeries.js`](src/utils/usageSeries.js)、[`usageEstimate.js`](src/utils/usageEstimate.js)、[`deepseekPricing.js`](src/utils/deepseekPricing.js) | 格式化、按日聚合、无履历约算、官方单价 |
| LLM JSON / 拆章 | [`llmJson.js`](src/utils/llmJson.js)、[`outlineChapters.js`](src/utils/outlineChapters.js)、[`check-llm-json.mjs`](scripts/check-llm-json.mjs) | 近似 JSON 修复；拆章解析与 token 估算；本地回归 |
| 按纲快照 | [`outlineSnapshot.js`](src/utils/outlineSnapshot.js)、[`check-outline-snapshot.mjs`](scripts/check-outline-snapshot.mjs) | 拒脏占位、正文实质门槛；本地验收 |
| 按纲队列 | [`outlineQueue.js`](src/services/outlineQueue.js)、[`bookOutlineQueue.js`](src/services/bookOutlineQueue.js)、[`sectionQueue.js`](src/services/sectionQueue.js) | 整章按纲、`outline_run`、落盘校验 |
| 草稿落盘 | [`draftAccept.js`](src/services/draftAccept.js)、[`genJobs.js`](src/stores/genJobs.js) | `targetChapterId` / `skipAutoAccept`；错章拒写；可触发情节抽取 |
| 情节抽取 | [`tropeExtract.js`](src/services/tropeExtract.js)、[`writingTasks.js`](src/utils/writingTasks.js) | 写后 `trope_extract`；设置开关 |
| 插图 / lore 视觉 | [`illustration.js`](src/services/illustration.js)、[`loreVisual.js`](src/utils/loreVisual.js)、[`genBlock.js`](src/utils/genBlock.js) | 分镜生成、出图落盘、插图块模型；context 可带 tropes |
| 抽角色 | [`castExtract.js`](src/services/castExtract.js) | 自动抽角落盘后 `bumpCastRevision` |
| 应用更新 | [`appUpdate.js`](src/services/appUpdate.js)、[`updateFlow.js`](src/services/updateFlow.js) | 检查 / 下载 / 启动流程 |
| 全局状态 | [`src/stores/appState.js`](src/stores/appState.js)、[`aiPanelState.js`](src/stores/aiPanelState.js) | 当前作品与导航；`castRevision` / `tropeRevision` / `storyRevision`；AI 面板按任务指令槽与 `selectedTropeIds` |
| 客户端 | [`src/services/tauri.js`](src/services/tauri.js)、[`llmClient.js`](src/services/llmClient.js)、[`projectClient.js`](src/services/projectClient.js)、[`storyClient.js`](src/services/storyClient.js)、[`guiBridge.js`](src/services/guiBridge.js) | invoke / 事件桥 / 分镜读写 / `updateChapterMeta` |

---

## 7. 后端模块

| 模块 | 路径 | 职责 |
|---|---|---|
| 共享 API | [`src-tauri/src/api.rs`](src-tauri/src/api.rs) | GUI / CLI / RPC 共用业务、`dispatch_rpc`；分镜、出图、对话会话、`chapter_update_meta` |
| Tauri 命令 | [`src-tauri/src/commands.rs`](src-tauri/src/commands.rs) | `#[tauri::command]` 薄封装（含 update / image / storyboard / chat / llm_chat 流式） |
| 对话会话 | [`src-tauri/src/chat.rs`](src-tauri/src/chat.rs) | 本作 `chat/novel.json`、自由聊 `%APPDATA%/kk_novel_ai/chat/free.json`；含人设字段 |
| GUI 流式 | [`src-tauri/src/gui_writing.rs`](src-tauri/src/gui_writing.rs) | emit `llm-chunk` / `done` / `error` |
| 写作引擎 | [`src-tauri/src/writing/mod.rs`](src-tauri/src/writing/mod.rs) | 任务、上下文、`run_writing`；完整度补写；`outline_run`；`{{tropes}}` 注入；Prompt 经 `prompt_i18n` |
| 连贯 / 收束 | [`src-tauri/src/writing/continuity.rs`](src-tauri/src/writing/continuity.rs) | 半句检测、按纲钩子、总结压缩、拒脏占位 |
| 方向锚点 | [`src-tauri/src/writing/advance.rs`](src-tauri/src/writing/advance.rs) | 按纲「写完即停」与自由续写升级锚点 |
| Prompt i18n | [`src-tauri/src/prompt_i18n.rs`](src-tauri/src/prompt_i18n.rs) | 按 `writing_locale` 选中 / 英 / 日模板（含 `scene_complete`、`trope_extract`） |
| 用户文案 i18n | [`src-tauri/src/i18n.rs`](src-tauri/src/i18n.rs)、[`src-tauri/locales/`](src-tauri/locales/) | 按 `ui_locale` 取错误与提示 |
| 节拍 | [`src-tauri/src/writing/beat_engine.rs`](src-tauri/src/writing/beat_engine.rs) | 按纲进度状态机 |
| 召回 / 去重 | [`retrieve.rs`](src-tauri/src/writing/retrieve.rs)、[`dedupe.rs`](src-tauri/src/writing/dedupe.rs) | lore 召回（可排除未勾选 trope）、复读抑制 |
| 作品磁盘 | [`src-tauri/src/project/mod.rs`](src-tauri/src/project/mod.rs) | 章 / lore / memory / 进度；`kind_dir`；`ChapterMeta.trope_ids`；滚动摘要过滤脏快照 |
| 总谱 / 分镜 | [`src-tauri/src/story/mod.rs`](src-tauri/src/story/mod.rs) | plot / timeline / relations / canon / storyboard |
| 文生图 | [`src-tauri/src/image.rs`](src-tauri/src/image.rs) | OpenAI 兼容出图、读 data URL |
| 应用更新 | [`src-tauri/src/update.rs`](src-tauri/src/update.rs) | 查 Release、下载、启动并退出 |
| LLM | [`src-tauri/src/llm/mod.rs`](src-tauri/src/llm/mod.rs)、[`stream.rs`](src-tauri/src/llm/stream.rs)、[`balance.rs`](src-tauri/src/llm/balance.rs) | OpenAI 兼容流式、取消、thinking 关闭；DeepSeek 余额 |
| 知识库 | [`src-tauri/src/kb/mod.rs`](src-tauri/src/kb/mod.rs) | 通用库聚合；角色仓与情节库目录保障 |
| 导入蒸馏 | [`src-tauri/src/import/mod.rs`](src-tauri/src/import/mod.rs) | TXT 切章、lore_extract（含 tropes 落地与 usage 记账） |
| 导出 | [`src-tauri/src/export/mod.rs`](src-tauri/src/export/mod.rs) | TXT / EPUB / PDF；正文与插图段交错；导出语言随写作 locale |
| IPC | [`src-tauri/src/ipc/mod.rs`](src-tauri/src/ipc/mod.rs) | loopback NDJSON，`ipc.json` |
| 设置 / 路径 | [`settings.rs`](src-tauri/src/settings.rs)、[`paths.rs`](src-tauri/src/paths.rs) | `%APPDATA%/kk_novel_ai/`；`ui_locale` / `writing_locale` / `writing_auto_trope`；DeepSeek 与图像端点 |
| 日志 / 用量 | [`genlog.rs`](src-tauri/src/genlog.rs)、[`usage.rs`](src-tauri/src/usage.rs)、[`project_genlog.rs`](src-tauri/src/project_genlog.rs) | 全局 `gen_log.jsonl`、账本；作品内履历 |

Prompt 模板目录：[`src-tauri/prompts/`](src-tauri/prompts/)（根目录为中文；[`en/`](src-tauri/prompts/en/)、[`ja/`](src-tauri/prompts/ja/) 为英文 / 日文；如 `continue_chapter.md`、`length_fill.md`、`scene_complete.md`、`trope_extract.md`、`outline_to_chapters.md`、`beats_to_storyboard.md`）。

---

## 8. 作品目录与应用数据

作品根目录（实现：[`project/mod.rs`](src-tauri/src/project/mod.rs)、[`story/mod.rs`](src-tauri/src/story/mod.rs)、[`image.rs`](src-tauri/src/image.rs)、[`chat.rs`](src-tauri/src/chat.rs)）：

```text
MyNovel/
  project.json               # 章节列表；章元数据可含 trope_ids
  memory.json
  stats.json
  gen_activity.jsonl         # 作品级 AI / 保存履历索引（新生成后出现）
  embeddings.sqlite          # 配置 embedding_model 后
  chat/novel.json            # 本作对话会话（含人设字段；不写章节）
  story/plot.json | timeline.json | relations.json | canon.json
  story/storyboard.json      # 分镜表（风格前缀、负面词、按章镜头）
  chapters/*.md
  chapters/.progress/        # 按纲节拍进度
  chapters/.genblocks/       # 生成块 sidecar（含插图块元数据）
  chapters/.genlog/*.jsonl   # 按章履历；项目级任务为 _project.jsonl
  assets/illustrations/      # 插图文件（相对路径写入块）
  lore/characters/*.json
  lore/world/*.json
  lore/tropes/*.json         # 情节套路卡（kind=trope）
  lore/kinks/*.json          # 性癖写法卡（kind=kink）
```

应用数据（Windows 典型 `%APPDATA%\kk_novel_ai\`，[`paths.rs`](src-tauri/src/paths.rs)）：`settings.json`（含 `ui_locale` / `writing_locale` / `writing_auto_trope`）、`ipc.json`、`gen_log.jsonl`、用量账本、`chat/free.json`（自由聊会话，含人设字段）。旧作品无 `gen_activity` 时分析页回退全局日志或按配置约算。滚动摘要重建时会跳过不可用的写后快照（过短 / 过长 / 脏占位），见 [`project/mod.rs`](src-tauri/src/project/mod.rs)。

相关命令：`gen_log_list`、`project_gen_log_list`、`usage_summary`、`provider_balance`、`story_storyboard_get` / `story_storyboard_save`、`image_generate`、`chat_session_get` / `chat_session_save`、`chapter_update_meta`（见 [`cli.rs`](src-tauri/src/cli.rs)、[`api.rs`](src-tauri/src/api.rs)）。

---

## 9. 开发与构建

前置：Node.js / npm、Rust（`cargo`）。调试 GUI 需要 Vite 占用 `5173`（[`tauri.conf.json`](src-tauri/tauri.conf.json) `devUrl`）。

```powershell
cd D:\KKFiles\KKProjects\Kinit\kk_novel_ai
npm install

# 前端热更新
npm run dev

# 另一终端：Tauri 调试（需已跑 Vite）
npx tauri dev
```

打包（版本递增默认开启；产物进 `dist/`，不入库）：

| 目标 | 命令 | 脚本路径 |
|---|---|---|
| 仅 Windows EXE | `python build.py` 或 `npm run build:windows` | [`build.py`](build.py) |
| 仅 Android APK | `python build.py --platform android` 或 `npm run build:android` | [`build.py`](build.py)、[`build_android.py`](build_android.py) |
| 双端 | `python build.py --platform all` 或 `npm run build:all` | 同上 |
| 前端静态产物 | `npm run frontend:build` | [`build-frontend.mjs`](build-frontend.mjs) |
| Android 工具链 | `npm run android:bootstrap` | [`build_android.py`](build_android.py) `--bootstrap-only` |
| 只补传 Release | `python build.py --publish-only` | [`build.py`](build.py)（用已有 `dist/` 产物） |
| LLM JSON 回归 | `node scripts/check-llm-json.mjs` | [`scripts/check-llm-json.mjs`](scripts/check-llm-json.mjs)、[`src/utils/llmJson.js`](src/utils/llmJson.js)、[`src/utils/outlineChapters.js`](src/utils/outlineChapters.js) |
| 按纲快照验收 | `node scripts/check-outline-snapshot.mjs` | [`scripts/check-outline-snapshot.mjs`](scripts/check-outline-snapshot.mjs)、[`src/utils/outlineSnapshot.js`](src/utils/outlineSnapshot.js) |

**发版行为（默认）**：`python build.py` 在 **release 成功后默认** 会调用 Cursor（`--model auto`）按上次 tag 相对当前工作区的 diff 重写 [`README.md`](README.md)，并 `git add -A` 提交全部工作区改动、push、上传 GitHub Release（exe/apk）。相关提示词模板：[`scripts/release-readme-prompt.md`](scripts/release-readme-prompt.md)。

| 开关 | 作用 |
|---|---|
| `--no-github-release` | 跳过整段发版（不重写 README、不 commit/push、不上传 Release） |
| `--no-cursor-readme` | 只跳过 Cursor 重写 README；仍可提交与上传 |
| `--publish-only` | 不构建，用当前版本号与 `dist/` 已有产物补传 Release（默认不再跑 Cursor） |
| `--github-release` | debug 构建若也要发版时显式打开 |

发版需本机已安装并登录 [GitHub CLI](https://cli.github.com/)（`gh auth login`），或设置 `GH_TOKEN`。Cursor 重写 README 需 PATH 中有 Agent CLI，或设置 `CURSOR_API_KEY`（可选 `pip install cursor-sdk`）。

Android 细节：[`docs/android-setup.md`](docs/android-setup.md)、[`scripts/android-setup.mjs`](scripts/android-setup.mjs)。移动端抽检：[`docs/mobile-qa-checklist.md`](docs/mobile-qa-checklist.md)。

CLI 调试优先用 `kk_novel_cli`（构建后在 `src-tauri/target/...`）。子命令说明：[`docs/lmstudio.md`](docs/lmstudio.md)。PowerShell 下任务名 `continue` 需当参数传递，避免关键字冲突。

大文件 TXT 导入验收示例：[`scripts/test_import_wendao.ps1`](scripts/test_import_wendao.ps1)，语料 [`test_files/《问道红尘》.txt`](test_files/《问道红尘》.txt)。

---

## 10. 文档索引

| 文档 | 路径 | 内容 |
|---|---|---|
| 本 README | [`README.md`](README.md) | 仓库入口与结构总览 |
| 项目分析 | [`docs/project-analysis.md`](docs/project-analysis.md) | 架构图、模块、数据模型、缺口 |
| 里程碑 TODO | [`docs/todo.md`](docs/todo.md) | M1–M54 及明细（均带代码路径） |
| LM Studio / CLI | [`docs/lmstudio.md`](docs/lmstudio.md) | 本地服务、按纲流程、RPC |
| Android | [`docs/android-setup.md`](docs/android-setup.md) | JDK/SDK 引导、签名、产物 |
| 移动 QA | [`docs/mobile-qa-checklist.md`](docs/mobile-qa-checklist.md) | 触控与布局抽检 |
| 角色仓说明 | [`docs/character_roster_from_stories.md`](docs/character_roster_from_stories.md) | 从故事抽角色 |
| 发版 README 提示词 | [`scripts/release-readme-prompt.md`](scripts/release-readme-prompt.md) | `build.py` 注入后交给 Cursor |

---

## 11. 后续可做（建议 TODO）

已完成的产品里程碑见 [`docs/todo.md`](docs/todo.md)（M1–M54）。本仓库会继续收集 Issue / 用户反馈，把有效建议排进迭代。分析文档里仍开放的工程向建议：

| # | 建议项 | 说明 | 涉及路径 |
|---|---|---|---|
| N7 | 关系图拖拽 / 自动布局 | 现多为圆形布局 | [`src/views/StoryView.vue`](src/views/StoryView.vue) |
| N8 | story_sync 结构化 diff UI | 现为 JSON 确认 | [`src/components/AiPanel.vue`](src/components/AiPanel.vue) |
| N9 | EPUB 导入 | 当前导入以 TXT 为主 | [`src-tauri/src/import/mod.rs`](src-tauri/src/import/mod.rs) |
| N10 | 吸取更多使用建议并优化 | 持续进行；Issue / 反馈优先入库 | 本文；[`docs/todo.md`](docs/todo.md) |
| N11 | 默认推荐 DeepSeek 写作 | 设置页与文档引导；强模型槽对齐 DeepSeek | [`settings.rs`](src-tauri/src/settings.rs)、[`SettingsView.vue`](src/views/SettingsView.vue)、[`llm/mod.rs`](src-tauri/src/llm/mod.rs) |
| N12 | 分析页跨日账本持久化图表 | 现按已加载履历（约 500 条）聚合近 14 天 | [`UsageAnalyticsView.vue`](src/views/UsageAnalyticsView.vue)、[`usage.rs`](src-tauri/src/usage.rs) |
| N13 | 插图批量按卷出图 / 队列 | 现为单镜头 / 单块出图 | [`illustration.js`](src/services/illustration.js)、[`image.rs`](src-tauri/src/image.rs) |
| N14 | 移动端应用内更新体验 | 桌面可下载启动；移动端引导打开 Release 页 | [`update.rs`](src-tauri/src/update.rs)、[`SettingsView.vue`](src/views/SettingsView.vue) |
| N15 | 写作链路共用 LLM JSON 容错 | 拆章已接 `outlineChapters`；其它任务可继续复用 | [`llmJson.js`](src/utils/llmJson.js)、[`outlineChapters.js`](src/utils/outlineChapters.js)、[`writing/`](src-tauri/src/writing/) |
| N16 | 对话页多会话 / 导出 | 现每模式单文件会话；人设已可配置 | [`ChatView.vue`](src/views/ChatView.vue)、[`chat.rs`](src-tauri/src/chat.rs) |
| N17 | i18n 文案覆盖率与校对 | 主流程已覆盖；边角 Toast / CLI 可继续补译 | [`src/i18n/locales/`](src/i18n/locales/)、[`src-tauri/locales/`](src-tauri/locales/)、[`src-tauri/prompts/`](src-tauri/prompts/) |
| N18 | 按纲收束钩子启发式再精 | 现依章纲末句压缩匹配正文尾部 | [`continuity.rs`](src-tauri/src/writing/continuity.rs)、[`writing/mod.rs`](src-tauri/src/writing/mod.rs) |
| N19 | 情节库跨作品复用 / 导入导出 | 现按作品 lore 目录存放 | [`TropeLibraryView.vue`](src/views/TropeLibraryView.vue)、[`project/mod.rs`](src-tauri/src/project/mod.rs)、[`kb/mod.rs`](src-tauri/src/kb/mod.rs) |

已知约束：Debug GUI 依赖 Vite `5173`；Release 读 `frontend-dist/`；蒸馏依赖可用的分析模型（推荐 DeepSeek）+ `analysis_model`，长书请用 `--from` / `--to` 分段。DeepSeek 官方仅提供余额 API，无 Bearer 可查的「今日已用 token」；今日/累计消耗以本应用履历与账本为准。插图需自行配置兼容文生图端点；未配置时仍可写正文，不可出图。本作对话需先打开作品；自由聊写在应用数据目录。界面与写作语言默认 `zh-CN`；未译键回退中文。按纲完整度补写有次数与章长上限，极端短写仍可能需手动续写。情节 / 性癖注入以本章 `trope_ids` 为准；未勾选条目不会当必达，写后抽取可在设置中关闭。
