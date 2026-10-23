import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'

// Tauri 期望固定端口；端口被占用时直接失败，避免前端与桌面壳加载地址不一致。
const devPort = 1420

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: devPort,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    target: 'chrome110',
    sourcemap: false,
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.spec.ts', 'tests/**/*.spec.ts'],
    restoreMocks: true,
  },
})
