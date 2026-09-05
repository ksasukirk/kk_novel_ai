# Kk Novel Ai

**Kk Novel Ai** 是面向长篇连载的本地小说创作台：大纲、章节、设定与记忆都落在你自己的磁盘目录里，不绑云端作品库；桌面 GUI 负责写与改，命令行 / 脚本也能驱动同一套写作流水线。

写作、拆章、润色与知识蒸馏推荐接 **DeepSeek** 官方 API（OpenAI 兼容）；也支持本机或其他兼容端点。插图链路可另配 OpenAI 兼容文生图端点。项目会持续听取反馈并迭代体验。

| 项 | 值 |
|---|---|
| 当前版本 | `0.2.20`（`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 同步） |
| 标识符 | `com.kk.kk-novel-ai` |
| 仓库 | [https://github.com/ksasukirk/kk_novel_ai](https://github.com/ksasukirk/kk_novel_ai) |
| 作者 | kk |

文档：模块分析 [`docs/project-analysis.md`](docs/project-analysis.md) · 里程碑 [`docs/todo.md`](docs/todo.md) · 模型与 CLI [`docs/lmstudio.md`](docs/lmstudio.md)

**持续优化**：欢迎在 [Issues](https://github.com/ksasukirk/kk_novel_ai/issues) 提建议，或直接向作者反馈。合理诉求会尽量排进后续迭代（见 [`docs/todo.md`](docs/todo.md) 与本文第 11 节）。

**模型建议**：长篇续写、拆章、润色与蒸馏，优先使用 **DeepSeek**。设置页提供 Flash / Pro / 本机 LM Studio 预设、空闲/高峰单价与缓存友好续写；应用对 `deepseek.com` 默认关闭思考链，避免长写作 `content` 为空（见 [`src-tauri/src/settings.rs`](src-tauri/src/settings.rs)、[`src/views/SettingsView.vue`](src/views/SettingsView.vue)、[`src/utils/deepseekPricing.js`](src/utils/deepseekPricing.js)）。本机 [LM Studio](https://lmstudio.ai/) 等 OpenAI 兼容服务仍可用，配置说明见 [`docs/lmstudio.md`](docs/lmstudio.md)。

**用量与分析**：侧栏「分析」页展示 DeepSeek 官方余额、本应用累计花费 / token、近 14 天趋势与按模型柱状图；无履历时按当前单价与写作参数做约算（不写假账）。业务 AI 调用会记入全局 `gen_log.jsonl` 与作品目录 `gen_activity.jsonl`（见第 8 节）。

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
| R11 | v0.2.20 插图 / 分镜 / 应用内更新 / 发版流水线 | 完成 | 本文第 1 节；[`illustration.js`](src/services/illustration.js)、[`image.rs`](src-tauri/src/image.rs)、[`update.rs`](src-tauri/src/update.rs)、[`build.py`](build.py) |

---

## 1. 本版（0.2.20）做了什么

相对往期，本版把「写完再配图」和「装完还能自己更新」接到同一套本地作品流里，并让 release 打包能一键把说明与安装包推到 GitHub。

- **章内插图块**：写作编辑器可插入 / 重生成 / 删除插图块，配图落在作品目录相对路径，正文块与插图块并列（见 [`ChapterBlockEditor.vue`](src/components/ChapterBlockEditor.vue)、[`genBlock.js`](src/utils/genBlock.js)、[`illustration.js`](src/services/illustration.js)）。
- **总谱「分镜」页**：可按当前章节拍生成分镜表、编辑风格前缀与负面词、从镜头出提示词并生成插图，再落回章节块；分镜存盘独立于「按正文重建总谱」（见 [`StoryView.vue`](src/views/StoryView.vue)、[`story/mod.rs`](src-tauri/src/story/mod.rs)、[`beats_to_storyboard.md`](src-tauri/prompts/beats_to_storyboard.md)）。
- **文生图设置**：设置页可配 OpenAI 兼容图像端点、模型、尺寸与 API Key；提示词对话框可结合 lore 视觉描述起草（见 [`SettingsView.vue`](src/views/SettingsView.vue)、[`image.rs`](src-tauri/src/image.rs)、[`IllustrationPromptDialog.vue`](src/components/IllustrationPromptDialog.vue)、[`loreVisual.js`](src/utils/loreVisual.js)）。
- **导出带图**：TXT / EPUB / PDF 导出会识别插图块；TXT 写占位说明，EPUB / PDF 嵌入图片（见 [`export/mod.rs`](src-tauri/src/export/mod.rs)）。
- **应用内检查更新**：设置页可查 GitHub Release、下载桌面安装包并启动新版本（连不上时可打开 Release 页）（见 [`appUpdate.js`](src/services/appUpdate.js)、[`updateFlow.js`](src/services/updateFlow.js)、[`UpdateDialog.vue`](src/components/UpdateDialog.vue)、[`update.rs`](src-tauri/src/update.rs)）。
- **发版流水线**：`python build.py` 在 release 成功后默认用 Cursor 按上次 tag 的 diff 重写本 README，再提交工作区、push，并把 exe/apk 上传到 GitHub Release（见第 9 节与 [`build.py`](build.py)）。

---

## 2. 它做什么

- 以**本地作品目录**为真相源：`project.json`、章节 Markdown、lore / memory / 总谱 / 分镜 JSON。
- 续写、润色、拆章、按纲节拍、设定召回、知识库蒸馏，都走同一套 Rust 写作引擎。
- 桌面 GUI（Tauri 2 + Vue 3）负责编辑与预览；CLI / NDJSON RPC 给脚本编排；GUI 在线时 CLI 默认可经本机 IPC 驱动同一套预览（见 [`src-tauri/src/ipc/mod.rs`](src-tauri/src/ipc/mod.rs)、[`src/services/guiBridge.js`](src/services/guiBridge.js)）。
- Windows 桌面为主，Android APK 由 `build_android.py` 引导工具链后打包（见 [`docs/android-setup.md`](docs/android-setup.md)）。
- **持续迭代**：会吸取更多建议来优化本软件；写作模型**最好使用 DeepSeek**（设置页配置端点与模型槽）。
- **用量可追溯**：续写 / 润色 / 书名建议 / 导入蒸馏等业务 AI 调用记 token 与花费；侧栏「分析」可看余额、KPI 与趋势。
- **插图可选**：分镜与章内插图依赖你配置的文生图端点；不配也能正常写作与导出纯文本。

---

## 3. 技术栈

| 层 | 选型 | 路径 |
|---|---|---|
| 桌面壳 | Tauri 2.11 | [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) |
| 前端 | Vue 3 + Vite 5 | [`package.json`](package.json)、[`vite.config.js`](vite.config.js)、[`src/main.js`](src/main.js) |
| 后端 | Rust 2021 | [`src-tauri/src/`](src-tauri/src/) |
| HTTP / 流式 | reqwest + rustls | [`src-tauri/src/llm/mod.rs`](src-tauri/src/llm/mod.rs) |
| 文生图 | OpenAI 兼容 images API | [`src-tauri/src/image.rs`](src-tauri/src/image.rs) |
| CLI | clap 4 | [`src-tauri/src/cli.rs`](src-tauri/src/cli.rs) |
| RAG | rusqlite + 关键词回退 | [`src-tauri/src/rag/mod.rs`](src-tauri/src/rag/mod.rs) |
| 导出 | TXT / EPUB / PDF（zip、krilla，可嵌插图） | [`src-tauri/src/export/mod.rs`](src-tauri/src/export/mod.rs) |
| 应用更新 | GitHub Release 检查 / 下载 | [`src-tauri/src/update.rs`](src-tauri/src/update.rs) |

窗口无系统边框（`decorations: false`），自定义标题栏在 [`src/App.vue`](src/App.vue)；主题 Token 在 [`src/style.css`](src/style.css)。

---

## 4. 架构

```text
Vue (src/)  --invoke / listen-->  Tauri commands.rs  -->  api.rs
                                                      -->  writing/*  -->  llm/*  -->  LM Studio / OpenAI 兼容
                                                      -->  image.rs   -->  文生图端点
                                                      -->  update.rs  -->  GitHub Release
                                                      -->  project/* / story/*  -->  作品目录
CLI (cli.rs)  --默认 IPC-->  运行中 GUI（ipc/mod.rs + guiBridge.js）
              --offline / rpc-->  同一套 api.rs（不驱动界面）
```

| 二进制 | 路径 | 行为 |
|---|---|---|
| `kk_novel_ai` | [`src-tauri/src/main.rs`](src-tauri/src/main.rs) | 无参启动 GUI；有子命令或 `--cli` 走 CLI |
| `kk_novel_cli` | [`src-tauri/src/bin/kk_novel_cli.rs`](src-tauri/src/bin/kk_novel_cli.rs) | 纯控制台入口，调试 stdout 更稳 |

GUI 装配：[`src-tauri/src/lib.rs`](src-tauri/src/lib.rs)（注册命令、`CancelRegistry` / `PrepareRegistry`，桌面端启动 IPC）。

---

## 5. 仓库目录

```text
kk_novel_ai/
├── README.md                 # 本文件
├── package.json              # npm 脚本与前端依赖
├── vite.config.js
├── index.html
├── build.py                  # Windows / 可选 Android 总打包 + 默认发版
├── build_android.py          # Android 工具链引导 + APK
├── build-frontend.mjs        # 前端产物 frontend-dist/
├── src/                      # Vue 3
│   ├── App.vue
│   ├── views/                # 作品 / 知识库 / 写作 / 大纲 / 总谱 / 设定 / 分析 / 日志 / 设置
│   ├── components/           # AiPanel、编辑块、插图对话框、更新对话框、壳、思维导图等
│   ├── services/             # Tauri / LLM / 作品 / 插图 / 更新 / GUI 桥 / 按纲队列
│   ├── stores/               # appState 等
│   └── utils/                # 用量 / DeepSeek 单价 / lore 视觉 / 生成块
├── src-tauri/                # Tauri + Rust
│   ├── src/                  # 后端模块（含 image.rs / update.rs）
│   ├── prompts/              # 写作 / 分镜 / 插图 Prompt 模板
│   ├── tauri.conf.json
│   └── tauri.android.conf.json
├── scripts/                  # android-setup、发版 README 提示词、导入验收等
├── docs/                     # 分析、TODO、LM Studio、Android、移动 QA
└── test_files/               # 导入语料（如《问道红尘》）
```

构建产物不入库：`dist/`、`frontend-dist/`、`outputs/`、`node_modules/`、`src-tauri/target/`（见 [`.gitignore`](.gitignore)）。人设规则目录 `.cursor/rules/` 同样不提交。

---

## 6. 前端视图与关键路径

侧栏定义：[`src/App.vue`](src/App.vue)（作品、知识库、角色定义、总谱、大纲、写作、设定、分析、日志、设置）。

| 界面 | 路径 | 职责 |
|---|---|---|
| 壳 / 侧栏 / 页眉 | [`src/components/shell/AppSidebar.vue`](src/components/shell/AppSidebar.vue)、[`PageHeader.vue`](src/components/shell/PageHeader.vue)、[`PageBackground.vue`](src/components/shell/PageBackground.vue) | 布局、主题、移动抽屉 |
| 作品 | [`src/views/ProjectHome.vue`](src/views/ProjectHome.vue) | 新建/打开/最近、仪表盘、书名建议 |
| 知识库 | [`src/views/KnowledgeHome.vue`](src/views/KnowledgeHome.vue) | 一书一库、通用库；导入走此页 |
| 写作 | [`src/views/EditorView.vue`](src/views/EditorView.vue)、[`src/components/AiPanel.vue`](src/components/AiPanel.vue)、[`ChapterBlockEditor.vue`](src/components/ChapterBlockEditor.vue) | 章树、块编辑（含插图块）、按纲队列、流式预览 |
| 大纲 | [`src/views/OutlineView.vue`](src/views/OutlineView.vue) | 全书大纲、卷弧、章纲 |
| 总谱 | [`src/views/StoryView.vue`](src/views/StoryView.vue)、[`MindMapBoard.vue`](src/components/MindMapBoard.vue) | 故事线 / 时间线 / 关系 / Canon / **分镜** |
| 设定 / 角色仓 | [`src/views/LoreView.vue`](src/views/LoreView.vue)、[`CharacterRosterView.vue`](src/views/CharacterRosterView.vue) | lore 与全局角色；视觉字段辅助出图 |
| 分析 | [`src/views/UsageAnalyticsView.vue`](src/views/UsageAnalyticsView.vue)、[`src/components/analytics/`](src/components/analytics/) | 余额、KPI、折线/柱状、履历详情；无数据时配置约算 |
| 日志 | [`src/views/GenLogView.vue`](src/views/GenLogView.vue) | 轻量历史与导出 |
| 设置 | [`src/views/SettingsView.vue`](src/views/SettingsView.vue) | 端点、DeepSeek 预设、文生图、检查更新、多模型槽 |
| 插图 / 更新 UI | [`IllustrationPromptDialog.vue`](src/components/IllustrationPromptDialog.vue)、[`UpdateDialog.vue`](src/components/UpdateDialog.vue) | 提示词确认出图；更新说明与进度 |
| 用量工具 | [`usageFormat.js`](src/utils/usageFormat.js)、[`usageSeries.js`](src/utils/usageSeries.js)、[`usageEstimate.js`](src/utils/usageEstimate.js)、[`deepseekPricing.js`](src/utils/deepseekPricing.js) | 格式化、按日聚合、无履历约算、官方单价 |
| 插图 / lore 视觉 | [`illustration.js`](src/services/illustration.js)、[`loreVisual.js`](src/utils/loreVisual.js)、[`genBlock.js`](src/utils/genBlock.js) | 分镜生成、出图落盘、插图块模型 |
| 应用更新 | [`appUpdate.js`](src/services/appUpdate.js)、[`updateFlow.js`](src/services/updateFlow.js) | 检查 / 下载 / 启动流程 |
| 全局状态 | [`src/stores/appState.js`](src/stores/appState.js) | 当前作品与导航 |
| 客户端 | [`src/services/tauri.js`](src/services/tauri.js)、[`llmClient.js`](src/services/llmClient.js)、[`projectClient.js`](src/services/projectClient.js)、[`storyClient.js`](src/services/storyClient.js)、[`guiBridge.js`](src/services/guiBridge.js) | invoke / 事件桥 / 分镜读写 |

按纲生成队列：[`src/services/bookOutlineQueue.js`](src/services/bookOutlineQueue.js)、[`src/services/outlineQueue.js`](src/services/outlineQueue.js)。

---

## 7. 后端模块

| 模块 | 路径 | 职责 |
|---|---|---|
| 共享 API | [`src-tauri/src/api.rs`](src-tauri/src/api.rs) | GUI / CLI / RPC 共用业务、`dispatch_rpc`；分镜与出图入口 |
| Tauri 命令 | [`src-tauri/src/commands.rs`](src-tauri/src/commands.rs) | `#[tauri::command]` 薄封装（含 update / image / storyboard） |
| GUI 流式 | [`src-tauri/src/gui_writing.rs`](src-tauri/src/gui_writing.rs) | emit `llm-chunk` / `done` / `error` |
| 写作引擎 | [`src-tauri/src/writing/mod.rs`](src-tauri/src/writing/mod.rs) | 任务、上下文、`run_writing`；分镜相关写作任务 |
| 节拍 | [`src-tauri/src/writing/beat_engine.rs`](src-tauri/src/writing/beat_engine.rs) | 按纲进度状态机 |
| 召回 / 去重 | [`retrieve.rs`](src-tauri/src/writing/retrieve.rs)、[`dedupe.rs`](src-tauri/src/writing/dedupe.rs)、[`advance.rs`](src-tauri/src/writing/advance.rs) | lore 召回、复读抑制、方向锚点 |
| 作品磁盘 | [`src-tauri/src/project/mod.rs`](src-tauri/src/project/mod.rs) | 章 / lore / memory / 进度 sidecar |
| 总谱 / 分镜 | [`src-tauri/src/story/mod.rs`](src-tauri/src/story/mod.rs) | plot / timeline / relations / canon / **storyboard** |
| 文生图 | [`src-tauri/src/image.rs`](src-tauri/src/image.rs) | OpenAI 兼容出图、读 data URL |
| 应用更新 | [`src-tauri/src/update.rs`](src-tauri/src/update.rs) | 查 Release、下载、启动并退出 |
| LLM | [`src-tauri/src/llm/mod.rs`](src-tauri/src/llm/mod.rs)、[`stream.rs`](src-tauri/src/llm/stream.rs)、[`balance.rs`](src-tauri/src/llm/balance.rs) | OpenAI 兼容流式、取消、thinking 关闭；DeepSeek `GET /user/balance` |
| 知识库 | [`src-tauri/src/kb/mod.rs`](src-tauri/src/kb/mod.rs) | 通用库聚合 |
| 导入蒸馏 | [`src-tauri/src/import/mod.rs`](src-tauri/src/import/mod.rs) | TXT 切章、lore_extract（带 usage 记账） |
| 导出 | [`src-tauri/src/export/mod.rs`](src-tauri/src/export/mod.rs) | TXT / EPUB / PDF；正文与插图段交错 |
| IPC | [`src-tauri/src/ipc/mod.rs`](src-tauri/src/ipc/mod.rs) | loopback NDJSON，`ipc.json` |
| 设置 / 路径 | [`settings.rs`](src-tauri/src/settings.rs)、[`paths.rs`](src-tauri/src/paths.rs) | `%APPDATA%/kk_novel_ai/`；DeepSeek 与图像端点 |
| 日志 / 用量 | [`genlog.rs`](src-tauri/src/genlog.rs)、[`usage.rs`](src-tauri/src/usage.rs)、[`project_genlog.rs`](src-tauri/src/project_genlog.rs) | 全局 `gen_log.jsonl`、账本；作品内 `gen_activity.jsonl` / `.genlog/` |

Prompt 模板目录：[`src-tauri/prompts/`](src-tauri/prompts/)（如 `continue_chapter.md`、`outline_to_chapters.md`、`lore_extract.md`、`suggest_book_title.md`、`beats_to_storyboard.md`、`content_to_image_prompt.md`）。

---

## 8. 作品目录与应用数据

作品根目录（实现：[`project/mod.rs`](src-tauri/src/project/mod.rs)、[`story/mod.rs`](src-tauri/src/story/mod.rs)、[`image.rs`](src-tauri/src/image.rs)）：

```text
MyNovel/
  project.json
  memory.json
  stats.json
  gen_activity.jsonl         # 作品级 AI / 保存履历索引（新生成后出现）
  embeddings.sqlite          # 配置 embedding_model 后
  story/plot.json | timeline.json | relations.json | canon.json
  story/storyboard.json      # 分镜表（风格前缀、负面词、按章镜头）
  chapters/*.md
  chapters/.progress/        # 按纲节拍进度
  chapters/.genblocks/       # 生成块 sidecar（含插图块元数据）
  chapters/.genlog/*.jsonl   # 按章履历；项目级任务为 _project.jsonl
  assets/illustrations/      # 插图文件（相对路径写入块）
  lore/characters/*.json
  lore/world/*.json
```

应用数据（Windows 典型 `%APPDATA%\kk_novel_ai\`，[`paths.rs`](src-tauri/src/paths.rs)）：`settings.json`、`ipc.json`、`gen_log.jsonl`、用量账本。旧作品无 `gen_activity` 时分析页回退全局日志或按配置约算。

相关命令：`gen_log_list`、`project_gen_log_list`、`usage_summary`、`provider_balance`、`story_storyboard_get` / `story_storyboard_save`、`image_generate`（见 [`cli.rs`](src-tauri/src/cli.rs)、[`api.rs`](src-tauri/src/api.rs)）。

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
| 里程碑 TODO | [`docs/todo.md`](docs/todo.md) | M1–M53 及明细（均带代码路径） |
| LM Studio / CLI | [`docs/lmstudio.md`](docs/lmstudio.md) | 本地服务、按纲流程、RPC |
| Android | [`docs/android-setup.md`](docs/android-setup.md) | JDK/SDK 引导、签名、产物 |
| 移动 QA | [`docs/mobile-qa-checklist.md`](docs/mobile-qa-checklist.md) | 触控与布局抽检 |
| 角色仓说明 | [`docs/character_roster_from_stories.md`](docs/character_roster_from_stories.md) | 从故事抽角色 |
| 发版 README 提示词 | [`scripts/release-readme-prompt.md`](scripts/release-readme-prompt.md) | `build.py` 注入后交给 Cursor |

---

## 11. 后续可做（建议 TODO）

已完成的产品里程碑见 [`docs/todo.md`](docs/todo.md)（M1–M53）。本仓库会继续收集 Issue / 用户反馈，把有效建议排进迭代。分析文档里仍开放的工程向建议：

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

已知约束：Debug GUI 依赖 Vite `5173`；Release 读 `frontend-dist/`；蒸馏依赖可用的分析模型（推荐 DeepSeek）+ `analysis_model`，长书请用 `--from` / `--to` 分段。DeepSeek 官方仅提供余额 API，无 Bearer 可查的「今日已用 token」；今日/累计消耗以本应用履历与账本为准。插图需自行配置兼容文生图端点；未配置时仍可写正文，不可出图。
