import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { assetApi } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import type { Asset, SectionKey } from '@/types'

export type AssetSortKey = 'name' | 'size' | 'createdAt'
export type SortDirection = 'asc' | 'desc'

export const useAssetsStore = defineStore('assets', () => {
  const assets = ref<Asset[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)
  const query = ref('')
  const sortKey = ref<AssetSortKey>('createdAt')
  const sortDirection = ref<SortDirection>('desc')

  const filteredAssets = computed(() => {
    const keyword = query.value.trim().toLowerCase()
    const matched = keyword
      ? assets.value.filter(
          (asset) =>
            asset.displayName.toLowerCase().includes(keyword) ||
            (asset.extension ?? '').toLowerCase() === keyword.replace(/^\./, ''),
        )
      : [...assets.value]

    const direction = sortDirection.value === 'asc' ? 1 : -1
    return matched.sort((left, right) => {
      switch (sortKey.value) {
        case 'name':
          return left.displayName.localeCompare(right.displayName, 'zh-Hans-CN') * direction
        case 'size':
          return (left.sizeBytes - right.sizeBytes) * direction
        case 'createdAt':
        default:
          return left.createdAt.localeCompare(right.createdAt) * direction
      }
    })
  })

  const imageAssets = computed(() =>
    assets.value.filter((asset) => asset.mediaKind === 'image'),
  )
  const missingCount = computed(
    () => assets.value.filter((asset) => asset.status === 'missing').length,
  )

  async function fetchAssets(workId: string, sectionKey: SectionKey | null): Promise<void> {
    loading.value = true
    error.value = null
    try {
      assets.value = await assetApi.list(workId, sectionKey)
    } catch (cause) {
      error.value = toIpcError(cause).message
      assets.value = []
    } finally {
      loading.value = false
    }
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

  function reset(): void {
    assets.value = []
    error.value = null
    query.value = ''
  }

  return {
    assets,
    loading,
    error,
    query,
    sortKey,
    sortDirection,
    filteredAssets,
    imageAssets,
    missingCount,
    fetchAssets,
    openAsset,
    revealAsset,
    loadPreview,
    reset,
  }
})
