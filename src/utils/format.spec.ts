import { describe, expect, it } from 'vitest'
import {
  baseName,
  extensionOf,
  formatBytes,
  formatDateTime,
  formatDimensions,
  formatDuration,
  formatFrameRate,
  formatProgress,
  progressPercent,
} from '@/utils/format'

describe('formatBytes', () => {
  it('formats zero and small values', () => {
    expect(formatBytes(0)).toBe('0 B')
    expect(formatBytes(512)).toBe('512 B')
  })

  it('formats larger units with sensible precision', () => {
    expect(formatBytes(1024)).toBe('1.00 KB')
    expect(formatBytes(1536)).toBe('1.50 KB')
    expect(formatBytes(10 * 1024 * 1024)).toBe('10.0 MB')
    expect(formatBytes(200 * 1024 * 1024)).toBe('200 MB')
  })

  it('handles missing values', () => {
    expect(formatBytes(null)).toBe('—')
    expect(formatBytes(undefined)).toBe('—')
    expect(formatBytes(Number.NaN)).toBe('—')
  })
})

describe('progress helpers', () => {
  it('computes percentage with clamping', () => {
    expect(progressPercent(50, 100)).toBe(50)
    expect(progressPercent(200, 100)).toBe(100)
    expect(progressPercent(10, null)).toBe(0)
    expect(progressPercent(10, 0)).toBe(0)
  })

  it('formats progress text', () => {
    expect(formatProgress(1024, 2048)).toBe('1.00 KB / 2.00 KB')
    expect(formatProgress(1024, null)).toBe('1.00 KB')
  })
})

describe('path helpers', () => {
  it('extracts base name from Windows and POSIX paths', () => {
    expect(baseName('C:\\Works\\我的视频\\脚本.md')).toBe('脚本.md')
    expect(baseName('/home/user/clip.mp4')).toBe('clip.mp4')
    expect(baseName('C:\\Works\\trailing\\')).toBe('trailing')
  })

  it('extracts lowercase extension', () => {
    expect(extensionOf('封面.PNG')).toBe('png')
    expect(extensionOf('README')).toBeNull()
    expect(extensionOf('.gitignore')).toBeNull()
  })
})

describe('formatDateTime', () => {
  it('returns placeholder for empty values', () => {
    expect(formatDateTime(null)).toBe('—')
    expect(formatDateTime('not-a-date')).toBe('—')
  })

  it('formats a valid ISO timestamp', () => {
    const formatted = formatDateTime('2026-01-02T03:04:05.000Z')
    expect(formatted).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/)
  })
})

describe('media formatting', () => {
  it('formats duration in minutes and hours', () => {
    expect(formatDuration(0)).toBe('0:00')
    expect(formatDuration(65_000)).toBe('1:05')
    expect(formatDuration(3_725_000)).toBe('1:02:05')
    expect(formatDuration(null)).toBe('—')
    expect(formatDuration(-1)).toBe('—')
  })

  it('formats dimensions and frame rate', () => {
    expect(formatDimensions(1920, 1080)).toBe('1920 × 1080')
    expect(formatDimensions(null, 1080)).toBe('—')
    expect(formatFrameRate(29.97)).toBe('29.97 fps')
    expect(formatFrameRate(25)).toBe('25 fps')
    expect(formatFrameRate(null)).toBeNull()
  })
})
