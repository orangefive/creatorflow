import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { templateApi } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import type { TemplateInput, WorkTemplate } from '@/types'

export const useTemplatesStore = defineStore('templates', () => {
  const items = ref<WorkTemplate[]>([])
  const defaultId = ref<string | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const builtin = computed(() => items.value.filter((template) => template.isBuiltin))
  const custom = computed(() => items.value.filter((template) => !template.isBuiltin))
  const defaultTemplate = computed(
    () => items.value.find((template) => template.id === defaultId.value) ?? null,
  )

  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      const [list, fallback] = await Promise.all([templateApi.list(), templateApi.default()])
      items.value = list
      defaultId.value = fallback.id
    } catch (cause) {
      error.value = toIpcError(cause).message
    } finally {
      loading.value = false
    }
  }

  async function create(input: TemplateInput): Promise<WorkTemplate | null> {
    error.value = null
    try {
      const created = await templateApi.create(input)
      await load()
      return created
    } catch (cause) {
      error.value = toIpcError(cause).message
      return null
    }
  }

  async function update(templateId: string, input: TemplateInput): Promise<boolean> {
    error.value = null
    try {
      await templateApi.update(templateId, input)
      await load()
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  async function copy(templateId: string, name?: string): Promise<WorkTemplate | null> {
    error.value = null
    try {
      const created = await templateApi.copy(templateId, name)
      await load()
      return created
    } catch (cause) {
      error.value = toIpcError(cause).message
      return null
    }
  }

  async function remove(templateId: string): Promise<boolean> {
    error.value = null
    try {
      await templateApi.remove(templateId)
      await load()
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  async function setDefault(templateId: string): Promise<boolean> {
    error.value = null
    try {
      await templateApi.setDefault(templateId)
      await load()
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  return {
    items,
    builtin,
    custom,
    defaultId,
    defaultTemplate,
    loading,
    error,
    load,
    create,
    update,
    copy,
    remove,
    setDefault,
  }
})
