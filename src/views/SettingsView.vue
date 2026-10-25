<template>
  <div class="cf-page">
    <div class="cf-page-header">
      <div>
        <h1 class="cf-page-title">设置</h1>
        <p class="cf-page-subtitle">本地工作区与应用信息</p>
      </div>
    </div>

    <el-alert
      v-if="settings.error"
      class="notice"
      type="error"
      :closable="false"
      show-icon
      :title="settings.error"
    />

    <section class="block cf-card">
      <h2 class="block__title">作品根目录</h2>
      <p class="block__desc">
        新建作品时会在该目录下创建作品文件夹。修改根目录不会移动已有作品。
      </p>
      <div class="path-row">
        <el-input
          :model-value="settings.settings.workspaceRoot ?? ''"
          readonly
          placeholder="尚未设置"
        />
        <el-button :loading="settings.loading" @click="chooseRoot">更改目录</el-button>
      </div>
    </section>

    <section class="block cf-card">
      <h2 class="block__title">数据库备份</h2>
      <p class="block__desc">
        备份包含作品与素材索引、标签、设置和导入任务记录；<strong>不包含</strong>作品目录中的
        视频、音频、图片等原始文件，也不包含可重新生成的缩略图缓存。作品原文件请另行备份。
      </p>
      <div class="path-row">
        <el-input :model-value="backup.directory" readonly placeholder="尚未确定备份目录" />
        <el-button :loading="backup.loading" @click="chooseBackupDirectory">更改目录</el-button>
        <el-button type="primary" :loading="backup.creating" @click="createBackup">
          立即备份
        </el-button>
      </div>

      <el-alert
        v-if="backup.error"
        class="inline-error"
        type="error"
        :closable="false"
        show-icon
        :title="backup.error"
      />

      <el-table
        :data="backup.entries"
        v-loading="backup.loading"
        size="small"
        class="backup-table"
        empty-text="还没有备份"
      >
        <el-table-column prop="fileName" label="备份文件" min-width="220" show-overflow-tooltip />
        <el-table-column label="创建时间" width="165">
          <template #default="{ row }">{{ row.createdAt ? formatDateTime(row.createdAt) : '—' }}</template>
        </el-table-column>
        <el-table-column label="大小" width="95" align="right">
          <template #default="{ row }">{{ formatBytes(row.sizeBytes) }}</template>
        </el-table-column>
        <el-table-column label="结构版本" width="90" align="center">
          <template #default="{ row }">{{ row.manifest?.schemaVersion ?? '—' }}</template>
        </el-table-column>
        <el-table-column label="状态" width="110">
          <template #default="{ row }">
            <el-tooltip :disabled="!row.problem" :content="row.problem ?? ''" placement="top">
              <el-tag :type="row.valid ? 'success' : 'danger'" size="small">
                {{ row.valid ? '可恢复' : '不可用' }}
              </el-tag>
            </el-tooltip>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="150" align="right">
          <template #default="{ row }">
            <el-button size="small" text @click="verifyBackup(row)">校验</el-button>
            <el-button
              size="small"
              text
              type="danger"
              :disabled="!row.valid || backup.restoring"
              @click="restoreBackup(row)"
            >
              恢复
            </el-button>
          </template>
        </el-table-column>
      </el-table>
      <p class="block__desc" style="margin: 12px 0 0">
        恢复前会先为当前数据库生成一份安全快照（文件名以 <code>pre-restore-</code> 开头），
        恢复失败时保持现有数据不变。恢复旧版本备份后会自动执行数据库升级。
      </p>
    </section>

    <section class="block cf-card">
      <h2 class="block__title">缩略图缓存</h2>
      <p class="block__desc">
        缩略图只写入应用缓存目录，可随时删除并在浏览时重新生成。清理不会改动作品目录中的原始文件，
        也不会删除素材索引。
      </p>
      <div class="path-row">
        <el-input :model-value="cache.stats?.directory ?? ''" readonly />
        <el-button :loading="cache.clearing" @click="clearCache">清理全部缓存</el-button>
      </div>

      <div class="cache-usage">
        <el-progress
          :percentage="Math.round(cache.usageRatio * 100)"
          :stroke-width="8"
          :show-text="false"
          :status="cache.usageRatio > 0.9 ? 'warning' : undefined"
        />
        <span class="cf-muted">
          已占用 {{ formatBytes(cache.stats?.totalBytes ?? 0) }}
          / 上限 {{ formatBytes(cache.stats?.limitBytes ?? 0) }}
          · {{ cache.stats?.fileCount ?? 0 }} 个文件
        </span>
      </div>

      <div class="limit-row">
        <span class="cf-muted">容量上限</span>
        <el-input-number
          v-model="cache.limitMiB"
          :min="cache.minMiB"
          :max="cache.maxMiB"
          :step="256"
          controls-position="right"
        />
        <span class="cf-muted">MiB（{{ cache.minMiB }}—{{ cache.maxMiB }}）</span>
        <el-button :loading="cache.saving" @click="saveCacheLimit">保存</el-button>
      </div>

      <el-alert
        v-if="cache.error"
        class="inline-error"
        type="error"
        :closable="false"
        show-icon
        :title="cache.error"
      />
      <p class="block__desc" style="margin: 12px 0 0">
        超出上限时，应用会按最久未使用的顺序清理旧缩略图；刚生成、可能仍在写入的文件会被跳过。
      </p>
    </section>

    <section class="block cf-card">
      <h2 class="block__title">数据与日志</h2>
      <el-descriptions :column="1" border>
        <el-descriptions-item label="应用版本">
          {{ settings.info?.version ?? '—' }}
        </el-descriptions-item>
        <el-descriptions-item label="Tauri 版本">
          {{ settings.info?.tauriVersion ?? '—' }}
        </el-descriptions-item>
        <el-descriptions-item label="数据库位置">
          <div class="copy-row">
            <span class="mono">{{ settings.info?.databasePath ?? '—' }}</span>
            <el-button
              size="small"
              text
              :disabled="!settings.info"
              @click="copy(settings.info?.databasePath)"
            >
              复制
            </el-button>
          </div>
        </el-descriptions-item>
        <el-descriptions-item label="日志目录">
          <div class="copy-row">
            <span class="mono">{{ settings.info?.logDirectory ?? '—' }}</span>
            <el-button
              size="small"
              text
              :disabled="!settings.info"
              @click="copy(settings.info?.logDirectory)"
            >
              复制
            </el-button>
          </div>
        </el-descriptions-item>
      </el-descriptions>
      <p class="block__desc" style="margin-top: 12px">
        数据库保存的是作品与素材索引，真实文件始终保存在你选择的磁盘目录中。
      </p>
    </section>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useBackupStore } from '@/stores/backup'
import { useCacheStore } from '@/stores/cache'
import { useSettingsStore } from '@/stores/settings'
import { isTauriRuntime } from '@/services/ipc'
import type { BackupEntry } from '@/types'
import { formatBytes, formatDateTime } from '@/utils/format'

const settings = useSettingsStore()
const backup = useBackupStore()
const cache = useCacheStore()

async function chooseRoot(): Promise<void> {
  const ok = await settings.chooseWorkspaceRoot()
  if (ok) ElMessage.success('作品根目录已保存')
  else if (settings.error) ElMessage.error(settings.error)
}

async function chooseBackupDirectory(): Promise<void> {
  const ok = await backup.chooseDirectory()
  if (ok) ElMessage.success('备份目录已更新')
  else if (backup.error) ElMessage.error(backup.error)
}

async function createBackup(): Promise<void> {
  const ok = await backup.create()
  if (ok) ElMessage.success('备份已创建并通过校验')
  else ElMessage.error(backup.error ?? '备份失败')
}

async function verifyBackup(entry: BackupEntry): Promise<void> {
  const result = await backup.verify(entry)
  if (!result) {
    ElMessage.error(backup.error ?? '校验失败')
    return
  }
  if (result.valid) {
    ElMessage.success(`校验通过：结构版本 ${result.schemaVersion}，完整性与外键均正常`)
  } else {
    ElMessage.error(result.problem ?? '备份无效')
  }
}

async function restoreBackup(entry: BackupEntry): Promise<void> {
  try {
    await ElMessageBox.confirm(
      `用「${entry.fileName}」覆盖当前数据库？\n\n` +
        '系统会先为当前数据库生成安全快照。恢复完成后界面会自动刷新，' +
        '作品中未包含在备份里的新记录将会消失。',
      '恢复数据库',
      {
        confirmButtonText: '恢复',
        cancelButtonText: '取消',
        type: 'warning',
        confirmButtonClass: 'el-button--danger',
      },
    )
  } catch {
    return
  }

  const outcome = await backup.restore(entry)
  if (!outcome) {
    ElMessage.error(backup.error ?? '恢复失败')
    return
  }
  ElMessage.success(`恢复完成，结构版本 ${outcome.backupSchemaVersion} → ${outcome.currentSchemaVersion}`)
  // 恢复后重新加载界面状态，避免残留旧数据
  setTimeout(() => window.location.reload(), 800)
}

async function clearCache(): Promise<void> {
  try {
    await ElMessageBox.confirm(
      '清理全部缩略图缓存？清理后浏览素材时会重新生成，不影响原文件与素材记录。',
      '清理缓存',
      { confirmButtonText: '清理', cancelButtonText: '取消', type: 'warning' },
    )
  } catch {
    return
  }
  const result = await cache.clear()
  if (result) {
    ElMessage.success(
      `已清理 ${result.removedFiles} 个文件，释放 ${formatBytes(result.freedBytes)}`,
    )
  } else {
    ElMessage.error(cache.error ?? '清理失败')
  }
}

async function saveCacheLimit(): Promise<void> {
  const ok = await cache.saveLimit()
  if (ok) ElMessage.success(`容量上限已设为 ${cache.limitMiB} MiB`)
  else ElMessage.error(cache.error ?? '保存失败')
}

async function copy(value: string | undefined): Promise<void> {
  if (!value) return
  try {
    await navigator.clipboard.writeText(value)
    ElMessage.success('已复制')
  } catch {
    ElMessage.warning('无法写入剪贴板')
  }
}

onMounted(async () => {
  if (!isTauriRuntime()) return
  await Promise.all([settings.load(), backup.load(), cache.load()])
})
</script>

<style scoped>
.notice {
  margin-bottom: 16px;
}

.block {
  padding: 20px;
  margin-bottom: 16px;
}

.block__title {
  margin: 0 0 6px;
  font-size: 15px;
  font-weight: 600;
}

.block__desc {
  margin: 0 0 14px;
  font-size: 12px;
  color: var(--cf-text-muted);
  line-height: 1.7;
}

.path-row {
  display: flex;
  gap: 8px;
}

.copy-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.inline-error {
  margin-top: 12px;
}

.cache-usage {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 14px;
}

.limit-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
}

.backup-table {
  margin-top: 14px;
}

.mono {
  font-family: 'Cascadia Mono', Consolas, monospace;
  font-size: 12px;
  word-break: break-all;
}
</style>
