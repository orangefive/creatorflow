import { invoke } from '@tauri-apps/api/core'
import type { AppErrorPayload } from '@/types'

/**
 * 前端唯一的后端调用入口。
 * 所有 invoke 都从这里出，便于统一错误归一化与后续替换传输层。
 */
export class IpcError extends Error {
  readonly code: string
  readonly detail: string | null

  constructor(payload: AppErrorPayload) {
    super(payload.message)
    this.name = 'IpcError'
    this.code = payload.code
    this.detail = payload.detail
  }
}

function isErrorPayload(value: unknown): value is AppErrorPayload {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    'message' in value &&
    typeof (value as { code: unknown }).code === 'string' &&
    typeof (value as { message: unknown }).message === 'string'
  )
}

export function toIpcError(error: unknown): IpcError {
  if (error instanceof IpcError) return error
  if (isErrorPayload(error)) {
    return new IpcError({
      code: error.code,
      message: error.message,
      detail: error.detail ?? null,
    })
  }
  const message =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : '发生未知错误'
  return new IpcError({ code: 'unknown', message, detail: null })
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    throw toIpcError(error)
  }
}

/** 判断当前是否运行在 Tauri 容器内（纯浏览器预览时为 false） */
export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}
