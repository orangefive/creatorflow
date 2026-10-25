import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { taskApi } from '@/services/api'
import { useTasksStore } from '@/stores/tasks'
import type { Task, TaskSummary } from '@/types'

vi.mock('@/services/api', () => ({
  taskApi: {
    list: vi.fn(),
    summary: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    setStatus: vi.fn(),
    remove: vi.fn(),
    move: vi.fn(),
  },
}))

const summary: TaskSummary = {
  total: 3,
  todo: 1,
  inProgress: 1,
  done: 1,
  cancelled: 0,
  overdue: 0,
  completionRate: 1 / 3,
}

function makeTask(overrides: Partial<Task> = {}): Task {
  return {
    id: 'k1',
    workId: 'w1',
    title: '粗剪',
    description: null,
    status: 'todo',
    priority: 'normal',
    dueAt: null,
    sortOrder: 0,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    completedAt: null,
    isOverdue: false,
    ...overrides,
  }
}

describe('tasks store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('loads list together with summary', async () => {
    const store = useTasksStore()
    vi.mocked(taskApi.list).mockResolvedValue([makeTask()])
    vi.mocked(taskApi.summary).mockResolvedValue(summary)

    await store.load('w1')

    expect(store.items).toHaveLength(1)
    expect(store.summary?.completionRate).toBeCloseTo(1 / 3)
    expect(store.error).toBeNull()
  })

  it('keeps the previous state and reports the error when loading fails', async () => {
    const store = useTasksStore()
    vi.mocked(taskApi.list).mockRejectedValue({ code: 'not_found', message: '作品不存在' })
    vi.mocked(taskApi.summary).mockResolvedValue(summary)

    await store.load('missing')

    expect(store.items).toHaveLength(0)
    expect(store.error).toBe('作品不存在')
  })

  it('refreshes the list after creating a task', async () => {
    const store = useTasksStore()
    vi.mocked(taskApi.list).mockResolvedValue([makeTask()])
    vi.mocked(taskApi.summary).mockResolvedValue(summary)
    vi.mocked(taskApi.create).mockResolvedValue(makeTask())

    store.workId = 'w1'
    const ok = await store.create({ title: '粗剪' })

    expect(ok).toBe(true)
    expect(taskApi.create).toHaveBeenCalledWith('w1', { title: '粗剪' })
    expect(taskApi.list).toHaveBeenCalled()
  })

  it('does not report success when creating fails', async () => {
    const store = useTasksStore()
    store.workId = 'w1'
    vi.mocked(taskApi.create).mockRejectedValue({ code: 'invalid_input', message: '任务标题不能为空' })

    const ok = await store.create({ title: '' })

    expect(ok).toBe(false)
    expect(store.error).toBe('任务标题不能为空')
  })

  it('switches status through the api', async () => {
    const store = useTasksStore()
    store.workId = 'w1'
    vi.mocked(taskApi.setStatus).mockResolvedValue(makeTask({ status: 'done' }))
    vi.mocked(taskApi.list).mockResolvedValue([makeTask({ status: 'done' })])
    vi.mocked(taskApi.summary).mockResolvedValue({ ...summary, done: 2 })

    const ok = await store.setStatus('k1', 'done')

    expect(ok).toBe(true)
    expect(taskApi.setStatus).toHaveBeenCalledWith('k1', 'done')
    expect(store.summary?.done).toBe(2)
  })
})
