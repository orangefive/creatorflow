import { defineStore } from 'pinia'
import { reactive, ref } from 'vue'
import { taskApi } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import type { Task, TaskInput, TaskQuery, TaskStatus, TaskSummary } from '@/types'

export const useTasksStore = defineStore('tasks', () => {
  const workId = ref<string | null>(null)
  const items = ref<Task[]>([])
  const summary = ref<TaskSummary | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const query = reactive<TaskQuery>({
    status: null,
    priority: null,
    overdueOnly: false,
    sortKey: 'manual',
    sortDirection: 'asc',
  })

  async function load(target: string): Promise<void> {
    workId.value = target
    loading.value = true
    error.value = null
    try {
      const [list, stats] = await Promise.all([
        taskApi.list(target, { ...query }),
        taskApi.summary(target),
      ])
      items.value = list
      summary.value = stats
    } catch (cause) {
      error.value = toIpcError(cause).message
      items.value = []
    } finally {
      loading.value = false
    }
  }

  /** 只刷新统计，用于工作台与任务页之间同步进度。 */
  async function refreshSummary(): Promise<void> {
    if (!workId.value) return
    try {
      summary.value = await taskApi.summary(workId.value)
    } catch {
      // 统计失败不影响列表展示
    }
  }

  async function create(input: TaskInput): Promise<boolean> {
    if (!workId.value) return false
    error.value = null
    try {
      await taskApi.create(workId.value, input)
      await load(workId.value)
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  async function update(taskId: string, input: TaskInput): Promise<boolean> {
    error.value = null
    try {
      await taskApi.update(taskId, input)
      if (workId.value) await load(workId.value)
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  async function setStatus(taskId: string, status: TaskStatus): Promise<boolean> {
    error.value = null
    try {
      await taskApi.setStatus(taskId, status)
      if (workId.value) await load(workId.value)
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  async function remove(taskId: string): Promise<boolean> {
    error.value = null
    try {
      await taskApi.remove(taskId)
      if (workId.value) await load(workId.value)
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  async function move(taskId: string, direction: 'up' | 'down'): Promise<boolean> {
    error.value = null
    try {
      await taskApi.move(taskId, direction)
      if (workId.value) await load(workId.value)
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  function reset(): void {
    items.value = []
    summary.value = null
    error.value = null
  }

  return {
    workId,
    items,
    summary,
    loading,
    error,
    query,
    load,
    refreshSummary,
    create,
    update,
    setStatus,
    remove,
    move,
    reset,
  }
})
