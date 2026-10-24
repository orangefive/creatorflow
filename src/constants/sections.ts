import type { AssetStatus, MediaKind, SectionKey, WorkStatus } from '@/types'

export interface SectionMeta {
  key: SectionKey
  name: string
  folder: string
  description: string
  accept: string
}

/** 四个默认分区，顺序与磁盘目录排序一致 */
export const SECTION_LIST: SectionMeta[] = [
  {
    key: 'documents',
    name: '文档',
    folder: '01_文档',
    description: '脚本、字幕、参考资料',
    accept: '.md,.txt,.doc,.docx,.pdf,.srt,.ass,.rtf,.csv,.xlsx',
  },
  {
    key: 'audio',
    name: '音频',
    folder: '02_音频',
    description: '配乐、音效、人声',
    accept: '.mp3,.wav,.flac,.aac,.m4a,.ogg,.wma,.aif,.aiff',
  },
  {
    key: 'video',
    name: '视频',
    folder: '03_视频',
    description: '原始素材、剪辑工程、导出成片',
    accept: '.mp4,.mov,.mkv,.avi,.webm,.m4v,.wmv,.flv,.prproj,.drp,.veg,.aep',
  },
  {
    key: 'images',
    name: '图片',
    folder: '04_图片',
    description: '封面、截图、插画',
    accept: '.jpg,.jpeg,.png,.gif,.webp,.bmp,.svg,.tif,.tiff,.psd,.heic',
  },
]

export const SECTION_META: Record<SectionKey, SectionMeta> = SECTION_LIST.reduce(
  (acc, item) => ({ ...acc, [item.key]: item }),
  {} as Record<SectionKey, SectionMeta>,
)

export const SECTION_KEYS: SectionKey[] = SECTION_LIST.map((item) => item.key)

/** 与后端 `suggest_section_for_extension` 保持一致的扩展名映射（用于导入前预览） */
const EXTENSION_SECTION: Record<string, SectionKey> = {
  md: 'documents',
  txt: 'documents',
  doc: 'documents',
  docx: 'documents',
  pdf: 'documents',
  srt: 'documents',
  ass: 'documents',
  rtf: 'documents',
  csv: 'documents',
  xlsx: 'documents',
  mp3: 'audio',
  wav: 'audio',
  flac: 'audio',
  aac: 'audio',
  m4a: 'audio',
  ogg: 'audio',
  wma: 'audio',
  aif: 'audio',
  aiff: 'audio',
  mp4: 'video',
  mov: 'video',
  mkv: 'video',
  avi: 'video',
  webm: 'video',
  m4v: 'video',
  wmv: 'video',
  flv: 'video',
  prproj: 'video',
  drp: 'video',
  veg: 'video',
  aep: 'video',
  jpg: 'images',
  jpeg: 'images',
  png: 'images',
  gif: 'images',
  webp: 'images',
  bmp: 'images',
  svg: 'images',
  tif: 'images',
  tiff: 'images',
  psd: 'images',
  heic: 'images',
}

export function suggestSectionKey(fileName: string): SectionKey | null {
  const dotIndex = fileName.lastIndexOf('.')
  if (dotIndex <= 0 || dotIndex === fileName.length - 1) return null
  const extension = fileName.slice(dotIndex + 1).toLowerCase()
  return EXTENSION_SECTION[extension] ?? null
}

export function mediaKindOfSection(sectionKey: SectionKey): MediaKind {
  switch (sectionKey) {
    case 'documents':
      return 'document'
    case 'audio':
      return 'audio'
    case 'video':
      return 'video'
    case 'images':
      return 'image'
    default:
      return 'other'
  }
}

export const WORK_STATUS_META: Record<WorkStatus, { label: string; type: WorkStatusTagType }> = {
  idea: { label: '构思中', type: 'info' },
  scripting: { label: '脚本编写', type: 'info' },
  collecting: { label: '素材收集', type: 'warning' },
  in_production: { label: '制作中', type: 'primary' },
  review: { label: '待审核', type: 'warning' },
  published: { label: '已发布', type: 'success' },
  archived: { label: '已归档', type: 'info' },
  creating: { label: '创建中', type: 'warning' },
  error: { label: '创建失败', type: 'danger' },
}

export type WorkStatusTagType = 'info' | 'warning' | 'primary' | 'success' | 'danger'

/** 用户可手动选择的作品状态（不含内部状态 creating / error） */
export const SELECTABLE_WORK_STATUSES: WorkStatus[] = [
  'idea',
  'scripting',
  'collecting',
  'in_production',
  'review',
  'published',
]

export const MEDIA_KIND_LABEL: Record<MediaKind, string> = {
  document: '文档',
  audio: '音频',
  video: '视频',
  image: '图片',
  other: '其他',
}

export const MEDIA_KIND_OPTIONS: Array<{ value: MediaKind; label: string }> = (
  Object.keys(MEDIA_KIND_LABEL) as MediaKind[]
).map((value) => ({ value, label: MEDIA_KIND_LABEL[value] }))

export const ASSET_STATUS_META: Record<
  AssetStatus,
  { label: string; type: 'success' | 'info' | 'warning' | 'danger' }
> = {
  ready: { label: '正常', type: 'success' },
  missing: { label: '文件缺失', type: 'danger' },
  importing: { label: '导入中', type: 'warning' },
  failed: { label: '导入失败', type: 'danger' },
  trashed: { label: '已移除', type: 'info' },
}
