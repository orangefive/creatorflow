import { convertFileSrc } from '@tauri-apps/api/core'
import { call, isTauriRuntime } from './ipc'
import type {
  AppInfo,
  BackupEntry,
  BackupOverview,
  BackupVerification,
  CacheCleanupResult,
  CacheStats,
  RestoreOutcome,
  AppSettings,
  Asset,
  AssetPage,
  AssetQuery,
  BatchOutcome,
  CreateWorkInput,
  ImportJob,
  MediaCapabilities,
  PrepareImportResult,
  RelocationResult,
  Section,
  SectionKey,
  StartupRecoveryReport,
  Tag,
  Work,
  WorkStatus,
} from '@/types'

export const workApi = {
  list: (includeArchived = false) => call<Work[]>('list_works', { includeArchived }),
  get: (workId: string) => call<Work>('get_work', { workId }),
  create: (input: CreateWorkInput) => call<Work>('create_work', { input }),
  rename: (workId: string, newName: string) => call<Work>('rename_work', { workId, newName }),
  archive: (workId: string) => call<Work>('archive_work', { workId }),
  unarchive: (workId: string) => call<Work>('unarchive_work', { workId }),
  remove: (workId: string) => call<void>('delete_work', { workId }),
  updateStatus: (workId: string, status: WorkStatus) =>
    call<Work>('update_work_status', { workId, status }),
  listSections: (workId: string) => call<Section[]>('list_sections', { workId }),
}

export const assetApi = {
  list: (workId: string, sectionKey: SectionKey | null) =>
    call<Asset[]>('list_assets', { workId, sectionKey }),
  /** 搜索、筛选、排序与分页全部由后端 SQL 完成 */
  search: (query: AssetQuery) => call<AssetPage>('search_assets', { query }),
  open: (assetId: string) => call<void>('open_asset', { assetId }),
  reveal: (assetId: string) => call<void>('reveal_asset', { assetId }),
  preview: (assetId: string) => call<string>('get_asset_preview', { assetId }),
  relocate: (assetId: string, newPath: string, allowMismatch = false) =>
    call<RelocationResult>('relocate_asset', { assetId, newPath, allowMismatch }),
  retryMetadata: (assetId: string) => call<Asset>('retry_asset_metadata', { assetId }),
  remove: (assetId: string) => call<void>('delete_asset', { assetId }),
  batchDelete: (assetIds: string[]) => call<BatchOutcome>('batch_delete_assets', { assetIds }),
  capabilities: () => call<MediaCapabilities>('get_media_capabilities'),
}

export const tagApi = {
  listForWork: (workId: string) => call<Tag[]>('list_work_tags', { workId }),
  listForAsset: (assetId: string) => call<Tag[]>('list_asset_tags', { assetId }),
  listAll: () => call<Tag[]>('list_all_tags'),
  batchAdd: (tagName: string, assetIds: string[]) =>
    call<BatchOutcome>('batch_add_tag', { tagName, assetIds }),
  batchRemove: (tagId: string, assetIds: string[]) =>
    call<BatchOutcome>('batch_remove_tag', { tagId, assetIds }),
  add: (assetId: string, name: string) => call<Tag[]>('add_asset_tag', { assetId, name }),
  remove: (assetId: string, tagId: string) =>
    call<Tag[]>('remove_asset_tag', { assetId, tagId }),
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
  setWorkspaceRoot: (path: string) => call<AppSettings>('set_workspace_root', { path }),
  setLastOpenedWork: (workId: string | null) =>
    call<AppSettings>('set_last_opened_work', { workId }),
  info: () => call<AppInfo>('get_app_info'),
}

/** 把缩略图绝对路径转换为 asset 协议 URL；浏览器预览模式返回 null。 */
export function thumbnailUrl(absolutePath: string | null): string | null {
  if (!absolutePath || !isTauriRuntime()) return null
  try {
    return convertFileSrc(absolutePath)
  } catch {
    return null
  }
}

export const backupApi = {
  overview: () => call<BackupOverview>('get_backup_overview'),
  create: () => call<BackupEntry>('create_database_backup'),
  verify: (path: string) => call<BackupVerification>('verify_database_backup', { path }),
  restore: (path: string) => call<RestoreOutcome>('restore_database_backup', { path }),
  setDirectory: (path: string) => call<string>('set_backup_directory', { path }),
}

export const cacheApi = {
  stats: () => call<CacheStats>('get_cache_stats'),
  setLimit: (bytes: number) => call<number>('set_cache_limit', { bytes }),
  clear: () => call<CacheCleanupResult>('clear_thumbnail_cache'),
}
