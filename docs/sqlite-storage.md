# 分域多 SQLite 存储与启动迁移

代码路径索引与运维说明。

## 库布局

| 文件 | 路径解析 | 模块 |
|------|----------|------|
| `app.sqlite` | [`paths::app_data_dir`](../src-tauri/src/paths.rs) | [`storage/app_store.rs`](../src-tauri/src/storage/app_store.rs) |
| `characters.sqlite` | app_data | [`storage/characters_store.rs`](../src-tauri/src/storage/characters_store.rs) |
| `kb.sqlite` | app_data | [`storage/kb_store.rs`](../src-tauri/src/storage/kb_store.rs) |
| `activity.sqlite` | app_data | [`storage/activity_store.rs`](../src-tauri/src/storage/activity_store.rs) |
| `library.sqlite` | `novels/_library/` | [`storage/library_store.rs`](../src-tauri/src/storage/library_store.rs) |
| `work.sqlite` | 每作品根目录 | [`storage/work_store.rs`](../src-tauri/src/storage/work_store.rs) |
| `embeddings.sqlite` | 每作品根目录（已有） | [`rag/mod.rs`](../src-tauri/src/rag/mod.rs) |

连接统一：`PRAGMA journal_mode=WAL`、`busy_timeout=5000`（[`storage/mod.rs`](../src-tauri/src/storage/mod.rs)）。

`STORAGE_VERSION = 1`（[`storage/schema.rs`](../src-tauri/src/storage/schema.rs)）。

## 启动流程

1. 前端 [`App.vue`](../src/App.vue) `onMounted` 调用 [`storageMigrate.js`](../src/services/storageMigrate.js) `ensureStorageReady`
2. IPC `storage_migration_status` / `storage_migration_run`（[`api.rs`](../src-tauri/src/api.rs)、[`commands.rs`](../src-tauri/src/commands.rs)）
3. 引擎 [`storage/migrate.rs`](../src-tauri/src/storage/migrate.rs)：备份 → app → characters → kb → library → works → activity → verify → 归档 `.legacy/`
4. 进度事件 `storage-migrate-progress`；UI [`MigrationGate.vue`](../src/views/MigrationGate.vue)
5. 备份目录：`{app_data}/migrations/backup_YYYYMMDD-HHMMSS/`（[`backup_startup.rs`](../src-tauri/src/storage/backup_startup.rs)）

空安装：无旧 JSON/作品时建空库并标记 `done`，不备份。

## 读写切流

- 全局 `migration_state.done` → [`is_storage_migrated()`](../src-tauri/src/storage/mod.rs)
- 作品：`work.sqlite` 存在则走 [`work_store`](../src-tauri/src/storage/work_store.rs)（[`project/mod.rs`](../src-tauri/src/project/mod.rs)）
- 情节库列表默认摘要（无全文）：[`library_store::list_lite`](../src-tauri/src/storage/library_store.rs) / [`kb::list_trope_library_entries_lite`](../src-tauri/src/kb/mod.rs)
- 导出 ZIP：从库物化临时目录再打包（[`project/backup.rs`](../src-tauri/src/project/backup.rs)）
- 导入旧 ZIP：若已迁库则吸入 `work.sqlite` 并归档文件树

## 回滚

1. 关闭应用
2. 用 `{app_data}/migrations/backup_*` 还原 `app_data` 关键文件与 `novels/`
3. 删除各 `*.sqlite` 与 `migration_state.done`（或删整个 `app.sqlite`）后重启以重新迁移

失败时不移动 `.legacy/`；可点闸门「重试」。checkpoint 存在 `migration_state.checkpoint_json`。

## 迁移期互斥

`migration_in_progress` 为真时，[`rag/mod.rs`](../src-tauri/src/rag/mod.rs) 拒绝打开 embeddings；[`project_open`](../src-tauri/src/api.rs) 延迟重建会跳过。
