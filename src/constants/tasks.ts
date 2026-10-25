import type { TaskPriority, TaskStatus } from '@/types'

export type TagType = 'primary' | 'success' | 'info' | 'warning' | 'danger'

export const TASK_STATUS_META: Record<TaskStatus, { label: string; type: TagType }> = {
  todo: { label: '待办', type: 'info' },
  in_progress: { label: '进行中', type: 'primary' },
  done: { label: '已完成', type: 'success' },
  cancelled: { label: '已取消', type: 'info' },
}

export const TASK_PRIORITY_META: Record<TaskPriority, { label: string; type: TagType }> = {
  low: { label: '低', type: 'info' },
  normal: { label: '普通', type: 'info' },
  high: { label: '高', type: 'warning' },
  urgent: { label: '紧急', type: 'danger' },
}

export const TASK_STATUS_OPTIONS: { value: TaskStatus; label: string }[] = (
  Object.keys(TASK_STATUS_META) as TaskStatus[]
).map((value) => ({ value, label: TASK_STATUS_META[value].label }))

export const TASK_PRIORITY_OPTIONS: { value: TaskPriority; label: string }[] = (
  Object.keys(TASK_PRIORITY_META) as TaskPriority[]
).map((value) => ({ value, label: TASK_PRIORITY_META[value].label }))

export const TASK_SORT_OPTIONS = [
  { value: 'manual', label: '按手工顺序' },
  { value: 'due_at', label: '按截止时间' },
  { value: 'priority', label: '按优先级' },
  { value: 'created_at', label: '按创建时间' },
] as const

/** 截止时间的展示文案，逾期任务额外标注。 */
export function formatDue(dueAt: string | null): string {
  if (!dueAt) return '未设置'
  const date = new Date(dueAt)
  if (Number.isNaN(date.getTime())) return '未设置'
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`
}
