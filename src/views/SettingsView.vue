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
import { ElMessage } from 'element-plus'
import { useSettingsStore } from '@/stores/settings'
import { isTauriRuntime } from '@/services/ipc'

const settings = useSettingsStore()

async function chooseRoot(): Promise<void> {
  const ok = await settings.chooseWorkspaceRoot()
  if (ok) ElMessage.success('作品根目录已保存')
  else if (settings.error) ElMessage.error(settings.error)
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
  await settings.load()
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

.mono {
  font-family: 'Cascadia Mono', Consolas, monospace;
  font-size: 12px;
  word-break: break-all;
}
</style>
