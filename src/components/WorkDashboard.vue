<template>
  <div v-loading="loading" class="dashboard">
    <el-alert
      v-if="error"
      class="notice"
      type="error"
      :closable="false"
      show-icon
      :title="error"
    />

    <section class="summary cf-card">
      <div class="summary__main">
        <div class="summary__title">
          <h2>{{ dashboard?.work.name ?? '作品' }}</h2>
          <WorkStatusTag v-if="dashboard" :status="dashboard.work.status" />
        </div>
        <p class="summary__meta cf-muted">
          创建 {{ formatDateTime(dashboard?.work.createdAt) }} · 更新
          {{ formatDateTime(dashboard?.work.updatedAt) }} · {{ sections.length }} 个分区
        </p>
      </div>

      <div class="summary__progress">
        <div class="progress-label">任务完成率（按任务数量计算）</div>
        <template v-if="summary && summary.completionRate !== null">
          <el-progress
            :percentage="Math.round(summary.completionRate * 100)"
            :stroke-width="10"
            :show-text="false"
          />
          <div class="progress-value">
            {{ Math.round(summary.completionRate * 100) }}%
            <span class="cf-muted">（{{ summary.done }} / {{ countable }}）</span>
          </div>
        </template>
        <div v-else class="progress-empty">尚无任务</div>
      </div>
    </section>

    <div class="stats">
      <div class="stat cf-card">
        <div class="stat__value">{{ summary?.total ?? 0 }}</div>
        <div class="stat__label">总任务数</div>
      </div>
      <div class="stat cf-card">
        <div class="stat__value">{{ openCount }}</div>
        <div class="stat__label">未完成</div>
      </div>
      <div class="stat cf-card" :class="{ 'stat--danger': (summary?.overdue ?? 0) > 0 }">
        <div class="stat__value">{{ summary?.overdue ?? 0 }}</div>
        <div class="stat__label">逾期</div>
      </div>
      <div class="stat cf-card">
        <div class="stat__value">{{ summary?.done ?? 0 }}</div>
        <div class="stat__label">已完成</div>
      </div>
    </div>

    <div class="columns">
      <section class="cf-card column">
        <header class="column__head">
          <h3>待办任务</h3>
          <div class="column__actions">
            <el-button size="small" @click="emit('open-tasks')">查看全部任务</el-button>
            <el-button size="small" type="primary" :disabled="archived" @click="emit('create-task')">
              新建任务
            </el-button>
          </div>
        </header>

        <ul v-if="dashboard && dashboard.upcoming.length > 0" class="task-list">
          <li v-for="task in dashboard.upcoming" :key="task.id" class="task-item">
            <el-checkbox :model-value="false" @change="completeTask(task)" />
            <div class="task-item__body">
              <div class="task-item__title">
                {{ task.title }}
                <el-tag
                  v-if="task.isOverdue"
                  class="task-item__tag"
                  type="danger"
                  size="small"
                >
                  逾期
                </el-tag>
                <el-tag
                  :type="TASK_PRIORITY_META[task.priority].type"
                  class="task-item__tag"
                  size="small"
                  effect="plain"
                >
                  {{ TASK_PRIORITY_META[task.priority].label }}
                </el-tag>
              </div>
              <div class="task-item__meta cf-muted">
                {{ TASK_STATUS_META[task.status].label }} · 截止 {{ formatDue(task.dueAt) }}
              </div>
            </div>
          </li>
        </ul>
        <div v-else class="column__empty">
          <p>还没有任务。</p>
          <p class="cf-muted">
            新建作品时可以选择模板，自动生成初始任务；已创建的作品可以手动添加任务。
          </p>
        </div>
      </section>

      <section class="cf-card column">
        <header class="column__head">
          <h3>最近导入的素材</h3>
          <el-button size="small" text @click="emit('open-section', 'documents')">进入分区</el-button>
        </header>

        <ul v-if="dashboard && dashboard.recentAssets.length > 0" class="asset-list">
          <li
            v-for="asset in dashboard.recentAssets"
            :key="asset.id"
            class="asset-item"
            @click="emit('open-section', asset.sectionKey ?? 'documents')"
          >
            <div class="asset-thumb">
              <img v-if="asset.thumbnailPath" :src="thumbnailUrl(asset.thumbnailPath) ?? undefined" alt="" />
              <component v-else :is="iconFor(asset.mediaKind)" :size="16" :stroke-width="1.7" />
            </div>
            <div class="asset-body">
              <div class="asset-name">{{ asset.displayName }}</div>
              <div class="asset-meta cf-muted">
                {{ SECTION_META[asset.sectionKey ?? 'documents'].name }} ·
                {{ formatBytes(asset.sizeBytes) }}
              </div>
            </div>
          </li>
        </ul>
        <div v-else class="column__empty">
          <p>这个作品还没有素材。</p>
          <p class="cf-muted">用顶部的分区标签切换过去即可导入文件。</p>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { AudioLines, FileText, Image as ImageIcon, Video } from '@lucide/vue'
import WorkStatusTag from '@/components/WorkStatusTag.vue'
import {
  TASK_PRIORITY_META,
  TASK_STATUS_META,
  formatDue,
} from '@/constants/tasks'
import { SECTION_META } from '@/constants/sections'
import { taskApi, thumbnailUrl } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import { useWorksStore } from '@/stores/works'
import type { MediaKind, SectionKey, Task, TaskSummary, WorkDashboard } from '@/types'
import { formatBytes, formatDateTime } from '@/utils/format'

const props = defineProps<{ workId: string }>()
const emit = defineEmits<{
  'open-tasks': []
  'create-task': []
  'open-section': [key: SectionKey]
}>()

const works = useWorksStore()
const dashboard = ref<WorkDashboard | null>(null)
const summary = ref<TaskSummary | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

const sections = computed(() => works.sections)
const archived = computed(() => dashboard.value?.work.status === 'archived')
const openCount = computed(() => {
  const value = summary.value
  return value ? value.todo + value.inProgress : 0
})
const countable = computed(() => {
  const value = summary.value
  return value ? value.todo + value.inProgress + value.done : 0
})

function iconFor(kind: MediaKind) {
  switch (kind) {
    case 'audio':
      return AudioLines
    case 'video':
      return Video
    case 'image':
      return ImageIcon
    default:
      return FileText
  }
}

async function load(): Promise<void> {
  loading.value = true
  error.value = null
  try {
    const data = await taskApi.dashboard(props.workId)
    dashboard.value = data
    summary.value = data.summary
  } catch (cause) {
    error.value = toIpcError(cause).message
    dashboard.value = null
  } finally {
    loading.value = false
  }
}

async function completeTask(task: Task): Promise<void> {
  try {
    await taskApi.setStatus(task.id, 'done')
    ElMessage.success('任务已完成')
    await load()
  } catch (cause) {
    ElMessage.error(toIpcError(cause).message)
  }
}

watch(() => props.workId, load)
onMounted(load)
</script>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.notice {
  margin: 0;
}

.summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  padding: 18px 20px;
}

.summary__title {
  display: flex;
  align-items: center;
  gap: 10px;
}

.summary__title h2 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}

.summary__meta {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--cf-text-muted);
}

.summary__progress {
  width: 280px;
  flex: 0 0 280px;
}

.progress-label {
  font-size: 12px;
  color: var(--cf-text-muted);
  margin-bottom: 8px;
}

.progress-value {
  margin-top: 6px;
  font-size: 13px;
  font-weight: 500;
}

.progress-empty {
  font-size: 15px;
  font-weight: 500;
  color: var(--cf-text-muted);
}

.stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}

.stat {
  padding: 14px 16px;
}

.stat--danger .stat__value {
  color: #d9534f;
}

.stat__value {
  font-size: 22px;
  font-weight: 600;
}

.stat__label {
  margin-top: 2px;
  font-size: 12px;
  color: var(--cf-text-muted);
}

.columns {
  display: grid;
  grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
  gap: 12px;
}

.column {
  padding: 14px 16px;
  min-height: 220px;
}

.column__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.column__head h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}

.column__actions {
  display: flex;
  gap: 6px;
}

.column__empty {
  padding: 18px 4px;
  font-size: 13px;
}

.column__empty p {
  margin: 0 0 6px;
}

.task-list,
.asset-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.task-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px 0;
  border-bottom: 1px solid var(--cf-border);
}

.task-item:last-child {
  border-bottom: none;
}

.task-item__title {
  font-size: 13px;
  font-weight: 500;
}

.task-item__tag {
  margin-left: 6px;
}

.task-item__meta {
  margin-top: 2px;
  font-size: 12px;
}

.asset-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 0;
  cursor: pointer;
  border-bottom: 1px solid var(--cf-border);
}

.asset-item:last-child {
  border-bottom: none;
}

.asset-thumb {
  width: 30px;
  height: 30px;
  border-radius: 6px;
  background: #f3f4f6;
  color: #9ca3af;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  flex: 0 0 30px;
}

.asset-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.asset-name {
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset-meta {
  font-size: 12px;
}

@media (max-width: 1100px) {
  .summary {
    flex-direction: column;
    align-items: flex-start;
  }

  .summary__progress {
    width: 100%;
    flex: 1;
  }

  .columns {
    grid-template-columns: minmax(0, 1fr);
  }

  .stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
