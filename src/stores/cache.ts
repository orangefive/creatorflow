import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { cacheApi } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import type { CacheCleanupResult, CacheStats } from '@/types'

const MIB = 1024 * 1024

export const useCacheStore = defineStore('cache', () => {
  const stats = ref<CacheStats | null>(null)
  const loading = ref(false)
  const clearing = ref(false)
  const saving = ref(false)
  const error = ref<string | null>(null)
  /** 输入框以 MiB 为单位，避免暴露字节数 */
  const limitMiB = ref(2048)

  const usageRatio = computed(() => {
    if (!stats.value || stats.value.limitBytes === 0) return 0
    return Math.min(1, stats.value.totalBytes / stats.value.limitBytes)
  })

  const minMiB = computed(() => Math.ceil((stats.value?.minLimitBytes ?? 256 * MIB) / MIB))
  const maxMiB = computed(() => Math.floor((stats.value?.maxLimitBytes ?? 20 * 1024 * MIB) / MIB))

  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      stats.value = await cacheApi.stats()
      limitMiB.value = Math.round(stats.value.limitBytes / MIB)
    } catch (cause) {
      error.value = toIpcError(cause).message
    } finally {
      loading.value = false
    }
  }

  async function saveLimit(): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      const applied = await cacheApi.setLimit(Math.round(limitMiB.value * MIB))
      await load()
      limitMiB.value = Math.round(applied / MIB)
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    } finally {
      saving.value = false
    }
  }

  async function clear(): Promise<CacheCleanupResult | null> {
    clearing.value = true
    error.value = null
    try {
      const result = await cacheApi.clear()
      await load()
      return result
    } catch (cause) {
      error.value = toIpcError(cause).message
      return null
    } finally {
      clearing.value = false
    }
  }

  return {
    stats,
    loading,
    clearing,
    saving,
    error,
    limitMiB,
    usageRatio,
    minMiB,
    maxMiB,
    load,
    saveLimit,
    clear,
  }
})
