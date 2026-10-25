import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { backupApi } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import { useSettingsStore } from '@/stores/settings'
import type { BackupEntry, BackupOverview, BackupVerification, RestoreOutcome } from '@/types'

export const useBackupStore = defineStore('backup', () => {
  const overview = ref<BackupOverview | null>(null)
  const loading = ref(false)
  const creating = ref(false)
  const restoring = ref(false)
  const error = ref<string | null>(null)

  const entries = computed(() => overview.value?.entries ?? [])
  const directory = computed(() => overview.value?.directory ?? '')
  const databasePath = computed(() => overview.value?.databasePath ?? '')

  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      overview.value = await backupApi.overview()
    } catch (cause) {
      error.value = toIpcError(cause).message
    } finally {
      loading.value = false
    }
  }

  async function create(): Promise<boolean> {
    creating.value = true
    error.value = null
    try {
      const entry = await backupApi.create()
      await load()
      return entry.valid
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    } finally {
      creating.value = false
    }
  }

  async function verify(entry: BackupEntry): Promise<BackupVerification | null> {
    error.value = null
    try {
      return await backupApi.verify(entry.path)
    } catch (cause) {
      error.value = toIpcError(cause).message
      return null
    }
  }

  async function restore(entry: BackupEntry): Promise<RestoreOutcome | null> {
    restoring.value = true
    error.value = null
    try {
      return await backupApi.restore(entry.path)
    } catch (cause) {
      error.value = toIpcError(cause).message
      return null
    } finally {
      restoring.value = false
    }
  }

  /** 选择并保存备份目录；取消返回 false。 */
  async function chooseDirectory(): Promise<boolean> {
    const settings = useSettingsStore()
    const selected = await settings.pickDirectory('选择数据库备份目录')
    if (!selected) return false
    error.value = null
    try {
      await backupApi.setDirectory(selected)
      await load()
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  return {
    overview,
    entries,
    directory,
    databasePath,
    loading,
    creating,
    restoring,
    error,
    load,
    create,
    verify,
    restore,
    chooseDirectory,
  }
})
