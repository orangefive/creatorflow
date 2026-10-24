import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { ImportProgressEvent, MetadataStatus, ThumbnailStatus } from '@/types'

export const IMPORT_PROGRESS_EVENT = 'import://progress'
export const ASSET_METADATA_EVENT = 'asset://metadata'

/** 订阅导入进度事件，返回取消订阅函数 */
export async function onImportProgress(
  handler: (payload: ImportProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<ImportProgressEvent>(IMPORT_PROGRESS_EVENT, (event) => {
    handler(event.payload)
  })
}

export interface AssetMetadataEvent {
  assetId: string
  workId: string
  metadataStatus: MetadataStatus
  thumbnailStatus: ThumbnailStatus
}

/** 订阅素材元数据/缩略图完成事件 */
export async function onAssetMetadata(
  handler: (payload: AssetMetadataEvent) => void,
): Promise<UnlistenFn> {
  return listen<AssetMetadataEvent>(ASSET_METADATA_EVENT, (event) => {
    handler(event.payload)
  })
}
