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

export type MetadataStatus = 'pending' | 'ready' | 'partial' | 'failed' | 'unsupported'

export type ThumbnailStatus = 'pending' | 'ready' | 'failed' | 'unsupported'

export type AssetSortKey = 'name' | 'created_at' | 'size' | 'duration'

export type SortDirection = 'asc' | 'desc'

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
  archivedFromStatus: WorkStatus | null
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

export interface Tag {
  id: string
  name: string
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
  width: number | null
  height: number | null
  durationMs: number | null
  frameRate: number | null
  sampleRate: number | null
  channels: number | null
  codec: string | null
  sha256: string | null
  metadataStatus: MetadataStatus
  metadataError: string | null
  thumbnailStatus: ThumbnailStatus
  thumbnailRelativePath: string | null
  /** 缩略图在磁盘上的绝对路径，配合 convertFileSrc 使用 */
  thumbnailPath: string | null
  thumbnailError: string | null
  tags: Tag[]
  /** 所属作品名称，跨作品检索时用于区分 */
  workName: string | null
  /** 所属分区 */
  sectionKey: SectionKey | null
}

export interface AssetQuery {
  workId?: string | null
  sectionKey?: SectionKey | null
  mediaKind?: MediaKind | null
  status?: AssetStatus | null
  tagId?: string | null
  keyword?: string
  sortKey?: AssetSortKey
  sortDirection?: SortDirection
  limit?: number
  offset?: number
}

export interface AssetPage {
  items: Asset[]
  total: number
  limit: number
  offset: number
}

export interface RelocationResult {
  applied: boolean
  requiresConfirmation: boolean
  sizeMatches: boolean
  hashMatches: boolean | null
  relativePath: string
  message: string
  asset: Asset | null
}

export interface MediaCapabilities {
  ffmpegAvailable: boolean
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

export interface BackupManifest {
  formatVersion: number
  appVersion: string
  schemaVersion: number
  createdAt: string
  databaseFile: string
  sizeBytes: number
  sha256: string
  integrityOk: boolean
  foreignKeysOk: boolean
}

export interface BackupEntry {
  path: string
  fileName: string
  sizeBytes: number
  createdAt: string | null
  valid: boolean
  problem: string | null
  manifest: BackupManifest | null
}

export interface BackupOverview {
  directory: string
  databasePath: string
  schemaVersion: number
  latestSchemaVersion: number
  entries: BackupEntry[]
}

export interface BackupVerification {
  valid: boolean
  integrityOk: boolean
  foreignKeysOk: boolean
  schemaVersion: number
  problem: string | null
}

export interface RestoreOutcome {
  restoredFrom: string
  backupSchemaVersion: number
  currentSchemaVersion: number
  safetySnapshot: string
  integrityOk: boolean
  foreignKeysOk: boolean
}

export interface CacheStats {
  directory: string
  totalBytes: number
  fileCount: number
  limitBytes: number
  minLimitBytes: number
  maxLimitBytes: number
}

export interface CacheCleanupResult {
  removedFiles: number
  freedBytes: number
  remainingBytes: number
  skippedFiles: number
  failures: number
}

export interface BatchFailure {
  id: string
  name: string
  message: string
}

export interface BatchOutcome {
  succeeded: number
  failed: BatchFailure[]
}

export type TaskStatus = 'todo' | 'in_progress' | 'done' | 'cancelled'
export type TaskPriority = 'low' | 'normal' | 'high' | 'urgent'

export interface Task {
  id: string
  workId: string
  title: string
  description: string | null
  status: TaskStatus
  priority: TaskPriority
  dueAt: string | null
  sortOrder: number
  createdAt: string
  updatedAt: string
  completedAt: string | null
  isOverdue: boolean
}

export interface TaskInput {
  title: string
  description?: string | null
  priority?: TaskPriority
  dueAt?: string | null
  status?: TaskStatus
}

export interface TaskQuery {
  status?: TaskStatus | null
  priority?: TaskPriority | null
  overdueOnly?: boolean
  sortKey?: 'manual' | 'created_at' | 'due_at' | 'priority'
  sortDirection?: 'asc' | 'desc'
}

export interface TaskSummary {
  total: number
  todo: number
  inProgress: number
  done: number
  cancelled: number
  overdue: number
  completionRate: number | null
}

export interface WorkDashboard {
  work: Work
  summary: TaskSummary
  upcoming: Task[]
  recentAssets: Asset[]
}

export interface TemplateTask {
  id: string
  title: string
  description: string | null
  priority: TaskPriority
  sortOrder: number
}

export interface WorkTemplate {
  id: string
  name: string
  description: string | null
  isBuiltin: boolean
  createdAt: string
  updatedAt: string
  tasks: TemplateTask[]
}

export interface TemplateTaskInput {
  title: string
  description?: string | null
  priority?: TaskPriority
}

export interface TemplateInput {
  name: string
  description?: string | null
  tasks: TemplateTaskInput[]
}
