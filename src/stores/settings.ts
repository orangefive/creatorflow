import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { settingsApi } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import type { AppInfo, AppSettings } from '@/types'

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<AppSettings>({ workspaceRoot: null, lastOpenedWorkId: null })
  const info = ref<AppInfo | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const hasWorkspaceRoot = computed(() => Boolean(settings.value.workspaceRoot))

  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      const [loadedSettings, loadedInfo] = await Promise.all([
        settingsApi.get(),
        settingsApi.info(),
      ])
      settings.value = loadedSettings
      info.value = loadedInfo
    } catch (cause) {
      const ipcError = toIpcError(cause)
      error.value = ipcError.message
    } finally {
      loading.value = false
    }
  }

  /** 打开系统目录选择对话框，返回用户选择的路径（取消返回 null） */
  async function pickDirectory(title: string): Promise<string | null> {
    const selected = await open({ directory: true, multiple: false, title })
    if (typeof selected === 'string') return selected
    return null
  }

  async function setWorkspaceRoot(path: string): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      settings.value = await settingsApi.setWorkspaceRoot(path)
      return true
    } catch (cause) {
      const ipcError = toIpcError(cause)
      error.value = ipcError.message
      return false
    } finally {
      loading.value = false
    }
  }

  async function chooseWorkspaceRoot(): Promise<boolean> {
    const selected = await pickDirectory('选择作品根目录')
    if (!selected) return false
    return setWorkspaceRoot(selected)
  }

  async function rememberLastOpened(workId: string | null): Promise<void> {
    try {
      settings.value = await settingsApi.setLastOpenedWork(workId)
    } catch {
      // 记住上次打开的作品属于非关键体验，失败不影响主流程，故不打扰用户
    }
  }

  return {
    settings,
    info,
    loading,
    error,
    hasWorkspaceRoot,
    load,
    pickDirectory,
    setWorkspaceRoot,
    chooseWorkspaceRoot,
    rememberLastOpened,
  }
})
