import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { workApi } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import type { CreateWorkInput, Section, Work, WorkStatus } from '@/types'

export const useWorksStore = defineStore('works', () => {
  const works = ref<Work[]>([])
  const sections = ref<Section[]>([])
  const currentWork = ref<Work | null>(null)
  const loading = ref(false)
  const creating = ref(false)
  const error = ref<string | null>(null)
  const includeArchived = ref(false)

  const activeWorks = computed(() =>
    works.value.filter((work) => work.status !== 'archived'),
  )
  const archivedWorks = computed(() =>
    works.value.filter((work) => work.status === 'archived'),
  )
  const recentWorks = computed(() =>
    [...works.value]
      .filter((work) => work.status !== 'archived')
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
      .slice(0, 6),
  )

  function upsert(work: Work): void {
    const index = works.value.findIndex((item) => item.id === work.id)
    if (index >= 0) {
      works.value.splice(index, 1, work)
    } else {
      works.value.push(work)
    }
    if (currentWork.value?.id === work.id) {
      currentWork.value = work
    }
  }

  async function fetchWorks(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      works.value = await workApi.list(includeArchived.value)
    } catch (cause) {
      error.value = toIpcError(cause).message
    } finally {
      loading.value = false
    }
  }

  async function fetchWork(workId: string): Promise<Work | null> {
    error.value = null
    try {
      const work = await workApi.get(workId)
      currentWork.value = work
      upsert(work)
      sections.value = await workApi.listSections(workId)
      return work
    } catch (cause) {
      error.value = toIpcError(cause).message
      currentWork.value = null
      sections.value = []
      return null
    }
  }

  async function createWork(input: CreateWorkInput): Promise<Work | null> {
    creating.value = true
    error.value = null
    try {
      const work = await workApi.create(input)
      upsert(work)
      return work
    } catch (cause) {
      error.value = toIpcError(cause).message
      return null
    } finally {
      creating.value = false
    }
  }

  async function renameWork(workId: string, newName: string): Promise<boolean> {
    error.value = null
    try {
      upsert(await workApi.rename(workId, newName))
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  async function archiveWork(workId: string): Promise<boolean> {
    error.value = null
    try {
      const archived = await workApi.archive(workId)
      upsert(archived)
      if (currentWork.value?.id === workId) {
        currentWork.value = archived
      }
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  async function updateStatus(workId: string, status: WorkStatus): Promise<boolean> {
    error.value = null
    try {
      upsert(await workApi.updateStatus(workId, status))
      return true
    } catch (cause) {
      error.value = toIpcError(cause).message
      return false
    }
  }

  function reset(): void {
    currentWork.value = null
    sections.value = []
  }

  return {
    works,
    sections,
    currentWork,
    loading,
    creating,
    error,
    includeArchived,
    activeWorks,
    archivedWorks,
    recentWorks,
    fetchWorks,
    fetchWork,
    createWork,
    renameWork,
    archiveWork,
    updateStatus,
    reset,
  }
})
