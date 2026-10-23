/**
 * 前后端共享的数据契约。
 * 字段名与 Rust `serde(rename_all = "camelCase")` 的输出保持一致，
 * 修改此处时必须同步修改 `src-tauri/src/domain/`。
 */

export type WorkStatus =
  | 'idea'
  | 'scripting'
  | 'collecting'
  | 'in_production'
  | 'review'
  | 'published'
  | 'archived'
  | 'creating'
  | 'error'

export type SectionKey = 'documents' | 'audio' | 'video' | 'images'

export type MediaKind = 'document' | 'audio' | 'video' | 'image' | 'other'

export type AssetStatus = 'importing' | 'ready' | 'missing' | 'failed' | 'trashed'

export type ImportStatus =
  | 'queued'
  | 'copying'
  | 'verifying'
  | 'completed'
  | 'failed'
  | 'cancelled'

export interface Work {
  id: string
  name: string
  description: string | null
  status: WorkStatus
  rootPath: string
  templateKey: string | null
  createdAt: string
  updatedAt: string
  archivedAt: string | null
  assetCount: number
}

export interface Section {
  id: string
  workId: string
  key: SectionKey
  name: string
  relativePath: string
  sortOrder: number
  createdAt: string
  assetCount: number
}

export interface Asset {
  id: string
  workId: string
  sectionId: string
  displayName: string
  originalName: string
  relativePath: string
  sourcePath: string | null
  extension: string | null
  mediaKind: MediaKind
  sizeBytes: number
  status: AssetStatus
  isFavorite: boolean
  createdAt: string
  updatedAt: string
  /** 绝对路径仅用于展示与“在资源管理器中显示”，由后端解析 */
  absolutePath: string
}

export interface ImportJob {
  id: string
  workId: string
  sectionId: string
  sourcePath: string
  fileName: string
  targetRelativePath: string | null
  temporaryRelativePath: string | null
  sourceSizeBytes: number | null
  bytesCopied: number
  status: ImportStatus
  errorCode: string | null
  errorMessage: string | null
  assetId: string | null
  createdAt: string
  updatedAt: string
  completedAt: string | null
}

export interface AppSettings {
  workspaceRoot: string | null
  lastOpenedWorkId: string | null
}

export interface AppInfo {
  version: string
  tauriVersion: string
  databasePath: string
  logDirectory: string
}

/** 后端统一错误序列化结构 */
export interface AppErrorPayload {
  code: string
  message: string
  detail: string | null
}

/** 导入进度事件负载 */
export interface ImportProgressEvent {
  jobId: string
  workId: string
  status: ImportStatus
  bytesCopied: number
  totalBytes: number | null
  errorCode: string | null
  errorMessage: string | null
  targetRelativePath: string | null
}

/** 导入前由一个来源文件生成的目标建议 */
export interface ImportCandidate {
  sourcePath: string
  fileName: string
  sizeBytes: number
  suggestedSectionKey: SectionKey | null
}

export interface SkippedSource {
  sourcePath: string
  reason: string
}

export interface PrepareImportResult {
  candidates: ImportCandidate[]
  skipped: SkippedSource[]
}

export interface CreateWorkInput {
  name: string
  description?: string | null
  rootDirectory?: string | null
  templateKey?: string | null
}

export interface ImportRequest {
  workId: string
  sectionKey: SectionKey
  sourcePaths: string[]
}

export interface StartupRecoveryReport {
  examinedJobs: number
  completedJobs: number
  failedJobs: number
  cleanedTemporaryFiles: number
  messages: string[]
}
