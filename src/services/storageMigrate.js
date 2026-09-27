/**
 * 启动存储迁移（备份 + 分阶段进 SQLite）
 * 代码路径: kk_novel_ai/src/services/storageMigrate.js
 */
import { invoke, listen } from "./tauri.js";
import { appState } from "../stores/appState.js";

/**
 * @returns {Promise<{needed:boolean,done:boolean,version:number,backup_path?:string,empty_install?:boolean}>}
 */
export async function getStorageMigrationStatus() {
  return await invoke("storage_migration_status");
}

/**
 * 跑迁移；通过 appState.storageMigrate 更新进度
 * @returns {Promise<object>}
 */
export async function runStorageMigration() {
  appState.storageMigrate.active = true;
  appState.storageMigrate.error = "";
  appState.storageMigrate.percent = 0;
  appState.storageMigrate.stage = "start";
  appState.storageMigrate.label = "";

  let unlisten = null;
  try {
    unlisten = await listen("storage-migrate-progress", (event) => {
      const p = event.payload || {};
      appState.storageMigrate.percent = Number(p.percent) || 0;
      appState.storageMigrate.stage = p.stage || "";
      appState.storageMigrate.label = p.label || "";
      appState.storageMigrate.current = Number(p.current) || 0;
      appState.storageMigrate.total = Number(p.total) || 0;
    });
  } catch {
    /* 非 Tauri */
  }

  try {
    const result = await invoke("storage_migration_run");
    appState.storageMigrate.percent = 100;
    appState.storageMigrate.done = true;
    appState.storageMigrate.active = false;
    return result;
  } catch (e) {
    appState.storageMigrate.error = String(e?.message || e || "migration failed");
    appState.storageMigrate.active = false;
    throw e;
  } finally {
    if (typeof unlisten === "function") {
      try {
        unlisten();
      } catch {
        /* ignore */
      }
    }
  }
}

/**
 * 启动闸门：需要则跑迁移，完成后返回
 */
export async function ensureStorageReady() {
  try {
    const st = await getStorageMigrationStatus();
    appState.storageMigrate.needed = !!st.needed;
    appState.storageMigrate.backupPath = st.backup_path || "";
    if (!st.needed) {
      appState.storageMigrate.done = true;
      return st;
    }
    // 让出一帧，确保 MigrationGate 先渲染再跑重任务
    await new Promise((r) => setTimeout(r, 0));
    return await runStorageMigration();
  } catch (e) {
    const msg = String(e?.message || e || "storage check failed");
    // 浏览器预览 / 无 IPC：跳过闸门
    if (/not allowed|IPC|invoke|Tauri|plugin/i.test(msg) || typeof window === "undefined") {
      appState.storageMigrate.needed = false;
      appState.storageMigrate.done = true;
      return { skipped: true };
    }
    appState.storageMigrate.error = msg;
    appState.storageMigrate.needed = true;
    throw e;
  }
}
