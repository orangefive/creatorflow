import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { assetApi } from '@/services/api'
import { useGlobalSearchStore } from '@/stores/globalSearch'
import type { Asset, AssetPage } from '@/types'

vi.mock('@/services/api', () => ({
  assetApi: { search: vi.fn() },
  thumbnailUrl: (path: string | null) => path,
}))

function makePage(total: number, items: Asset[] = []): AssetPage {
  return { items, total, limit: 50, offset: 0 }
}

function makeAsset(id: string): Asset {
  return {
    id,
    workId: 'work-1',
    sectionId: 'section-1',
    displayName: `${id}.md`,
    originalName: `${id}.md`,
    relativePath: `01_文档/${id}.md`,
    sourcePath: null,
    extension: 'md',
    mediaKind: 'document',
    sizeBytes: 10,
    status: 'ready',
    isFavorite: false,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    absolutePath: `C:/works/${id}.md`,
    width: null,
    height: null,
    durationMs: null,
    frameRate: null,
    sampleRate: null,
    channels: null,
    codec: null,
    sha256: null,
    metadataStatus: 'pending',
    metadataError: null,
    thumbnailStatus: 'pending',
    thumbnailRelativePath: null,
    thumbnailPath: null,
    thumbnailError: null,
    tags: [],
    workName: '测试作品',
    sectionKey: 'documents',
  }
}

describe('global search store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('resets offset when filters change', async () => {
    const store = useGlobalSearchStore()
    store.query.offset = 100
    vi.mocked(assetApi.search).mockResolvedValue(makePage(0))

    store.setMediaKind('audio')

    expect(store.query.offset).toBe(0)
    expect(store.query.mediaKind).toBe('audio')
    await Promise.resolve()
    expect(assetApi.search).toHaveBeenCalled()
  })

  it('keeps page within bounds and reports pagination', async () => {
    const store = useGlobalSearchStore()
    vi.mocked(assetApi.search).mockResolvedValue(makePage(120, [makeAsset('a')]))

    await store.fetchResults()

    expect(store.total).toBe(120)
    expect(store.page).toBe(1)
    expect(store.pageCount).toBe(3)

    store.setPage(99)
    expect(store.query.offset).toBe(100)
  })

  it('clears results and records the error when the query fails', async () => {
    const store = useGlobalSearchStore()
    vi.mocked(assetApi.search).mockRejectedValue({ code: 'db_error', message: '查询失败' })

    await store.fetchResults()

    expect(store.items).toHaveLength(0)
    expect(store.total).toBe(0)
    expect(store.error).toBe('查询失败')
  })

  it('resets every filter at once', async () => {
    const store = useGlobalSearchStore()
    store.query.workId = 'work-1'
    store.query.keyword = '脚本'
    store.query.status = 'missing'
    store.query.offset = 50

    store.resetFilters()

    expect(store.query.workId).toBeNull()
    expect(store.query.keyword).toBe('')
    expect(store.query.status).toBeNull()
    expect(store.query.offset).toBe(0)
  })
})
