<template>
  <div class="shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand__mark">CF</div>
        <div>
          <div class="brand__name">CreatorFlow</div>
          <div class="brand__tagline">作品工作空间</div>
        </div>
      </div>

      <nav class="nav">
        <router-link
          v-for="item in primaryNav"
          :key="item.key"
          :to="item.to"
          class="nav__item"
          :class="{ 'nav__item--active': activeNavKey === item.key }"
        >
          <component :is="item.icon" :size="17" :stroke-width="1.8" />
          <span>{{ item.label }}</span>
        </router-link>

        <button class="nav__item nav__item--button" type="button" @click="openQueue">
          <component :is="Inbox" :size="17" :stroke-width="1.8" />
          <span>导入任务</span>
          <el-badge
            v-if="imports.activeJobs.length > 0"
            :value="imports.activeJobs.length"
            class="nav__badge"
            type="primary"
          />
          <span v-else-if="imports.failedJobs.length > 0" class="nav__dot" />
        </button>
      </nav>

      <div class="sidebar__footer">
        <button class="nav__item nav__item--button" type="button" @click="router.push('/settings')">
          <component :is="Settings" :size="17" :stroke-width="1.8" />
          <span>设置</span>
        </button>
        <div class="version">v{{ version }}</div>
      </div>
    </aside>

    <main class="main">
      <div v-if="!tauriReady" class="browser-warning">
        当前运行在浏览器预览模式，作品与文件功能需要启动桌面端（<code>npm run tauri:dev</code>）。
      </div>
      <slot />
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, type Component } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { FolderOpen, House, Inbox, Search, Settings } from '@lucide/vue'
import { isTauriRuntime } from '@/services/ipc'
import { useImportsStore } from '@/stores/imports'
import { useSettingsStore } from '@/stores/settings'

const route = useRoute()
const router = useRouter()
const imports = useImportsStore()
const settings = useSettingsStore()

const tauriReady = ref(isTauriRuntime())
const version = computed(() => settings.info?.version ?? '0.1.0')

interface NavItem {
  key: 'home' | 'all' | 'search'
  to: string
  label: string
  icon: Component
}

const primaryNav: NavItem[] = [
  { key: 'home', to: '/', label: '首页', icon: House },
  { key: 'all', to: '/?view=all', label: '所有作品', icon: FolderOpen },
  { key: 'search', to: '/search', label: '全局搜索', icon: Search },
]

const activeNavKey = computed<'home' | 'all' | 'search'>(() => {
  if (route.path.startsWith('/search')) return 'search'
  if (route.path.startsWith('/works')) return 'all'
  return route.query.view === 'all' ? 'all' : 'home'
})

function openQueue(): void {
  imports.panelVisible = true
}

onMounted(async () => {
  if (tauriReady.value) {
    await imports.attachListener()
    await settings.load()
  }
})
</script>

<style scoped>
.shell {
  display: flex;
  height: 100%;
  background: var(--cf-bg);
}

.sidebar {
  width: 208px;
  flex: 0 0 208px;
  display: flex;
  flex-direction: column;
  padding: 18px 12px 14px;
  background: var(--cf-sidebar-bg);
  border-right: 1px solid var(--cf-border);
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 8px 18px;
}

.brand__mark {
  width: 32px;
  height: 32px;
  border-radius: 9px;
  background: linear-gradient(135deg, #4b76e0, #3558b8);
  color: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  font-size: 13px;
  letter-spacing: 0.5px;
}

.brand__name {
  font-weight: 600;
  font-size: 14px;
}

.brand__tagline {
  font-size: 11px;
  color: var(--cf-text-muted);
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav__item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 9px 10px;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: #374151;
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s ease;
}

.nav__item:hover {
  background: #f1f3f7;
}

.nav__item--active {
  background: #eef2fb;
  color: var(--cf-accent);
  font-weight: 600;
}

.nav__item--button {
  font-family: inherit;
}

.nav__badge {
  margin-left: auto;
}

.nav__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #e6a23c;
  margin-left: auto;
}

.sidebar__footer {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.version {
  padding: 4px 10px;
  font-size: 11px;
  color: #9ca3af;
}

.main {
  flex: 1;
  min-width: 0;
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.browser-warning {
  padding: 8px 16px;
  background: #fff7e6;
  border-bottom: 1px solid #f5dab1;
  color: #8a5a00;
  font-size: 12px;
}
</style>
