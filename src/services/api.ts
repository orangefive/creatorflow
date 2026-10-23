import { call } from './ipc'
import type {
  AppInfo,
  AppSettings,
  Asset,
  CreateWorkInput,
  ImportJob,
  PrepareImportResult,
  Section,
  SectionKey,
  StartupRecoveryReport,
  Work,
  WorkStatus,
} from '@/types'

export const workApi = {
  list: (includeArchived = false) =>
    call<Work[]>('list_works', { includeArchived }),
  get: (workId: string) => call<Work>('get_work', { workId }),
  create: (input: CreateWorkInput) => call<Work>('create_work', { input }),
  rename: (workId: string, newName: string) =>
    call<Work>('rename_work', { workId, newName }),
  archive: (workId: string) => call<Work>('archive_work', { workId }),
  updateStatus: (workId: string, status: WorkStatus) =>
    call<Work>('update_work_status', { workId, status }),
  listSections: (workId: string) => call<Section[]>('list_sections', { workId }),
}

export const assetApi = {
  list: (workId: string, sectionKey: SectionKey | null) =>
    call<Asset[]>('list_assets', { workId, sectionKey }),
  open: (assetId: string) => call<void>('open_asset', { assetId }),
  reveal: (assetId: string) => call<void>('reveal_asset', { assetId }),
  preview: (assetId: string) => call<string>('get_asset_preview', { assetId }),
}

export const importApi = {
  prepare: (workId: string, sourcePaths: string[]) =>
    call<PrepareImportResult>('prepare_import', { workId, sourcePaths }),
  start: (workId: string, sectionKey: SectionKey, sourcePaths: string[]) =>
    call<ImportJob[]>('start_import', { workId, sectionKey, sourcePaths }),
  listJobs: (workId: string | null) => call<ImportJob[]>('list_import_jobs', { workId }),
  cancel: (jobId: string) => call<void>('cancel_import', { jobId }),
  retry: (jobId: string) => call<ImportJob>('retry_import', { jobId }),
  startupReport: () => call<StartupRecoveryReport | null>('get_startup_recovery_report'),
}

export const settingsApi = {
  get: () => call<AppSettings>('get_settings'),
  setWorkspaceRoot: (path: string) =>
    call<AppSettings>('set_workspace_root', { path }),
  setLastOpenedWork: (workId: string | null) =>
    call<AppSettings>('set_last_opened_work', { workId }),
  info: () => call<AppInfo>('get_app_info'),
}
