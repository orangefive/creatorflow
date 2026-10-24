import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { assetApi, tagApi } from '@/services/api'
import { useAssetsStore } from '@/stores/assets'
import type { Asset, AssetPage } from '@/types'

vi.mock('@/services/api', () => ({
  assetApi: {
    search: vi.fn(),
    capabilities: vi.fn(),
    open: vi.fn(),
    reveal: vi.fn(),
    preview: vi.fn(),
    relocate: vi.fn(),
    retryMetadata: vi.fn(),
    remove: vi.fn(),
  },
  tagApi: {
    listForWork: vi.fn(),
    listForAsset: vi.fn(),
    add: vi.fn(),
    remove: vi.fn(),
  },
  thumbnailUrl: (path: string | null) => path,
}))

function makeAsset(overrides: Partial<Asset> = {}): Asset {
  return {
    id: 'asset-1',
    workId: 'work-1',
    sectionId: 'section-1',
    displayName: '脚本.md',
    originalName: '脚本.md',
    relativePath: '01_文档/脚本.md',
    sourcePath: null,
    extension: 'md',
    mediaKind: 'document',
    sizeBytes: 128,
    status: 'ready',
    isFavorite: false,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    absolutePath: 'C:/works/脚本.md',
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
    ...overrides,
  }
}

function page(items: Asset[], total: number, limit = 50, offset = 0): AssetPage {
  return { items, total, limit, offset }
}

const searchMock = vi.mocked(assetApi.search)

describe('assets store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    searchMock.mockReset()
    vi.mocked(assetApi.capabilities).mockReset()
  })

  it('queries the backend with filters, sorting and pagination', async () => {
    searchMock
      .mockResolvedValueOnce(page([makeAsset()], 120, 50, 50))
      .mockResolvedValueOnce(page([], 4, 1, 0))

    const store = useAssetsStore()
    store.query = '脚本'
    store.mediaKind = 'document'
    store.status = 'ready'
    store.tagId = 'tag-1'
    store.sortKey = 'size'
    store.sortDirection = 'asc'
    store.page = 2
    await store.fetchAssets('work-1', 'documents')

    expect(searchMock).toHaveBeenNthCalledWith(1, {
      workId: 'work-1',
      sectionKey: 'documents',
      mediaKind: 'document',
      status: 'ready',
      tagId: 'tag-1',
      keyword: '脚本',
      sortKey: 'size',
      sortDirection: 'asc',
      limit: 50,
      offset: 50,
    })
    expect(store.items).toHaveLength(1)
    expect(store.total).toBe(120)
    expect(store.missingCount).toBe(4)
  })

  it('clears items and records the error when the query fails', async () => {
    searchMock.mockRejectedValueOnce(new Error('数据库不可用'))

    const store = useAssetsStore()
    await store.fetchAssets('work-1', null)

    expect(store.items).toEqual([])
    expect(store.total).toBe(0)
    expect(store.error).toBe('数据库不可用')
  })

  it('clamps page navigation to the available range', async () => {
    searchMock
      .mockResolvedValueOnce(page([], 120))
      .mockResolvedValueOnce(page([], 0, 1, 0))

    const store = useAssetsStore()
    await store.fetchAssets('work-1', null)

    store.setPage(99)
    expect(store.page).toBe(3)

    store.setPage(0)
    expect(store.page).toBe(1)
  })

  it('resets filters and paging together', () => {
    const store = useAssetsStore()
    store.query = '关键词'
    store.mediaKind = 'audio'
    store.status = 'missing'
    store.tagId = 'tag-1'
    store.page = 3

    store.resetFilters()

    expect(store.query).toBe('')
    expect(store.mediaKind).toBeNull()
    expect(store.status).toBeNull()
    expect(store.tagId).toBeNull()
    expect(store.page).toBe(1)
    expect(store.hasFilters).toBe(false)
  })

  it('adds new tag names and removes cleared ones', async () => {
    const asset = makeAsset({
      tags: [{ id: 'tag-old', name: '旧标签', assetCount: 1 }],
    })
    vi.mocked(tagApi.add).mockResolvedValue([
      { id: 'tag-old', name: '旧标签', assetCount: 1 },
      { id: 'tag-new', name: '新标签', assetCount: 1 },
    ])
    vi.mocked(tagApi.remove).mockResolvedValue([])

    const store = useAssetsStore()
    const error = await store.updateAssetTags(asset, ['新标签'])

    expect(error).toBeNull()
    expect(tagApi.add).toHaveBeenCalledWith('asset-1', '新标签')
    expect(tagApi.remove).toHaveBeenCalledWith('asset-1', 'tag-old')
  })

  it('reports tag update failures without swallowing them', async () => {
    const asset = makeAsset()
    vi.mocked(tagApi.add).mockRejectedValue(new Error('标签过长'))

    const store = useAssetsStore()
    const error = await store.updateAssetTags(asset, ['超长标签'])

    expect(error).toBe('标签过长')
  })
})
