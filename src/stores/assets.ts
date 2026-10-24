import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { assetApi, tagApi, thumbnailUrl } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import type {
  Asset,
  AssetSortKey,
  AssetStatus,
  MediaKind,
  RelocationResult,
  SectionKey,
  SortDirection,
  Tag,
} from '@/types'

export const useAssetsStore = defineStore('assets', () => {
  const items = ref<Asset[]>([])
  const total = ref(0)
  const missingCount = ref(0)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const query = ref('')
  const mediaKind = ref<MediaKind | null>(null)
  const status = ref<AssetStatus | null>(null)
  const tagId = ref<string | null>(null)
  const sortKey = ref<AssetSortKey>('created_at')
  const sortDirection = ref<SortDirection>('desc')
  const page = ref(1)
  const pageSize = ref(50)
  const ffmpegAvailable = ref(true)

  const pageCount = computed(() => Math.max(1, Math.ceil(total.value / pageSize.value)))
  const hasFilters = computed(
    () =>
      query.value.trim().length > 0 ||
      mediaKind.value !== null ||
      status.value !== null ||
      tagId.value !== null,
  )

  function thumbnailFor(asset: Asset): string | null {
    return thumbnailUrl(asset.thumbnailPath)
  }

  async function fetchAssets(workId: string, sectionKey: SectionKey | null): Promise<void> {
    loading.value = true
    error.value = null
    try {
      const offset = (page.value - 1) * pageSize.value
      const result = await assetApi.search({
        workId,
        sectionKey,
        mediaKind: mediaKind.value,
        status: status.value,
        tagId: tagId.value,
        keyword: query.value.trim() || undefined,
        sortKey: sortKey.value,
        sortDirection: sortDirection.value,
        limit: pageSize.value,
        offset,
      })
      items.value = result.items
      total.value = result.total

      // 分页数据无法推出整体缺失数量，单独做一次轻量统计
      const missing = await assetApi.search({
        workId,
        status: 'missing',
        limit: 1,
        offset: 0,
      })
      missingCount.value = missing.total
    } catch (cause) {
      error.value = toIpcError(cause).message
      items.value = []
      total.value = 0
    } finally {
      loading.value = false
    }
  }

  async function loadCapabilities(): Promise<void> {
    try {
      const capabilities = await assetApi.capabilities()
      ffmpegAvailable.value = capabilities.ffmpegAvailable
    } catch {
      ffmpegAvailable.value = false
    }
  }

  function setPage(next: number): void {
    const clamped = Math.min(Math.max(1, next), pageCount.value)
    if (clamped !== page.value) {
      page.value = clamped
    }
  }

  function applyFilters(): void {
    page.value = 1
  }

  function resetFilters(): void {
    query.value = ''
    mediaKind.value = null
    status.value = null
    tagId.value = null
    sortKey.value = 'created_at'
    sortDirection.value = 'desc'
    page.value = 1
  }

  async function openAsset(assetId: string): Promise<string | null> {
    try {
      await assetApi.open(assetId)
      return null
    } catch (cause) {
      return toIpcError(cause).message
    }
  }

  async function revealAsset(assetId: string): Promise<string | null> {
    try {
      await assetApi.reveal(assetId)
      return null
    } catch (cause) {
      return toIpcError(cause).message
    }
  }

  async function loadPreview(assetId: string): Promise<string | null> {
    try {
      return await assetApi.preview(assetId)
    } catch (cause) {
      error.value = toIpcError(cause).message
      return null
    }
  }

  async function relocate(
    assetId: string,
    newPath: string,
    allowMismatch = false,
  ): Promise<RelocationResult> {
    return assetApi.relocate(assetId, newPath, allowMismatch)
  }

  async function retryMetadata(assetId: string): Promise<string | null> {
    try {
      const updated = await assetApi.retryMetadata(assetId)
      const index = items.value.findIndex((asset) => asset.id === assetId)
      if (index >= 0) items.value[index] = updated
      return null
    } catch (cause) {
      return toIpcError(cause).message
    }
  }

  /** 删除素材：文件进回收站，记录从当前列表移除并刷新总数。 */
  async function deleteAsset(assetId: string): Promise<string | null> {
    try {
      await assetApi.remove(assetId)
      items.value = items.value.filter((asset) => asset.id !== assetId)
      total.value = Math.max(0, total.value - 1)
      return null
    } catch (cause) {
      return toIpcError(cause).message
    }
  }

  async function loadWorkTags(workId: string): Promise<Tag[]> {
    try {
      return await tagApi.listForWork(workId)
    } catch (cause) {
      error.value = toIpcError(cause).message
      return []
    }
  }

  /** 标签多选变更：新增名称创建并关联，移除的关联删除。 */
  async function updateAssetTags(asset: Asset, names: string[]): Promise<string | null> {
    const existing = new Map(asset.tags.map((tag) => [tag.name, tag]))
    const next = new Set(names)

    try {
      for (const name of names) {
        if (!existing.has(name)) {
          const updated = await tagApi.add(asset.id, name)
          asset.tags = updated
        }
      }
      for (const tag of existing.values()) {
        if (!next.has(tag.name)) {
          const updated = await tagApi.remove(asset.id, tag.id)
          asset.tags = updated
        }
      }
      return null
    } catch (cause) {
      return toIpcError(cause).message
    }
  }

  function reset(): void {
    items.value = []
    total.value = 0
    missingCount.value = 0
    error.value = null
    resetFilters()
  }

  return {
    items,
    total,
    missingCount,
    loading,
    error,
    query,
    mediaKind,
    status,
    tagId,
    sortKey,
    sortDirection,
    page,
    pageSize,
    pageCount,
    hasFilters,
    ffmpegAvailable,
    thumbnailFor,
    fetchAssets,
    loadCapabilities,
    setPage,
    applyFilters,
    resetFilters,
    openAsset,
    revealAsset,
    loadPreview,
    relocate,
    retryMetadata,
    deleteAsset,
    loadWorkTags,
    updateAssetTags,
    reset,
  }
})
