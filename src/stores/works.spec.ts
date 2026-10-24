import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { workApi } from '@/services/api'
import { useWorksStore } from '@/stores/works'
import type { Work } from '@/types'

vi.mock('@/services/api', () => ({
  workApi: {
    list: vi.fn(),
    get: vi.fn(),
    create: vi.fn(),
    rename: vi.fn(),
    archive: vi.fn(),
    unarchive: vi.fn(),
    remove: vi.fn(),
    updateStatus: vi.fn(),
    listSections: vi.fn(),
  },
}))

function makeWork(overrides: Partial<Work> = {}): Work {
  return {
    id: 'work-1',
    name: '测试作品',
    description: null,
    status: 'in_production',
    rootPath: 'F:/作品/测试作品',
    templateKey: null,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    archivedAt: null,
    archivedFromStatus: null,
    assetCount: 0,
    ...overrides,
  }
}

describe('works store archive lifecycle', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('drops the work from the list after deletion', async () => {
    const store = useWorksStore()
    store.works = [makeWork(), makeWork({ id: 'work-2', name: '另一个作品' })]
    vi.mocked(workApi.remove).mockResolvedValue(undefined)

    const ok = await store.deleteWork('work-1')

    expect(ok).toBe(true)
    expect(store.works.map((work) => work.id)).toEqual(['work-2'])
    expect(store.error).toBeNull()
  })

  it('keeps the record and reports the error when deletion fails', async () => {
    const store = useWorksStore()
    store.works = [makeWork()]
    vi.mocked(workApi.remove).mockRejectedValue({
      code: 'io_error',
      message: '无法将文件移入回收站',
    })

    const ok = await store.deleteWork('work-1')

    expect(ok).toBe(false)
    expect(store.works).toHaveLength(1)
    expect(store.error).toBe('无法将文件移入回收站')
  })

  it('clears the current work when it is the deleted one', async () => {
    const store = useWorksStore()
    store.works = [makeWork()]
    store.currentWork = makeWork()
    store.sections = []
    vi.mocked(workApi.remove).mockResolvedValue(undefined)

    await store.deleteWork('work-1')

    expect(store.currentWork).toBeNull()
  })

  it('replaces the record with the restored status after unarchive', async () => {
    const store = useWorksStore()
    store.works = [makeWork({ status: 'archived', archivedAt: '2026-01-02T00:00:00.000Z' })]
    vi.mocked(workApi.unarchive).mockResolvedValue(
      makeWork({ status: 'in_production', archivedFromStatus: null }),
    )

    const ok = await store.unarchiveWork('work-1')

    expect(ok).toBe(true)
    const restored = store.works[0]
    expect(restored?.status).toBe('in_production')
    expect(restored?.archivedAt).toBeNull()
  })

  it('surfaces the backend message when unarchive fails', async () => {
    const store = useWorksStore()
    store.works = [makeWork({ status: 'archived' })]
    vi.mocked(workApi.unarchive).mockRejectedValue({
      code: 'invalid_input',
      message: '该作品当前不在归档状态',
    })

    const ok = await store.unarchiveWork('work-1')

    expect(ok).toBe(false)
    expect(store.error).toBe('该作品当前不在归档状态')
  })
})
