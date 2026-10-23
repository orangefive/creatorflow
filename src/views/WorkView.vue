<template>
  <div class="work-page">
    <div class="work-header">
      <div class="work-header__main">
        <el-button text :icon="ChevronLeft" class="back" @click="goHome">返回</el-button>
        <div class="title-block">
          <div class="title-row">
            <h1 class="work-title">{{ work?.name ?? '作品' }}</h1>
            <WorkStatusTag v-if="work" :status="work.status" />
          </div>
          <p class="work-meta">
            <span v-if="work?.description">{{ work.description }}</span>
            <span v-else class="cf-muted">暂无描述</span>
            <span class="dot">·</span>
            <button type="button" class="path" :title="work?.rootPath" @click="copyPath">
              {{ work?.rootPath }}
            </button>
          </p>
        </div>
      </div>

      <div class="work-header__actions">
        <el-select
          :model-value="work?.status"
          class="status-select"
          :disabled="work?.status === 'archived'"
          @update:model-value="onStatusChange"
        >
          <el-option
            v-for="status in SELECTABLE_WORK_STATUSES"
            :key="status"
            :label="WORK_STATUS_META[status].label"
            :value="status"
          />
        </el-select>
        <el-button :loading="works.loading" @click="reload">刷新</el-button>
        <el-button
          :disabled="work?.status === 'archived'"
          @click="archive"
        >
          归档
        </el-button>
      </div>
    </div>

    <el-alert
      v-if="works.error"
      class="notice"
      type="error"
      :closable="false"
      show-icon
      :title="works.error"
    />

    <el-tabs :model-value="activeSection" class="section-tabs" @update:model-value="switchSection">
      <el-tab-pane
        v-for="section in sections"
        :key="section.key"
        :name="section.key"
      >
        <template #label>
          <span class="tab-label">
            <component :is="sectionIcon(section.key)" :size="15" :stroke-width="1.7" />
            {{ section.name }}
            <span class="tab-count">{{ section.assetCount }}</span>
          </span>
        </template>
      </el-tab-pane>
    </el-tabs>

    <div class="work-content">
      <el-skeleton v-if="loadingWork" :rows="6" animated />
      <AssetListView
        v-else-if="work && activeSection"
        :key="`${work.id}-${activeSection}`"
        :work-id="work.id"
        :section-key="activeSection"
      />
      <div v-else class="cf-empty">作品不存在或已被移除</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { ChevronLeft, FileText, AudioLines, Image as ImageIcon, Video } from '@lucide/vue'
import AssetListView from '@/views/AssetListView.vue'
import WorkStatusTag from '@/components/WorkStatusTag.vue'
import {
  SECTION_KEYS,
  SELECTABLE_WORK_STATUSES,
  WORK_STATUS_META,
} from '@/constants/sections'
import { useSettingsStore } from '@/stores/settings'
import { useWorksStore } from '@/stores/works'
import { isTauriRuntime } from '@/services/ipc'
import type { SectionKey, WorkStatus } from '@/types'

const route = useRoute()
const router = useRouter()
const works = useWorksStore()
const settings = useSettingsStore()
const loadingWork = ref(true)

const workId = computed(() => String(route.params.workId ?? ''))
const work = computed(() => works.currentWork)
const sections = computed(() => works.sections)

const activeSection = computed<SectionKey>(() => {
  const raw = route.params.sectionKey
  const value = Array.isArray(raw) ? raw[0] : raw
  if (value && (SECTION_KEYS as string[]).includes(value)) {
    return value as SectionKey
  }
  return 'documents'
})

function sectionIcon(key: SectionKey) {
  switch (key) {
    case 'audio':
      return AudioLines
    case 'video':
      return Video
    case 'images':
      return ImageIcon
    default:
      return FileText
  }
}

async function load(): Promise<void> {
  loadingWork.value = true
  const loaded = await works.fetchWork(workId.value)
  loadingWork.value = false
  if (loaded) {
    await settings.rememberLastOpened(loaded.id)
  }
}

async function reload(): Promise<void> {
  await load()
}

function switchSection(name: string | number): void {
  const key = String(name) as SectionKey
  if (!(SECTION_KEYS as string[]).includes(key)) return
  router.replace(`/works/${workId.value}/${key}`)
}

function goHome(): void {
  router.push('/')
}

async function copyPath(): Promise<void> {
  if (!work.value) return
  try {
    await navigator.clipboard.writeText(work.value.rootPath)
    ElMessage.success('目录位置已复制')
  } catch {
    ElMessage.warning('无法写入剪贴板，请手动复制')
  }
}

async function onStatusChange(value: unknown): Promise<void> {
  if (!work.value || typeof value !== 'string') return
  const status = value as WorkStatus
  const ok = await works.updateStatus(work.value.id, status)
  if (ok) ElMessage.success(`状态已更新为「${WORK_STATUS_META[status].label}」`)
  else ElMessage.error(works.error ?? '状态更新失败')
}

async function archive(): Promise<void> {
  if (!work.value) return
  try {
    await ElMessageBox.confirm(
      `归档「${work.value.name}」？归档只改变作品状态，不会移动或删除磁盘文件。`,
      '归档作品',
      { confirmButtonText: '归档', cancelButtonText: '取消', type: 'warning' },
    )
  } catch {
    return
  }
  const ok = await works.archiveWork(work.value.id)
  if (ok) ElMessage.success('作品已归档')
  else ElMessage.error(works.error ?? '归档失败')
}

onMounted(async () => {
  if (!isTauriRuntime()) {
    loadingWork.value = false
    return
  }
  if (!settings.info) await settings.load()
  await load()
})
</script>

<style scoped>
.work-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.work-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 16px 28px 10px;
}

.work-header__main {
  display: flex;
  gap: 6px;
  min-width: 0;
}

.back {
  margin-top: 2px;
  padding: 4px;
}

.title-block {
  min-width: 0;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.work-title {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.work-meta {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--cf-text-muted);
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.dot {
  color: #d1d5db;
}

.path {
  border: none;
  background: none;
  padding: 0;
  font: inherit;
  color: #6b7280;
  cursor: pointer;
  max-width: 460px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.path:hover {
  color: var(--cf-accent);
}

.work-header__actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.status-select {
  width: 128px;
}

.notice {
  margin: 0 28px 8px;
}

.section-tabs {
  padding: 0 28px;
}

/* 分区内容由下方 AssetListView 渲染，隐藏 el-tabs 自带的空内容区 */
.section-tabs :deep(.el-tabs__content) {
  display: none;
}

.tab-label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.tab-count {
  font-size: 11px;
  color: #9ca3af;
}

.work-content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  padding-top: 10px;
}

.work-content :deep(.el-skeleton) {
  padding: 0 28px;
}
</style>
