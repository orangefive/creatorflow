import { defineStore } from 'pinia'
import { computed, reactive, ref } from 'vue'
import { assetApi } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import type { Asset, AssetQuery, AssetSortKey, AssetStatus, MediaKind, SectionKey } from '@/types'

export const GLOBAL_PAGE_SIZE_OPTIONS = [20, 50, 100, 200] as const

export const useGlobalSearchStore = defineStore('globalSearch', () => {
  const query = reactive<AssetQuery>({
    workId: null,
    sectionKey: null,
    mediaKind: null,
    status: null,
    tagId: null,
    keyword: '',
    sortKey: 'created_at',
    sortDirection: 'desc',
    limit: 50,
    offset: 0,
  })

  const items = ref<Asset[]>([])
  const total = ref(0)
  const loading = ref(false)
  const loaded = ref(false)
  const error = ref<string | null>(null)
  const missingCount = ref(0)

  const page = computed(() => Math.floor((query.offset ?? 0) / (query.limit ?? 50)) + 1)
  const pageCount = computed(() => Math.max(1, Math.ceil(total.value / (query.limit ?? 50))))
  const hasFilters = computed(
    () =>
      Boolean(query.workId) ||
      Boolean(query.sectionKey) ||
      Boolean(query.mediaKind) ||
      Boolean(query.status) ||
      Boolean(query.tagId) ||
      Boolean(query.keyword?.trim()),
  )

  async function fetchResults(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      const result = await assetApi.search({ ...query })
      items.value = result.items
      total.value = result.total
      missingCount.value = result.items.filter((asset) => asset.status === 'missing').length
      loaded.value = true
    } catch (cause) {
      error.value = toIpcError(cause).message
      items.value = []
      total.value = 0
    } finally {
      loading.value = false
    }
  }

  function applyFilters(): void {
    query.offset = 0
    void fetchResults()
  }

  function setKeyword(keyword: string): void {
    query.keyword = keyword
    applyFilters()
  }

  function setWork(workId: string | null): void {
    query.workId = workId
    applyFilters()
  }

  function setSection(sectionKey: SectionKey | null): void {
    query.sectionKey = sectionKey
    applyFilters()
  }

  function setMediaKind(mediaKind: MediaKind | null): void {
    query.mediaKind = mediaKind
    applyFilters()
  }

  function setStatus(status: AssetStatus | null): void {
    query.status = status
    applyFilters()
  }

  function setTag(tagId: string | null): void {
    query.tagId = tagId
    applyFilters()
  }

  function setSort(sortKey: AssetSortKey, sortDirection: 'asc' | 'desc'): void {
    query.sortKey = sortKey
    query.sortDirection = sortDirection
    applyFilters()
  }

  function setPage(target: number): void {
    const limit = query.limit ?? 50
    const bounded = Math.min(Math.max(1, target), pageCount.value)
    query.offset = (bounded - 1) * limit
    void fetchResults()
  }

  function setPageSize(limit: number): void {
    query.limit = limit
    query.offset = 0
    void fetchResults()
  }

  function resetFilters(): void {
    query.workId = null
    query.sectionKey = null
    query.mediaKind = null
    query.status = null
    query.tagId = null
    query.keyword = ''
    query.offset = 0
  }

  return {
    query,
    items,
    total,
    loading,
    loaded,
    error,
    missingCount,
    page,
    pageCount,
    hasFilters,
    fetchResults,
    applyFilters,
    setKeyword,
    setWork,
    setSection,
    setMediaKind,
    setStatus,
    setTag,
    setSort,
    setPage,
    setPageSize,
    resetFilters,
  }
})
