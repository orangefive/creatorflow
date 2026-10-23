const INVALID_CHARS = /[<>:"/\\|?*]/
const RESERVED_NAMES = new Set([
  'con',
  'prn',
  'aux',
  'nul',
  'com1',
  'com2',
  'com3',
  'com4',
  'com5',
  'com6',
  'com7',
  'com8',
  'com9',
  'lpt1',
  'lpt2',
  'lpt3',
  'lpt4',
  'lpt5',
  'lpt6',
  'lpt7',
  'lpt8',
  'lpt9',
])

export const WORK_NAME_MAX_LENGTH = 80

function containsControlChar(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    if (value.charCodeAt(index) < 32) return true
  }
  return false
}

/**
 * 作品名称校验（与 Rust `domain::work::validate_work_name` 规则保持一致）。
 * 返回中文错误文案，合法时返回 null。
 */
export function validateWorkName(rawName: string): string | null {
  const name = rawName.trim()
  if (name.length === 0) return '作品名称不能为空'
  if (name !== rawName) return '作品名称首尾不能包含空格'
  if (name.length > WORK_NAME_MAX_LENGTH) {
    return `作品名称不能超过 ${WORK_NAME_MAX_LENGTH} 个字符`
  }
  if (INVALID_CHARS.test(name)) {
    return '作品名称不能包含 \\ / : * ? " < > | 等字符'
  }
  if (containsControlChar(name)) {
    return '作品名称不能包含控制字符'
  }
  if (name === '.' || name === '..') return '作品名称不能是 . 或 ..'
  if (name.endsWith('.')) return '作品名称不能以句点结尾'
  const stem = name.split('.')[0]?.trim().toLowerCase() ?? ''
  if (RESERVED_NAMES.has(stem)) {
    return `"${stem.toUpperCase()}" 是 Windows 保留名称，请换一个名称`
  }
  return null
}
