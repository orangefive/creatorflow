<template>
  <div class="task-panel">
    <div class="task-toolbar">
      <el-select
        :model-value="tasks.query.status ?? ''"
        class="filter"
        placeholder="全部状态"
        clearable
        @update:model-value="onStatusFilter"
      >
        <el-option
          v-for="option in TASK_STATUS_OPTIONS"
          :key="option.value"
          :label="option.label"
          :value="option.value"
        />
      </el-select>
      <el-select
        :model-value="tasks.query.priority ?? ''"
        class="filter"
        placeholder="全部优先级"
        clearable
        @update:model-value="onPriorityFilter"
      >
        <el-option
          v-for="option in TASK_PRIORITY_OPTIONS"
          :key="option.value"
          :label="option.label"
          :value="option.value"
        />
      </el-select>
      <el-select v-model="tasks.query.sortKey" class="filter" @change="reload">
        <el-option
          v-for="option in TASK_SORT_OPTIONS"
          :key="option.value"
          :label="option.label"
          :value="option.value"
        />
      </el-select>
      <el-checkbox :model-value="tasks.query.overdueOnly" @change="onOverdueToggle">
        只看逾期
      </el-checkbox>
      <span class="cf-spacer" />
      <span class="cf-muted">
        共 {{ tasks.summary?.total ?? 0 }} 个任务 · 完成率按任务数量计算
      </span>
      <el-button :icon="RefreshCw" :loading="tasks.loading" @click="reload">刷新</el-button>
      <el-button type="primary" :icon="Plus" @click="openCreate">新建任务</el-button>
    </div>

    <el-alert
      v-if="tasks.error"
      class="notice"
      type="error"
      :closable="false"
      show-icon
      :title="tasks.error"
    />

    <div class="cf-card">
      <el-table
        v-loading="tasks.loading"
        :data="tasks.items"
        row-key="id"
        :empty-text="emptyText"
      >
        <el-table-column label="状态" width="96">
          <template #default="{ row }">
            <el-tag :type="TASK_STATUS_META[row.status as TaskStatus].type" size="small">
              {{ TASK_STATUS_META[row.status as TaskStatus].label }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="任务" min-width="260">
          <template #default="{ row }">
            <div class="task-title">{{ row.title }}</div>
            <div v-if="row.description" class="task-desc">{{ row.description }}</div>
          </template>
        </el-table-column>
        <el-table-column label="优先级" width="96">
          <template #default="{ row }">
            <el-tag :type="TASK_PRIORITY_META[row.priority as TaskPriority].type" size="small" effect="plain">
              {{ TASK_PRIORITY_META[row.priority as TaskPriority].label }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="截止时间" width="170">
          <template #default="{ row }">
            <span :class="{ 'is-overdue': row.isOverdue }">{{ formatDue(row.dueAt) }}</span>
            <el-tag v-if="row.isOverdue" class="overdue-tag" type="danger" size="small">逾期</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="208" align="right">
          <template #default="{ row }">
            <el-button
              v-if="row.status === 'done' || row.status === 'cancelled'"
              size="small"
              text
              @click="reopen(row)"
            >
              重新打开
            </el-button>
            <el-button v-else size="small" text type="primary" @click="complete(row)">
              完成
            </el-button>
            <el-button size="small" text @click="openEdit(row)">编辑</el-button>
            <el-dropdown trigger="click" @command="(command: string) => onCommand(row, command)">
              <el-button size="small" text>更多</el-button>
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item
                    command="up"
                    :disabled="tasks.query.sortKey !== 'manual'"
                  >
                    上移
                  </el-dropdown-item>
                  <el-dropdown-item
                    command="down"
                    :disabled="tasks.query.sortKey !== 'manual'"
                  >
                    下移
                  </el-dropdown-item>
                  <el-dropdown-item command="delete" divided>删除任务</el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <TaskFormDialog v-model="formVisible" :work-id="workId" :task="editing" @saved="onSaved" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, RefreshCw } from '@lucide/vue'
import TaskFormDialog from '@/components/TaskFormDialog.vue'
import {
  TASK_PRIORITY_META,
  TASK_PRIORITY_OPTIONS,
  TASK_SORT_OPTIONS,
  TASK_STATUS_META,
  TASK_STATUS_OPTIONS,
  formatDue,
} from '@/constants/tasks'
import { useTasksStore } from '@/stores/tasks'
import type { Task, TaskPriority, TaskStatus } from '@/types'

const props = defineProps<{ workId: string }>()
const emit = defineEmits<{ changed: [] }>()

const tasks = useTasksStore()
const formVisible = ref(false)
const editing = ref<Task | null>(null)

const emptyText = computed(() => {
  if (tasks.query.overdueOnly) return '没有逾期任务'
  if (tasks.query.status || tasks.query.priority) return '没有符合条件的任务'
  return '还没有任务，点右上角「新建任务」开始'
})

async function reload(): Promise<void> {
  await tasks.load(props.workId)
}

function onStatusFilter(value: string): void {
  tasks.query.status = (value || null) as TaskStatus | null
  void reload()
}

function onPriorityFilter(value: string): void {
  tasks.query.priority = (value || null) as TaskPriority | null
  void reload()
}

function onOverdueToggle(value: string | number | boolean): void {
  tasks.query.overdueOnly = Boolean(value)
  void reload()
}

function openCreate(): void {
  editing.value = null
  formVisible.value = true
}

function openEdit(task: Task): void {
  editing.value = task
  formVisible.value = true
}

async function onSaved(): Promise<void> {
  await reload()
  emit('changed')
}

async function complete(task: Task): Promise<void> {
  const ok = await tasks.setStatus(task.id, 'done')
  if (ok) {
    ElMessage.success('任务已完成')
    emit('changed')
  } else {
    ElMessage.error(tasks.error ?? '更新任务失败')
  }
}

async function reopen(task: Task): Promise<void> {
  const ok = await tasks.setStatus(task.id, 'todo')
  if (ok) {
    ElMessage.success('任务已重新打开')
    emit('changed')
  } else {
    ElMessage.error(tasks.error ?? '更新任务失败')
  }
}

function onCommand(task: Task, command: string): void {
  if (command === 'delete') {
    void remove(task)
    return
  }
  if (command === 'up' || command === 'down') {
    void move(task, command)
  }
}

async function move(task: Task, direction: 'up' | 'down'): Promise<void> {
  const ok = await tasks.move(task.id, direction)
  if (!ok) ElMessage.error(tasks.error ?? '调整顺序失败')
  else emit('changed')
}

async function remove(task: Task): Promise<void> {
  try {
    await ElMessageBox.confirm(
      `删除任务「${task.title}」？删除后无法恢复，但不会影响任何素材文件。`,
      '删除任务',
      {
        confirmButtonText: '删除',
        cancelButtonText: '取消',
        type: 'warning',
        confirmButtonClass: 'el-button--danger',
      },
    )
  } catch {
    return
  }
  const ok = await tasks.remove(task.id)
  if (ok) {
    ElMessage.success('任务已删除')
    emit('changed')
  } else {
    ElMessage.error(tasks.error ?? '删除任务失败')
  }
}

onMounted(reload)
</script>

<style scoped>
.task-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}

.filter {
  width: 136px;
}

.notice {
  margin-bottom: 12px;
}

.task-title {
  font-weight: 500;
}

.task-desc {
  margin-top: 2px;
  font-size: 12px;
  color: var(--cf-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 520px;
}

.is-overdue {
  color: #d9534f;
}

.overdue-tag {
  margin-left: 6px;
}
</style>
