<template>
  <div class="asset-page">
    <div class="asset-toolbar">
      <div class="asset-toolbar__left">
        <el-input
          v-model="assets.query"
          :prefix-icon="Search"
          placeholder="搜索当前作品素材名称"
          clearable
          class="search"
        />
        <el-select v-model="assets.sortKey" class="sort" size="default">
          <el-option label="按导入时间" value="createdAt" />
          <el-option label="按名称" value="name" />
          <el-option label="按大小" value="size" />
        </el-select>
        <el-select v-model="assets.sortDirection" class="sort" size="default">
          <el-option label="降序" value="desc" />
          <el-option label="升序" value="asc" />
        </el-select>
      </div>
      <div class="asset-toolbar__right">
        <el-button :icon="RefreshCw" :loading="assets.loading" @click="load">刷新</el-button>
        <el-button type="primary" :icon="Upload" @click="pickFiles">导入文件</el-button>
      </div>
    </div>

    <el-alert
      v-if="assets.error"
      class="notice"
      type="error"
      :closable="false"
      show-icon
      :title="assets.error"
    />

    <el-alert
      v-if="assets.missingCount > 0"
      class="notice"
      type="warning"
      :closable="false"
      show-icon
      :title="`有 ${assets.missingCount} 个素材在磁盘上找不到，记录已保留，请检查文件是否被移动或删除。`"
    />

    <div class="asset-list cf-card">
      <el-table
        v-loading="assets.loading"
        :data="assets.filteredAssets"
        row-key="id"
        height="100%"
        :empty-text="emptyText"
        @row-dblclick="onRowDoubleClick"
      >
        <el-table-column label="名称" min-width="260">
          <template #default="{ row }">
            <div class="name-cell">
              <component :is="iconFor(row.mediaKind)" :size="16" :stroke-width="1.7" class="name-icon" />
              <span class="name-text" :title="row.relativePath">{{ row.displayName }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="类型" width="90">
          <template #default="{ row }">
            {{ MEDIA_KIND_LABEL[row.mediaKind as MediaKind] }}
          </template>
        </el-table-column>
        <el-table-column label="大小" width="110" align="right">
          <template #default="{ row }">{{ formatBytes(row.sizeBytes) }}</template>
        </el-table-column>
        <el-table-column label="导入时间" width="150">
          <template #default="{ row }">{{ formatDateTime(row.createdAt) }}</template>
        </el-table-column>
        <el-table-column label="状态" width="110">
          <template #default="{ row }">
            <el-tag
              size="small"
              :type="row.status === 'ready' ? 'success' : row.status === 'missing' ? 'danger' : 'warning'"
              effect="plain"
            >
              {{ statusLabel(row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="220" fixed="right">
          <template #default="{ row }">
            <el-button
              v-if="row.mediaKind === 'image'"
              size="small"
              text
              :disabled="row.status !== 'ready'"
              @click="preview(row)"
            >
              预览
            </el-button>
            <el-button
              size="small"
              text
              type="primary"
              :disabled="row.status !== 'ready'"
              @click="openAsset(row)"
            >
              打开
            </el-button>
            <el-button size="small" text :disabled="row.status !== 'ready'" @click="revealAsset(row)">
              定位
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <p class="drop-tip">
      也可以把文件直接拖入窗口导入到「{{ sectionName }}」分区，源文件不会被移动。
    </p>

    <div v-if="dragging" class="drop-overlay">
      <div class="drop-overlay__inner">
        <Upload :size="28" :stroke-width="1.6" />
        <p>松开鼠标，导入到「{{ sectionName }}」</p>
      </div>
    </div>

    <ImportPrepareDialog
      v-model="prepareVisible"
      :candidates="candidates"
      :fallback-section-key="sectionKey"
      @submit="startImport"
    />

    <el-dialog v-model="previewVisible" :title="previewTitle" width="720px" append-to-body>
      <div v-if="previewUrl" class="preview">
        <img :src="previewUrl" :alt="previewTitle" />
      </div>
      <el-skeleton v-else :rows="4" animated />
      <template #footer>
        <el-button @click="previewVisible = false">关闭</el-button>
        <el-button
          v-if="previewAsset"
          type="primary"
          plain
          @click="openAsset(previewAsset)"
        >
          使用默认程序打开
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { AudioLines, FileText, Image as ImageIcon, RefreshCw, Search, Upload, Video } from '@lucide/vue'
import ImportPrepareDialog from '@/components/ImportPrepareDialog.vue'
import { MEDIA_KIND_LABEL, SECTION_META } from '@/constants/sections'
import { isTauriRuntime } from '@/services/ipc'
import { useAssetsStore } from '@/stores/assets'
import { useImportsStore } from '@/stores/imports'
import type { Asset, ImportCandidate, MediaKind, SectionKey } from '@/types'
import { formatBytes, formatDateTime } from '@/utils/format'

const props = defineProps<{ workId: string; sectionKey: SectionKey }>()

const assets = useAssetsStore()
const imports = useImportsStore()

const prepareVisible = ref(false)
const candidates = ref<ImportCandidate[]>([])
const dragging = ref(false)
const previewVisible = ref(false)
const previewUrl = ref<string | null>(null)
const previewAsset = ref<Asset | null>(null)

let unlistenDragDrop: UnlistenFn | null = null

const sectionName = computed(() => SECTION_META[props.sectionKey].name)
const previewTitle = computed(() => previewAsset.value?.displayName ?? '图片预览')
const emptyText = computed(() =>
  assets.query.trim() ? '没有匹配的素材' : `「${sectionName.value}」分区还没有素材`,
)

function iconFor(kind: MediaKind) {
  switch (kind) {
    case 'audio':
      return AudioLines
    case 'video':
      return Video
    case 'image':
      return ImageIcon
    default:
      return FileText
  }
}

function statusLabel(status: Asset['status']): string {
  switch (status) {
    case 'ready':
      return '正常'
    case 'missing':
      return '文件缺失'
    case 'importing':
      return '导入中'
    case 'failed':
      return '导入失败'
    case 'trashed':
      return '已移除'
    default:
      return status
  }
}

async function load(): Promise<void> {
  await assets.fetchAssets(props.workId, props.sectionKey)
}

async function pickFiles(): Promise<void> {
  if (!isTauriRuntime()) {
    ElMessage.warning('文件导入需要在桌面端运行')
    return
  }
  const selected = await open({
    multiple: true,
    title: `选择要导入到「${sectionName.value}」的文件`,
    directory: false,
  })
  if (!selected) return
  const paths = Array.isArray(selected) ? selected : [selected]
  await handlePaths(paths)
}

async function handlePaths(paths: string[]): Promise<void> {
  if (paths.length === 0) return
  const result = await imports.prepare(props.workId, paths)
  if (result.skipped.length > 0) {
    const first = result.skipped[0]
    ElMessage.warning(
      `有 ${result.skipped.length} 个文件被跳过：${first?.reason ?? '无法读取'}`,
    )
  }
  if (result.candidates.length === 0) {
    ElMessage.error(imports.error ?? '没有可导入的文件')
    return
  }
  candidates.value = result.candidates
  prepareVisible.value = true
}

async function startImport(
  groups: Array<{ sectionKey: SectionKey; sourcePaths: string[] }>,
): Promise<void> {
  let started = 0
  for (const group of groups) {
    started += await imports.start(props.workId, group.sectionKey, group.sourcePaths)
  }
  if (started === 0) {
    ElMessage.error(imports.error ?? '导入任务创建失败')
    return
  }
  ElMessage.success(`已创建 ${started} 个导入任务`)
  if (groups.some((group) => group.sectionKey === props.sectionKey)) {
    await load()
  }
}

async function preview(asset: Asset): Promise<void> {
  previewAsset.value = asset
  previewUrl.value = null
  previewVisible.value = true
  const url = await assets.loadPreview(asset.id)
  if (url) previewUrl.value = url
  else ElMessage.error(assets.error ?? '无法读取图片预览')
}

async function openAsset(asset: Asset): Promise<void> {
  const error = await assets.openAsset(asset.id)
  if (error) ElMessage.error(error)
}

async function revealAsset(asset: Asset): Promise<void> {
  const error = await assets.revealAsset(asset.id)
  if (error) ElMessage.error(error)
}

function onRowDoubleClick(row: Asset): void {
  if (row.status === 'ready') void openAsset(row)
}

watch(
  () => [props.workId, props.sectionKey],
  () => {
    assets.reset()
    void load()
  },
)

watch(
  () => imports.completedSignal,
  () => {
    void load()
  },
)

onMounted(async () => {
  if (!isTauriRuntime()) return
  await load()
  if (isTauriRuntime()) {
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === 'over' || event.payload.type === 'enter') {
        dragging.value = true
      } else if (event.payload.type === 'drop') {
        dragging.value = false
        const paths = event.payload.paths.filter((path) => !path.endsWith('\\'))
        void handlePaths(paths)
      } else {
        dragging.value = false
      }
    })
  }
})

onUnmounted(() => {
  if (unlistenDragDrop) {
    unlistenDragDrop()
    unlistenDragDrop = null
  }
})
</script>

<style scoped>
.asset-page {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 4px 28px 20px;
  min-height: 0;
}

.asset-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}

.asset-toolbar__left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.asset-toolbar__right {
  display: flex;
  gap: 8px;
}

.search {
  width: 240px;
}

.sort {
  width: 120px;
}

.notice {
  margin-bottom: 12px;
}

.asset-list {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.name-icon {
  color: #6b7280;
  flex: 0 0 auto;
}

.name-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.drop-tip {
  margin: 10px 2px 0;
  font-size: 12px;
  color: #9ca3af;
}

.drop-overlay {
  position: absolute;
  inset: 8px 16px 16px;
  background: rgba(63, 106, 216, 0.06);
  border: 2px dashed #8fa8e0;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  z-index: 20;
}

.drop-overlay__inner {
  text-align: center;
  color: var(--cf-accent);
  font-weight: 500;
}

.preview {
  max-height: 62vh;
  overflow: auto;
  text-align: center;
}

.preview img {
  max-width: 100%;
  max-height: 60vh;
  object-fit: contain;
}
</style>
