import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useImportsStore } from '@/stores/imports'
import type { ImportJob, ImportProgressEvent } from '@/types'

function makeJob(overrides: Partial<ImportJob> = {}): ImportJob {
  return {
    id: 'job-1',
    workId: 'work-1',
    sectionId: 'section-1',
    sourcePath: 'C:\\素材\\脚本.md',
    fileName: '脚本.md',
    targetRelativePath: null,
    temporaryRelativePath: null,
    sourceSizeBytes: 1024,
    bytesCopied: 0,
    status: 'queued',
    errorCode: null,
    errorMessage: null,
    assetId: null,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    completedAt: null,
    ...overrides,
  }
}

function makeEvent(overrides: Partial<ImportProgressEvent> = {}): ImportProgressEvent {
  return {
    jobId: 'job-1',
    workId: 'work-1',
    status: 'copying',
    bytesCopied: 512,
    totalBytes: 1024,
    errorCode: null,
    errorMessage: null,
    targetRelativePath: '01_文档/脚本.md',
    ...overrides,
  }
}

describe('imports store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('applies progress events to the matching job', () => {
    const store = useImportsStore()
    store.jobs = [makeJob()]

    store.applyProgress(makeEvent())

    expect(store.jobs[0]?.status).toBe('copying')
    expect(store.jobs[0]?.bytesCopied).toBe(512)
    expect(store.jobs[0]?.targetRelativePath).toBe('01_文档/脚本.md')
  })

  it('ignores events for unknown jobs without throwing', () => {
    const store = useImportsStore()
    store.jobs = [makeJob()]

    store.applyProgress(makeEvent({ jobId: 'unknown' }))

    expect(store.jobs).toHaveLength(1)
    expect(store.completedSignal).toBe(0)
  })

  it('bumps completedSignal only when a job newly completes', () => {
    const store = useImportsStore()
    store.jobs = [makeJob({ status: 'copying' })]

    store.applyProgress(makeEvent({ status: 'completed', bytesCopied: 1024 }))
    expect(store.completedSignal).toBe(1)

    store.applyProgress(makeEvent({ status: 'completed', bytesCopied: 1024 }))
    expect(store.completedSignal).toBe(1)
  })

  it('tracks active and failed jobs', () => {
    const store = useImportsStore()
    store.jobs = [
      makeJob({ id: 'a', status: 'copying' }),
      makeJob({ id: 'b', status: 'failed', errorMessage: '磁盘空间不足' }),
      makeJob({ id: 'c', status: 'completed' }),
    ]

    expect(store.activeJobs.map((job) => job.id)).toEqual(['a'])
    expect(store.hasActiveJobs).toBe(true)
    expect(store.failedJobs.map((job) => job.id)).toEqual(['b'])
  })

  it('dismiss removes a finished job from the panel list', () => {
    const store = useImportsStore()
    store.jobs = [makeJob({ id: 'a', status: 'completed' }), makeJob({ id: 'b' })]

    store.dismiss('a')

    expect(store.jobs.map((job) => job.id)).toEqual(['b'])
  })
})
