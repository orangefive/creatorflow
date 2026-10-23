/** 通用格式化工具，纯函数便于单元测试 */

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || Number.isNaN(bytes)) return '—'
  if (bytes < 0) return '—'
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const value = bytes / Math.pow(1024, exponent)
  const digits = exponent === 0 ? 0 : value >= 100 ? 0 : value >= 10 ? 1 : 2
  return `${value.toFixed(digits)} ${units[exponent]}`
}

export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return '—'
  const pad = (value: number): string => value.toString().padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`
}

export function formatProgress(copied: number, total: number | null): string {
  if (total === null || total <= 0) return formatBytes(copied)
  return `${formatBytes(copied)} / ${formatBytes(total)}`
}

export function progressPercent(copied: number, total: number | null): number {
  if (total === null || total <= 0) return 0
  return Math.min(100, Math.round((copied / total) * 100))
}

/** 从 Windows 或 POSIX 路径中取出最后一段 */
export function baseName(path: string): string {
  const normalized = path.replace(/[\\/]+$/, '')
  const parts = normalized.split(/[\\/]/)
  const last = parts[parts.length - 1]
  return last ?? path
}

export function extensionOf(fileName: string): string | null {
  const dotIndex = fileName.lastIndexOf('.')
  if (dotIndex <= 0 || dotIndex === fileName.length - 1) return null
  return fileName.slice(dotIndex + 1).toLowerCase()
}
