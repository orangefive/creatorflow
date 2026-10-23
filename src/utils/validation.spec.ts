import { describe, expect, it } from 'vitest'
import { validateWorkName } from '@/utils/validation'

describe('validateWorkName', () => {
  it('accepts normal Chinese and latin names', () => {
    expect(validateWorkName('我的第一支视频')).toBeNull()
    expect(validateWorkName('Podcast Ep.01')).toBeNull()
    expect(validateWorkName('直播切片-2026')).toBeNull()
  })

  it('rejects empty and blank names', () => {
    expect(validateWorkName('')).toBe('作品名称不能为空')
    expect(validateWorkName('   ')).toBe('作品名称不能为空')
  })

  it('rejects surrounding whitespace', () => {
    expect(validateWorkName(' 视频 ')).toBe('作品名称首尾不能包含空格')
  })

  it('rejects Windows invalid characters', () => {
    for (const name of ['a/b', 'a\\b', 'a:b', 'a*b', 'a?b', 'a"b', 'a<b', 'a>b', 'a|b']) {
      expect(validateWorkName(name)).not.toBeNull()
    }
  })

  it('rejects reserved device names', () => {
    expect(validateWorkName('CON')).not.toBeNull()
    expect(validateWorkName('nul.txt')).not.toBeNull()
    expect(validateWorkName('Lpt1')).not.toBeNull()
  })

  it('rejects trailing dots and over-long names', () => {
    expect(validateWorkName('视频.')).not.toBeNull()
    expect(validateWorkName('长'.repeat(81))).not.toBeNull()
    expect(validateWorkName('长'.repeat(80))).toBeNull()
  })
})
