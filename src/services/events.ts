import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { ImportProgressEvent } from '@/types'

export const IMPORT_PROGRESS_EVENT = 'import://progress'

/** 订阅导入进度事件，返回取消订阅函数 */
export async function onImportProgress(
  handler: (payload: ImportProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<ImportProgressEvent>(IMPORT_PROGRESS_EVENT, (event) => {
    handler(event.payload)
  })
}
