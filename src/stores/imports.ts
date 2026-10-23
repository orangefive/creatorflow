import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { importApi } from '@/services/api'
import { onImportProgress } from '@/services/events'
import { toIpcError } from '@/services/ipc'
import type {
  ImportJob,
  ImportProgressEvent,
  PrepareImportResult,
  SectionKey,
} from '@/types'

const TERMINAL_STATUSES = new Set(['completed', 'failed', 'cancelled'])

export const useImportsStore = defineStore('imports', () => {
  const jobs = ref<ImportJob[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)
  const panelVisible = ref(false)
  /** 每次有任务完成时自增，供列表页监听后刷新素材 */
  const completedSignal = ref(0)
  let unlisten: UnlistenFn | null = null

  const activeJobs = computed(() =>
    jobs.value.filter((job) => !TERMINAL_STATUSES.has(job.status)),
  )
  const failedJobs = computed(() =>
    jobs.value.filter((job) => job.status === 'failed' || job.status === 'cancelled'),
  )
  const hasActiveJobs = computed(() => activeJobs.value.length > 0)

  function upsert(job: ImportJob): void {
    const index = jobs.value.findIndex((item) => item.id === job.id)
    if (index >= 0) {
      jobs.value.splice(index, 1, job)
    } else {
      jobs.value.unshift(job)
    }
  }

  function applyProgress(event: ImportProgressEvent): void {
    const index = jobs.value.findIndex((item) => item.id === event.jobId)
    if (index < 0) return
    const existing = jobs.value[index]
    if (!existing) return
    const wasTerminal = TERMINAL_STATUSES.has(existing.status)
    jobs.value.splice(index, 1, {
      ...existing,
      status: event.status,
      bytesCopied: event.bytesCopied,
      sourceSizeBytes: event.totalBytes ?? existing.sourceSizeBytes,
      errorCode: event.errorCode,
      errorMessage: event.errorMessage,
      targetRelativePath: event.targetRelativePath ?? existing.targetRelativePath,
    })
    if (!wasTerminal && event.status === 'completed') {
      completedSignal.value += 1
    }
  }

  async function attachListener(): Promise<void> {
    if (unlisten) return
    unlisten = await onImportProgress(applyProgress)
  }

  function detachListener(): void {
    if (unlisten) {
      unlisten()
      unlisten = null
    }
  }

  async function refresh(workId: string | null): Promise<void> {
    loading.value = true
    try {
      // 只保留最近 50 条，避免长期使用后队列面板无限增长
      jobs.value = (await importApi.listJobs(workId)).slice(0, 50)
    } catch (cause) {
      error.value = toIpcError(cause).message
    } finally {
      loading.value = false
    }
  }

  const skipped = ref<PrepareImportResult['skipped']>([])

  async function prepare(workId: string, sourcePaths: string[]): Promise<PrepareImportResult> {
    error.value = null
    try {
      const result = await importApi.prepare(workId, sourcePaths)
      skipped.value = result.skipped
      return result
    } catch (cause) {
      error.value = toIpcError(cause).message
      skipped.value = []
      return { candidates: [], skipped: [] }
    }
  }

  async function start(
    workId: string,
    sectionKey: SectionKey,
    sourcePaths: string[],
  ): Promise<number> {
    error.value = null
    try {
      const created = await importApi.start(workId, sectionKey, sourcePaths)
      created.forEach(upsert)
      if (created.length > 0) panelVisible.value = true
      return created.length
    } catch (cause) {
      error.value = toIpcError(cause).message
      return 0
    }
  }

  async function cancel(jobId: string): Promise<void> {
    try {
      await importApi.cancel(jobId)
    } catch (cause) {
      error.value = toIpcError(cause).message
    }
  }

  async function retry(jobId: string): Promise<void> {
    error.value = null
    try {
      const job = await importApi.retry(jobId)
      upsert(job)
    } catch (cause) {
      error.value = toIpcError(cause).message
    }
  }

  function dismiss(jobId: string): void {
    jobs.value = jobs.value.filter((job) => job.id !== jobId)
  }

  return {
    jobs,
    loading,
    error,
    skipped,
    panelVisible,
    completedSignal,
    activeJobs,
    failedJobs,
    hasActiveJobs,
    attachListener,
    detachListener,
    refresh,
    prepare,
    start,
    cancel,
    retry,
    dismiss,
    applyProgress,
  }
})
