<script setup>
/**
 * 启动存储迁移全屏闸门
 * 代码路径: kk_novel_ai/src/views/MigrationGate.vue
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { appState } from "../stores/appState.js";
import { runStorageMigration } from "../services/storageMigrate.js";

const { t } = useI18n();

const percent = computed(() => Math.min(100, Math.max(0, appState.storageMigrate.percent || 0)));
const stageLabel = computed(() => {
  const stage = appState.storageMigrate.stage || "";
  const key = `migration.stage.${stage}`;
  const translated = t(key);
  if (translated !== key) return translated;
  return appState.storageMigrate.label || stage || t("migration.working");
});

async function retry() {
  appState.storageMigrate.error = "";
  try {
    await runStorageMigration();
  } catch {
    /* 错误已写入 state */
  }
}
</script>

<template>
  <div class="migration-gate">
    <div class="migration-card">
      <h1 class="migration-title">{{ t("migration.title") }}</h1>
      <p class="migration-desc">{{ t("migration.desc") }}</p>
      <div class="migration-bar" role="progressbar" :aria-valuenow="percent" aria-valuemin="0" aria-valuemax="100">
        <div class="migration-bar-fill" :style="{ width: percent + '%' }" />
      </div>
      <p class="migration-stage">{{ stageLabel }} · {{ percent.toFixed(0) }}%</p>
      <p v-if="appState.storageMigrate.backupPath" class="migration-backup">
        {{ t("migration.backupPath") }}：{{ appState.storageMigrate.backupPath }}
      </p>
      <p v-if="appState.storageMigrate.error" class="migration-error">
        {{ appState.storageMigrate.error }}
      </p>
      <button
        v-if="appState.storageMigrate.error"
        type="button"
        class="migration-retry"
        @click="retry"
      >
        {{ t("migration.retry") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.migration-gate {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(160deg, #1a1f2e 0%, #0f1419 55%, #1c2333 100%);
  color: #e8ecf1;
  padding: 24px;
}
.migration-card {
  width: min(480px, 100%);
  padding: 28px 24px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
}
.migration-title {
  margin: 0 0 8px;
  font-size: 1.35rem;
  font-weight: 650;
}
.migration-desc {
  margin: 0 0 20px;
  opacity: 0.78;
  line-height: 1.5;
  font-size: 0.95rem;
}
.migration-bar {
  height: 10px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.1);
  overflow: hidden;
}
.migration-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, #5b8def, #7ec8e3);
  transition: width 0.25s ease;
}
.migration-stage {
  margin: 12px 0 0;
  font-size: 0.9rem;
  opacity: 0.9;
}
.migration-backup {
  margin: 10px 0 0;
  font-size: 0.75rem;
  opacity: 0.55;
  word-break: break-all;
}
.migration-error {
  margin: 14px 0 0;
  color: #ff8a8a;
  font-size: 0.9rem;
  line-height: 1.4;
}
.migration-retry {
  margin-top: 16px;
  padding: 8px 16px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.25);
  background: rgba(255, 255, 255, 0.08);
  color: inherit;
  cursor: pointer;
}
.migration-retry:hover {
  background: rgba(255, 255, 255, 0.14);
}
</style>
