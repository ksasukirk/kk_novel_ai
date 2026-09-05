# 发版文档任务（由 build.py 注入后交给 Cursor）

代码路径: kk_novel_ai/scripts/release-readme-prompt.md

你在仓库根目录工作。这是一次 **release 发版** 的文档步骤，请独立完成，不要提问、不要开 PR。调用方使用 Cursor **Auto** 模型（`--model auto`）。

## 注入变量

- 本版版本号：`{{VERSION}}`
- 仓库：{{GITHUB_URL}}
- 对比基准（往期）：`{{BASE_REF}}`
- 提交记录：见 `{{LOG_PATH}}`
- diff --stat：见 `{{STAT_PATH}}`
- 完整 diff（可能已截断）：见 `{{DIFF_PATH}}`
- commit message 必须写到：`{{COMMIT_MSG_PATH}}`

## 必须做的两件事

1. **整份覆盖** 仓库根目录的 `README.md`（不要只改一节）。
2. 把 git commit message 写入 `{{COMMIT_MSG_PATH}}`（UTF-8，无 BOM）。

## README 硬约束

- 使用中文；禁止 emoji。
- 版本号写成 `{{VERSION}}`，与 `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 对齐。
- 必须保留可点的 GitHub 链接：{{GITHUB_URL}}
- 保持现有骨架（可增删小节，但不要变成空壳）：产品定位、推荐 DeepSeek、目录/模块表（**每张表都要带代码文件路径**）、开发与打包。
- 根据 diff 写清 **本版做了什么**（用户能感知的功能，而不是文件清单）。
- 在「开发与构建」写明：`python build.py` 在 **release 成功后默认** 会调用 Cursor 按上次 tag 的 diff 重写 README，并 `git add -A` 提交全部工作区改动、push、上传 GitHub Release；跳过整段发版用 `--no-github-release`；只跳过 Cursor 重写 README 用 `--no-cursor-readme`。
- 不要贴聊天记录、不要写密钥、不要写 `.cursor/rules` 人设内容。

## commit message 硬约束

文件 `{{COMMIT_MSG_PATH}}` 格式：

```
Release v{{VERSION}}

正文：用一两段说明 why（本版相对往期的目的与结果）。不要堆文件名列表。
```

第一行必须恰好是 `Release v{{VERSION}}`，随后空一行再写正文。

## 范围

- **只改** `README.md` 和上面的 commit message 文件。
- 不要改源码、不要 `git commit` / `git push`（build.py 会提交）。
- 不要提交 `.cursor/`、密钥、`dist/`、`frontend-dist/`、`node_modules/`、`src-tauri/target/`。

完成后直接结束。
