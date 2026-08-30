# Kk Novel Ai

**Kk Novel Ai** 是面向长篇连载的本地小说创作台：大纲、章节、设定与记忆都落在你自己的磁盘目录里，不绑云端作品库；桌面 GUI 负责写与改，命令行 / 脚本也能驱动同一套写作流水线。

写作、拆章、润色与知识蒸馏推荐接 **DeepSeek** 官方 API（OpenAI 兼容）；也支持本机或其他兼容端点。项目会持续听取反馈并迭代体验（详见下文）。

| 项 | 值 |
|---|---|
| 当前版本 | `0.1.82`（`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 同步） |
| 标识符 | `com.kk.kk-novel-ai` |
| 仓库 | [https://github.com/ksasukirk/kk_novel_ai](https://github.com/ksasukirk/kk_novel_ai) |
| 作者 | kk |

文档：模块分析 [`docs/project-analysis.md`](docs/project-analysis.md) · 里程碑 [`docs/todo.md`](docs/todo.md) · 模型与 CLI [`docs/lmstudio.md`](docs/lmstudio.md)

**持续优化**：欢迎在 [Issues](https://github.com/ksasukirk/kk_novel_ai/issues) 提建议，或直接向作者反馈。合理诉求会尽量排进后续迭代（见 [`docs/todo.md`](docs/todo.md) 与本文第 10 节）。

**模型建议**：长篇续写、拆章、润色与蒸馏，优先使用 **DeepSeek**。应用已对 `deepseek.com` 默认关闭思考链，避免长写作 `content` 为空；强模型槽可指向如 `deepseek-v4-pro`（见 [`src-tauri/src/settings.rs`](src-tauri/src/settings.rs)、[`src/views/SettingsView.vue`](src/views/SettingsView.vue)）。本机 [LM Studio](https://lmstudio.ai/) 等 OpenAI 兼容服务仍可用，配置说明见 [`docs/lmstudio.md`](docs/lmstudio.md)。

---

## 0. 本文编写 TODO

| # | 项 | 状态 | 主要代码 / 文档路径 |
|---|---|---|---|
| R1 | 产品定位与开篇说明 | 完成（已优化为产品向表述） | 本文；[`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) |
| R2 | 技术栈与进程入口 | 完成 | [`package.json`](package.json)、[`src-tauri/Cargo.toml`](src-tauri/Cargo.toml)、[`src-tauri/src/main.rs`](src-tauri/src/main.rs) |
| R3 | 目录结构与前端视图 | 完成 | [`src/App.vue`](src/App.vue)、[`src/views/`](src/views/) |
| R4 | 后端模块与写作流水线 | 完成 | [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs)、[`src-tauri/src/writing/`](src-tauri/src/writing/) |
| R5 | 磁盘数据模型 | 完成 | [`src-tauri/src/project/mod.rs`](src-tauri/src/project/mod.rs)、[`src-tauri/src/paths.rs`](src-tauri/src/paths.rs) |
| R6 | 开发 / 打包 / Android | 完成 | [`build.py`](build.py)、[`build_android.py`](build_android.py)、[`docs/android-setup.md`](docs/android-setup.md) |
| R7 | 文档索引与后续建议 | 完成 | [`docs/`](docs/) |
| R8 | 写明持续吸取建议并优化软件 | 完成 | 本文开篇；[`docs/todo.md`](docs/todo.md) |
| R9 | 写明推荐使用 DeepSeek | 完成 | 本文开篇；[`src-tauri/src/settings.rs`](src-tauri/src/settings.rs)、[`src/views/SettingsView.vue`](src/views/SettingsView.vue) |
| R10 | 重写开篇（弱化旧技术堆砌句） | 完成 | 本文标题下首段 |

---

## 1. 它做什么

- 以**本地作品目录**为真相源：`project.json`、章节 Markdown、lore / memory / 总谱 JSON。
- 续写、润色、拆章、按纲节拍、设定召回、知识库蒸馏，都走同一套 Rust 写作引擎。
- 桌面 GUI（Tauri 2 + Vue 3）负责编辑与预览；CLI / NDJSON RPC 给脚本编排；GUI 在线时 CLI 默认可经本机 IPC 驱动同一套预览（见 [`src-tauri/src/ipc/mod.rs`](src-tauri/src/ipc/mod.rs)、[`src/services/guiBridge.js`](src/services/guiBridge.js)）。
- Windows 桌面为主，Android APK 由 `build_android.py` 引导工具链后打包（见 [`docs/android-setup.md`](docs/android-setup.md)）。
- **持续迭代**：会吸取更多建议来优化本软件；写作模型**最好使用 DeepSeek**（设置页配置端点与模型槽）。

---

## 2. 技术栈

| 层 | 选型 | 路径 |
|---|---|---|
| 桌面壳 | Tauri 2.11 | [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) |
| 前端 | Vue 3 + Vite 5 | [`package.json`](package.json)、[`vite.config.js`](vite.config.js)、[`src/main.js`](src/main.js) |
| 后端 | Rust 2021 | [`src-tauri/src/`](src-tauri/src/) |
| HTTP / 流式 | reqwest + rustls | [`src-tauri/src/llm/mod.rs`](src-tauri/src/llm/mod.rs) |
| CLI | clap 4 | [`src-tauri/src/cli.rs`](src-tauri/src/cli.rs) |
| RAG | rusqlite + 关键词回退 | [`src-tauri/src/rag/mod.rs`](src-tauri/src/rag/mod.rs) |
| 导出 | TXT / EPUB / PDF（zip、krilla） | [`src-tauri/src/export/mod.rs`](src-tauri/src/export/mod.rs) |

窗口无系统边框（`decorations: false`），自定义标题栏在 [`src/App.vue`](src/App.vue)；主题 Token 在 [`src/style.css`](src/style.css)。

---

## 3. 架构

```text
Vue (src/)  --invoke / listen-->  Tauri commands.rs  -->  api.rs
                                                      -->  writing/*  -->  llm/*  -->  LM Studio / OpenAI 兼容
                                                      -->  project/*  -->  作品目录
CLI (cli.rs)  --默认 IPC-->  运行中 GUI（ipc/mod.rs + guiBridge.js）
              --offline / rpc-->  同一套 api.rs（不驱动界面）
```

| 二进制 | 路径 | 行为 |
|---|---|---|
| `kk_novel_ai` | [`src-tauri/src/main.rs`](src-tauri/src/main.rs) | 无参启动 GUI；有子命令或 `--cli` 走 CLI |
| `kk_novel_cli` | [`src-tauri/src/bin/kk_novel_cli.rs`](src-tauri/src/bin/kk_novel_cli.rs) | 纯控制台入口，调试 stdout 更稳 |

GUI 装配：[`src-tauri/src/lib.rs`](src-tauri/src/lib.rs)（注册命令、`CancelRegistry` / `PrepareRegistry`，桌面端启动 IPC）。

---

## 4. 仓库目录

```text
kk_novel_ai/
├── README.md                 # 本文件
├── package.json              # npm 脚本与前端依赖
├── vite.config.js
├── index.html
├── build.py                  # Windows / 可选 Android 总打包
├── build_android.py          # Android 工具链引导 + APK
├── build-frontend.mjs        # 前端产物 frontend-dist/
├── src/                      # Vue 3
│   ├── App.vue
│   ├── views/                # 作品 / 知识库 / 写作 / 大纲 / 总谱 / 设定 / 日志 / 设置
│   ├── components/           # AiPanel、编辑块、壳、思维导图等
│   ├── services/             # Tauri / LLM / 作品 / GUI 桥 / 按纲队列
│   ├── stores/               # appState 等
│   └── utils/
├── src-tauri/                # Tauri + Rust
│   ├── src/                  # 后端模块
│   ├── prompts/              # 写作 Prompt 模板
│   ├── tauri.conf.json
│   └── tauri.android.conf.json
├── scripts/                  # android-setup、导入验收等
├── docs/                     # 分析、TODO、LM Studio、Android、移动 QA
└── test_files/               # 导入语料（如《问道红尘》）
```

构建产物不入库：`dist/`、`frontend-dist/`、`outputs/`、`node_modules/`、`src-tauri/target/`（见 [`.gitignore`](.gitignore)）。人设规则目录 `.cursor/rules/` 同样不提交。

---

## 5. 前端视图与关键路径

侧栏定义：[`src/App.vue`](src/App.vue)（作品、知识库、角色定义、总谱、大纲、写作、设定、日志、设置）。

| 界面 | 路径 | 职责 |
|---|---|---|
| 壳 / 侧栏 / 页眉 | [`src/components/shell/AppSidebar.vue`](src/components/shell/AppSidebar.vue)、[`PageHeader.vue`](src/components/shell/PageHeader.vue)、[`PageBackground.vue`](src/components/shell/PageBackground.vue) | 布局、主题、移动抽屉 |
| 作品 | [`src/views/ProjectHome.vue`](src/views/ProjectHome.vue) | 新建/打开/最近、仪表盘、书名建议 |
| 知识库 | [`src/views/KnowledgeHome.vue`](src/views/KnowledgeHome.vue) | 一书一库、通用库；导入走此页 |
| 写作 | [`src/views/EditorView.vue`](src/views/EditorView.vue)、[`src/components/AiPanel.vue`](src/components/AiPanel.vue)、[`ChapterBlockEditor.vue`](src/components/ChapterBlockEditor.vue) | 章树、块编辑、按纲队列、流式预览 |
| 大纲 | [`src/views/OutlineView.vue`](src/views/OutlineView.vue) | 全书大纲、卷弧、章纲 |
| 总谱 | [`src/views/StoryView.vue`](src/views/StoryView.vue)、[`MindMapBoard.vue`](src/components/MindMapBoard.vue) | 故事线 / 时间线 / 关系 / Canon |
| 设定 / 角色仓 | [`src/views/LoreView.vue`](src/views/LoreView.vue)、[`CharacterRosterView.vue`](src/views/CharacterRosterView.vue) | lore 与全局角色 |
| 日志 | [`src/views/GenLogView.vue`](src/views/GenLogView.vue) | 生成记录、用量、导出 |
| 设置 | [`src/views/SettingsView.vue`](src/views/SettingsView.vue) | 端点、多模型槽、写作参数 |
| 全局状态 | [`src/stores/appState.js`](src/stores/appState.js) | 当前作品与导航 |
| 客户端 | [`src/services/tauri.js`](src/services/tauri.js)、[`llmClient.js`](src/services/llmClient.js)、[`projectClient.js`](src/services/projectClient.js)、[`guiBridge.js`](src/services/guiBridge.js) | invoke / 事件桥 |

按纲生成队列：[`src/services/bookOutlineQueue.js`](src/services/bookOutlineQueue.js)、[`src/services/outlineQueue.js`](src/services/outlineQueue.js)。

---

## 6. 后端模块

| 模块 | 路径 | 职责 |
|---|---|---|
| 共享 API | [`src-tauri/src/api.rs`](src-tauri/src/api.rs) | GUI / CLI / RPC 共用业务、`dispatch_rpc` |
| Tauri 命令 | [`src-tauri/src/commands.rs`](src-tauri/src/commands.rs) | `#[tauri::command]` 薄封装 |
| GUI 流式 | [`src-tauri/src/gui_writing.rs`](src-tauri/src/gui_writing.rs) | emit `llm-chunk` / `done` / `error` |
| 写作引擎 | [`src-tauri/src/writing/mod.rs`](src-tauri/src/writing/mod.rs) | 任务、上下文、`run_writing` |
| 节拍 | [`src-tauri/src/writing/beat_engine.rs`](src-tauri/src/writing/beat_engine.rs) | 按纲进度状态机 |
| 召回 / 去重 | [`retrieve.rs`](src-tauri/src/writing/retrieve.rs)、[`dedupe.rs`](src-tauri/src/writing/dedupe.rs)、[`advance.rs`](src-tauri/src/writing/advance.rs) | lore 召回、复读抑制、方向锚点 |
| 作品磁盘 | [`src-tauri/src/project/mod.rs`](src-tauri/src/project/mod.rs) | 章 / lore / memory / 进度 sidecar |
| 总谱 | [`src-tauri/src/story/mod.rs`](src-tauri/src/story/mod.rs) | plot / timeline / relations / canon |
| LLM | [`src-tauri/src/llm/mod.rs`](src-tauri/src/llm/mod.rs)、[`stream.rs`](src-tauri/src/llm/stream.rs) | OpenAI 兼容流式、取消、thinking 关闭 |
| 知识库 | [`src-tauri/src/kb/mod.rs`](src-tauri/src/kb/mod.rs) | 通用库聚合 |
| 导入蒸馏 | [`src-tauri/src/import/mod.rs`](src-tauri/src/import/mod.rs) | TXT 切章、lore_extract |
| IPC | [`src-tauri/src/ipc/mod.rs`](src-tauri/src/ipc/mod.rs) | loopback NDJSON，`ipc.json` |
| 设置 / 路径 | [`settings.rs`](src-tauri/src/settings.rs)、[`paths.rs`](src-tauri/src/paths.rs) | `%APPDATA%/kk_novel_ai/` |
| 日志 / 用量 | [`genlog.rs`](src-tauri/src/genlog.rs)、[`usage.rs`](src-tauri/src/usage.rs) | `gen_log.jsonl` |

Prompt 模板目录：[`src-tauri/prompts/`](src-tauri/prompts/)（如 `continue_chapter.md`、`outline_to_chapters.md`、`lore_extract.md`、`suggest_book_title.md`）。

---

## 7. 作品目录与应用数据

作品根目录（实现：[`project/mod.rs`](src-tauri/src/project/mod.rs)、[`story/mod.rs`](src-tauri/src/story/mod.rs)）：

```text
MyNovel/
  project.json
  memory.json
  stats.json
  embeddings.sqlite          # 配置 embedding_model 后
  story/plot.json | timeline.json | relations.json | canon.json
  chapters/*.md
  chapters/.progress/        # 按纲节拍进度
  chapters/.genblocks/       # 生成块 sidecar
  lore/characters/*.json
  lore/world/*.json
```

应用数据（Windows 典型 `%APPDATA%\kk_novel_ai\`，[`paths.rs`](src-tauri/src/paths.rs)）：`settings.json`、`ipc.json`、`gen_log.jsonl`。

---

## 8. 开发与构建

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

Android 细节：[`docs/android-setup.md`](docs/android-setup.md)、[`scripts/android-setup.mjs`](scripts/android-setup.mjs)。移动端抽检：[`docs/mobile-qa-checklist.md`](docs/mobile-qa-checklist.md)。

CLI 调试优先用 `kk_novel_cli`（构建后在 `src-tauri/target/...`）。子命令说明：[`docs/lmstudio.md`](docs/lmstudio.md)。PowerShell 下任务名 `continue` 需当参数传递，避免关键字冲突。

大文件 TXT 导入验收示例：[`scripts/test_import_wendao.ps1`](scripts/test_import_wendao.ps1)，语料 [`test_files/《问道红尘》.txt`](test_files/《问道红尘》.txt)。

---

## 9. 文档索引

| 文档 | 路径 | 内容 |
|---|---|---|
| 本 README | [`README.md`](README.md) | 仓库入口与结构总览 |
| 项目分析 | [`docs/project-analysis.md`](docs/project-analysis.md) | 架构图、模块、数据模型、缺口 |
| 里程碑 TODO | [`docs/todo.md`](docs/todo.md) | M1–M53 及明细（均带代码路径） |
| LM Studio / CLI | [`docs/lmstudio.md`](docs/lmstudio.md) | 本地服务、按纲流程、RPC |
| Android | [`docs/android-setup.md`](docs/android-setup.md) | JDK/SDK 引导、签名、产物 |
| 移动 QA | [`docs/mobile-qa-checklist.md`](docs/mobile-qa-checklist.md) | 触控与布局抽检 |
| 角色仓说明 | [`docs/character_roster_from_stories.md`](docs/character_roster_from_stories.md) | 从故事抽角色 |

---

## 10. 后续可做（建议 TODO）

已完成的产品里程碑见 [`docs/todo.md`](docs/todo.md)（M1–M53）。本仓库会继续收集 Issue / 用户反馈，把有效建议排进迭代。分析文档里仍开放的工程向建议：

| # | 建议项 | 说明 | 涉及路径 |
|---|---|---|---|
| N7 | 关系图拖拽 / 自动布局 | 现多为圆形布局 | [`src/views/StoryView.vue`](src/views/StoryView.vue) |
| N8 | story_sync 结构化 diff UI | 现为 JSON 确认 | [`src/components/AiPanel.vue`](src/components/AiPanel.vue) |
| N9 | EPUB 导入 | 当前导入以 TXT 为主 | [`src-tauri/src/import/mod.rs`](src-tauri/src/import/mod.rs) |
| N10 | 吸取更多使用建议并优化 | 持续进行；Issue / 反馈优先入库 | 本文；[`docs/todo.md`](docs/todo.md) |
| N11 | 默认推荐 DeepSeek 写作 | 设置页与文档引导；强模型槽对齐 DeepSeek | [`settings.rs`](src-tauri/src/settings.rs)、[`SettingsView.vue`](src/views/SettingsView.vue)、[`llm/mod.rs`](src-tauri/src/llm/mod.rs) |

已知约束：Debug GUI 依赖 Vite `5173`；Release 读 `frontend-dist/`；蒸馏依赖可用的分析模型（推荐 DeepSeek）+ `analysis_model`，长书请用 `--from` / `--to` 分段。
