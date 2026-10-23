<template>
  <el-drawer
    v-model="visible"
    title="导入任务"
    size="440px"
    :with-header="true"
    append-to-body
  >
    <div v-if="jobs.length === 0" class="cf-empty">
      <p>暂无导入任务</p>
      <p style="font-size: 12px">在作品分区中选择文件后，任务会出现在这里。</p>
    </div>

    <div v-else class="queue">
      <div v-for="job in jobs" :key="job.id" class="queue-item">
        <div class="queue-item__head">
          <span class="queue-item__name" :title="job.sourcePath">{{ job.fileName }}</span>
          <el-tag size="small" :type="statusType(job.status)">{{ statusLabel(job.status) }}</el-tag>
        </div>

        <div class="queue-item__meta">
          <span>{{ formatProgress(job.bytesCopied, job.sourceSizeBytes) }}</span>
          <span v-if="job.targetRelativePath" class="cf-muted">
            → {{ job.targetRelativePath }}
          </span>
        </div>

        <el-progress
          :percentage="progressPercent(job.bytesCopied, job.sourceSizeBytes)"
          :status="job.status === 'failed' ? 'exception' : job.status === 'completed' ? 'success' : undefined"
          :stroke-width="6"
        />

        <p v-if="job.errorMessage" class="queue-item__error">{{ job.errorMessage }}</p>

        <div class="queue-item__actions">
          <el-button
            v-if="job.status === 'copying' || job.status === 'queued' || job.status === 'verifying'"
            size="small"
            text
            @click="cancel(job.id)"
          >
            取消
          </el-button>
          <el-button
            v-if="job.status === 'failed' || job.status === 'cancelled'"
            size="small"
            text
            type="primary"
            @click="retry(job.id)"
          >
            重试
          </el-button>
          <el-button
            v-if="job.status === 'completed' || job.status === 'failed' || job.status === 'cancelled'"
            size="small"
            text
            @click="dismiss(job.id)"
          >
            移除记录
          </el-button>
        </div>
      </div>
    </div>
  </el-drawer>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ElMessage } from 'element-plus'
import { useImportsStore } from '@/stores/imports'
import type { ImportStatus } from '@/types'
import { formatProgress, progressPercent } from '@/utils/format'

const store = useImportsStore()

const jobs = computed(() => store.jobs)
const visible = computed({
  get: () => store.panelVisible,
  set: (value: boolean) => {
    store.panelVisible = value
  },
})

function statusLabel(status: ImportStatus): string {
  switch (status) {
    case 'queued':
      return '排队中'
    case 'copying':
      return '复制中'
    case 'verifying':
      return '校验中'
    case 'completed':
      return '已完成'
    case 'failed':
      return '失败'
    case 'cancelled':
      return '已取消'
    default:
      return status
  }
}

function statusType(status: ImportStatus): 'success' | 'warning' | 'danger' | 'info' | 'primary' {
  switch (status) {
    case 'completed':
      return 'success'
    case 'failed':
      return 'danger'
    case 'cancelled':
      return 'info'
    case 'copying':
    case 'verifying':
      return 'primary'
    default:
      return 'warning'
  }
}

async function cancel(jobId: string): Promise<void> {
  await store.cancel(jobId)
}

async function retry(jobId: string): Promise<void> {
  await store.retry(jobId)
  if (store.error) {
    ElMessage.error(store.error)
  }
}

function dismiss(jobId: string): void {
  store.dismiss(jobId)
}
</script>

<style scoped>
.queue {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.queue-item {
  border: 1px solid var(--cf-border);
  border-radius: 8px;
  padding: 12px;
  background: #fdfdfe;
}

.queue-item__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 6px;
}

.queue-item__name {
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.queue-item__meta {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  font-size: 12px;
  color: var(--cf-text-muted);
  margin-bottom: 8px;
}

.queue-item__error {
  margin: 8px 0 0;
  font-size: 12px;
  color: #c45656;
  word-break: break-all;
}

.queue-item__actions {
  display: flex;
  justify-content: flex-end;
  gap: 4px;
  margin-top: 6px;
}
</style>
