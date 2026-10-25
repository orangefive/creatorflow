<template>
  <div class="asset-page">
    <div class="asset-toolbar">
      <div class="filters">
        <el-input
          v-model="assets.query"
          :prefix-icon="Search"
          placeholder="搜索素材名称"
          clearable
          class="search"
          @input="onSearchInput"
          @clear="reloadFromStart"
        />
        <el-select
          v-model="assets.mediaKind"
          placeholder="全部类型"
          clearable
          class="filter"
          @change="reloadFromStart"
        >
          <el-option
            v-for="option in MEDIA_KIND_OPTIONS"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-select
          v-model="assets.status"
          placeholder="全部状态"
          clearable
          class="filter"
          @change="reloadFromStart"
        >
          <el-option label="正常" value="ready" />
          <el-option label="文件缺失" value="missing" />
          <el-option label="导入失败" value="failed" />
        </el-select>
        <el-select
          v-model="assets.tagId"
          placeholder="全部标签"
          clearable
          class="filter"
          @change="reloadFromStart"
        >
          <el-option v-for="tag in tags" :key="tag.id" :label="tag.name" :value="tag.id" />
        </el-select>
        <el-select v-model="assets.sortKey" class="filter" @change="reloadFromStart">
          <el-option label="按导入时间" value="created_at" />
          <el-option label="按名称" value="name" />
          <el-option label="按大小" value="size" />
          <el-option label="按时长" value="duration" />
        </el-select>
        <el-tooltip :content="assets.sortDirection === 'desc' ? '当前降序' : '当前升序'">
          <el-button :icon="ArrowDownUp" @click="toggleDirection" />
        </el-tooltip>
        <el-button v-if="assets.hasFilters" text @click="clearFilters">清除筛选</el-button>
      </div>
      <div class="actions">
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
      :title="`当前作品有 ${assets.missingCount} 个素材在磁盘上找不到，可用「重新定位」重新关联。`"
    />

    <el-alert
      v-if="sectionKey === 'video' && !assets.ffmpegAvailable"
      class="notice"
      type="info"
      :closable="false"
      show-icon
      title="未检测到 ffmpeg：仍可导入视频并读取 MP4/MOV 时长与分辨率，但不会生成视频封面。"
    />

    <div v-if="selected.length > 0" class="batch-bar cf-card">
      <span class="batch-count">已选 {{ selected.length }} 项</span>
      <el-select
        v-model="batchTagId"
        class="batch-tag"
        placeholder="选择标签"
        clearable
        filterable
      >
        <el-option v-for="tag in tags" :key="tag.id" :label="tag.name" :value="tag.id" />
      </el-select>
      <el-button :disabled="!batchTagId" @click="batchRemoveTag">移除标签</el-button>
      <el-button @click="batchAddTag">添加标签…</el-button>
      <span class="cf-spacer" />
      <el-button type="danger" plain @click="batchDelete">移入回收站</el-button>
      <el-button text @click="clearSelection">取消选择</el-button>
    </div>

    <div class="asset-list cf-card">
      <el-table
        ref="tableRef"
        v-loading="assets.loading"
        :data="assets.items"
        row-key="id"
        height="100%"
        :empty-text="emptyText"
        @row-dblclick="onRowDoubleClick"
        @selection-change="onSelectionChange"
      >
        <el-table-column type="selection" width="44" />
        <el-table-column label="" width="68">
          <template #default="{ row }">
            <div class="thumb">
              <img
                v-if="thumbnailSrc(row)"
                :src="thumbnailSrc(row) as string"
                :alt="row.displayName"
                loading="lazy"
                @error="markThumbnailBroken(row.id)"
              />
              <component v-else :is="iconFor(row.mediaKind)" :size="18" :stroke-width="1.6" />
            </div>
          </template>
        </el-table-column>

        <el-table-column label="名称" min-width="160">
          <template #default="{ row }">
            <div class="name-cell">
              <span class="name-text" :title="row.relativePath">{{ row.displayName }}</span>
            </div>
          </template>
        </el-table-column>

        <el-table-column label="类型" width="70">
          <template #default="{ row }">
            {{ MEDIA_KIND_LABEL[row.mediaKind as MediaKind] }}
          </template>
        </el-table-column>

        <el-table-column label="媒体信息" width="185">
          <template #default="{ row }">
            <el-tooltip
              :disabled="!row.metadataError"
              :content="row.metadataError ?? ''"
              placement="top"
            >
              <span class="meta-text">{{ metadataText(row) }}</span>
            </el-tooltip>
          </template>
        </el-table-column>

        <el-table-column label="标签" width="140">
          <template #default="{ row }">
            <el-select
              :model-value="row.tags.map((tag: Tag) => tag.name)"
              multiple
              filterable
              allow-create
              default-first-option
              size="small"
              placeholder="+ 标签"
              class="tag-select"
              @change="onTagsChange(row, $event)"
            >
              <el-option
                v-for="tag in tagOptions(row)"
                :key="tag.id"
                :label="tag.name"
                :value="tag.name"
              />
            </el-select>
          </template>
        </el-table-column>

        <el-table-column label="大小" width="88" align="right">
          <template #default="{ row }">{{ formatBytes(row.sizeBytes) }}</template>
        </el-table-column>

        <el-table-column label="导入时间" width="140">
          <template #default="{ row }">{{ formatDateTime(row.createdAt) }}</template>
        </el-table-column>

        <el-table-column label="状态" width="84">
          <template #default="{ row }">
            <el-tag
              size="small"
              :type="ASSET_STATUS_META[row.status as AssetStatus].type"
              effect="plain"
            >
              {{ ASSET_STATUS_META[row.status as AssetStatus].label }}
            </el-tag>
          </template>
        </el-table-column>

        <el-table-column label="操作" width="268" fixed="right">
          <template #default="{ row }">
            <el-button
              v-if="row.mediaKind === 'image' && row.status === 'ready'"
              size="small"
              text
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
            <el-button
              size="small"
              text
              :disabled="row.status !== 'ready'"
              @click="revealAsset(row)"
            >
              定位
            </el-button>
            <el-button
              v-if="row.status === 'missing'"
              size="small"
              text
              type="warning"
              @click="relocate(row)"
            >
              重新定位
            </el-button>
            <el-button
              v-if="row.metadataStatus === 'failed'"
              size="small"
              text
              @click="retryMetadata(row)"
            >
              重试信息
            </el-button>
            <el-button size="small" text type="danger" @click="removeAsset(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <div class="footer">
      <span class="cf-muted">
        共 {{ assets.total }} 个素材
        <template v-if="assets.hasFilters">（已筛选）</template>
      </span>
      <el-pagination
        :current-page="assets.page"
        :page-size="assets.pageSize"
        :page-sizes="[20, 50, 100, 200]"
        :total="assets.total"
        layout="sizes, prev, pager, next"
        background
        @current-change="onPageChange"
        @size-change="onPageSizeChange"
      />
    </div>

    <p class="drop-tip">
      也可以把文件直接拖入窗口导入到当前分区，源文件不会被移动。
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
        <el-button v-if="previewAsset" type="primary" plain @click="openAsset(previewAsset)">
          使用默认程序打开
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  ArrowDownUp,
  AudioLines,
  FileText,
  Image as ImageIcon,
  RefreshCw,
  Search,
  Upload,
  Video,
} from '@lucide/vue'
import ImportPrepareDialog from '@/components/ImportPrepareDialog.vue'
import {
  ASSET_STATUS_META,
  MEDIA_KIND_LABEL,
  MEDIA_KIND_OPTIONS,
  SECTION_META,
} from '@/constants/sections'
import { assetApi, tagApi } from '@/services/api'
import { isTauriRuntime, toIpcError } from '@/services/ipc'
import { onAssetMetadata } from '@/services/events'
import { useAssetsStore } from '@/stores/assets'
import { useImportsStore } from '@/stores/imports'
import type { TableInstance } from 'element-plus'
import type {
  Asset,
  AssetStatus,
  BatchOutcome,
  ImportCandidate,
  MediaKind,
  SectionKey,
  Tag,
} from '@/types'
import {
  formatBytes,
  formatDateTime,
  formatDimensions,
  formatDuration,
  formatFrameRate,
} from '@/utils/format'

const emit = defineEmits<{ changed: [] }>()

const tableRef = ref<TableInstance | null>(null)
const selected = ref<Asset[]>([])
const batchTagId = ref('')

const props = defineProps<{ workId: string; sectionKey: SectionKey }>()

const assets = useAssetsStore()
const imports = useImportsStore()

const tags = ref<Tag[]>([])
const prepareVisible = ref(false)
const candidates = ref<ImportCandidate[]>([])
const dragging = ref(false)
const previewVisible = ref(false)
const previewUrl = ref<string | null>(null)
const previewAsset = ref<Asset | null>(null)
const brokenThumbnails = ref<Set<string>>(new Set())

let unlistenDragDrop: UnlistenFn | null = null
let unlistenMetadata: UnlistenFn | null = null
let searchTimer: ReturnType<typeof setTimeout> | null = null
let metadataTimer: ReturnType<typeof setTimeout> | null = null

const sectionName = computed(() => SECTION_META[props.sectionKey].name)
const previewTitle = computed(() => previewAsset.value?.displayName ?? '图片预览')
const emptyText = computed(() => {
  if (assets.hasFilters) return '没有符合条件的素材'
  return `「${sectionName.value}」分区还没有素材`
})

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

function thumbnailSrc(asset: Asset): string | null {
  if (brokenThumbnails.value.has(asset.id)) return null
  return assets.thumbnailFor(asset)
}

function markThumbnailBroken(assetId: string): void {
  const next = new Set(brokenThumbnails.value)
  next.add(assetId)
  brokenThumbnails.value = next
}

function metadataText(asset: Asset): string {
  switch (asset.mediaKind) {
    case 'image':
      return formatDimensions(asset.width, asset.height)
    case 'audio': {
      const parts = [formatDuration(asset.durationMs)]
      if (asset.sampleRate) parts.push(`${(asset.sampleRate / 1000).toFixed(1)} kHz`)
      return parts.join(' · ')
    }
    case 'video': {
      const parts = [
        formatDuration(asset.durationMs),
        formatDimensions(asset.width, asset.height),
      ]
      const fps = formatFrameRate(asset.frameRate)
      if (fps) parts.push(fps)
      return parts.join(' · ')
    }
    default:
      return '—'
  }
}

function tagOptions(asset: Asset): Tag[] {
  const merged = new Map<string, Tag>()
  for (const tag of tags.value) merged.set(tag.name, tag)
  for (const tag of asset.tags) merged.set(tag.name, tag)
  return Array.from(merged.values())
}

async function load(): Promise<void> {
  await assets.fetchAssets(props.workId, props.sectionKey)
}

function onSelectionChange(rows: Asset[]): void {
  selected.value = rows
}

function clearSelection(): void {
  tableRef.value?.clearSelection()
  selected.value = []
  batchTagId.value = ''
}

function reportBatch(outcome: BatchOutcome, action: string): boolean {
  if (outcome.failed.length === 0) {
    ElMessage.success(`${action}成功：${outcome.succeeded} 项`)
    return true
  }
  const first = outcome.failed[0]
  const sample = first ? `（例：${first.name} — ${first.message}）` : ''
  ElMessage.warning(
    `${action}完成：成功 ${outcome.succeeded} 项，失败 ${outcome.failed.length} 项${sample}`,
  )
  return false
}

async function batchAddTag(): Promise<void> {
  if (selected.value.length === 0) return
  let name = ''
  try {
    const result = await ElMessageBox.prompt(
      `为所选 ${selected.value.length} 个素材添加标签，同名标签会自动复用。`,
      '批量添加标签',
      {
        confirmButtonText: '添加',
        cancelButtonText: '取消',
        inputPlaceholder: '标签名称',
        inputValidator: (value: string) => (value.trim() ? true : '标签名称不能为空'),
      },
    )
    name = result.value.trim()
  } catch {
    return
  }

  try {
    const outcome = await tagApi.batchAdd(
      name,
      selected.value.map((asset) => asset.id),
    )
    reportBatch(outcome, '添加标签')
    await Promise.all([load(), loadTags()])
    clearSelection()
    emit('changed')
  } catch (cause) {
    ElMessage.error(toIpcError(cause).message)
  }
}

async function batchRemoveTag(): Promise<void> {
  if (!batchTagId.value || selected.value.length === 0) return
  try {
    const outcome = await tagApi.batchRemove(
      batchTagId.value,
      selected.value.map((asset) => asset.id),
    )
    reportBatch(outcome, '移除标签')
    await Promise.all([load(), loadTags()])
    clearSelection()
    emit('changed')
  } catch (cause) {
    ElMessage.error(toIpcError(cause).message)
  }
}

async function batchDelete(): Promise<void> {
  if (selected.value.length === 0) return
  try {
    await ElMessageBox.confirm(
      `将所选的 ${selected.value.length} 个素材移入系统回收站？\n\n` +
        '文件可从回收站还原；应用内的素材记录会同时移除。',
      '批量移入回收站',
      {
        confirmButtonText: '移入回收站',
        cancelButtonText: '取消',
        type: 'warning',
        confirmButtonClass: 'el-button--danger',
      },
    )
  } catch {
    return
  }

  try {
    const outcome = await assetApi.batchDelete(selected.value.map((asset) => asset.id))
    reportBatch(outcome, '移入回收站')
    await load()
    clearSelection()
    emit('changed')
  } catch (cause) {
    ElMessage.error(toIpcError(cause).message)
  }
}

async function removeAsset(asset: Asset): Promise<void> {
  try {
    await ElMessageBox.confirm(
      `删除「${asset.displayName}」？\n\n文件会被移到系统回收站，列表中的记录同时移除，之后可从回收站还原。`,
      '删除素材',
      {
        confirmButtonText: '移到回收站',
        cancelButtonText: '取消',
        type: 'warning',
        confirmButtonClass: 'el-button--danger',
      },
    )
  } catch {
    return
  }

  const message = await assets.deleteAsset(asset.id)
  if (message) {
    ElMessage.error(message)
    return
  }
  ElMessage.success('素材已移到回收站')

  // 当前页删空且不是第一页时回退一页，避免停在空白页
  if (assets.items.length === 0 && assets.page > 1) {
    assets.setPage(assets.page - 1)
    await load()
  }
  emit('changed')
}

async function loadTags(): Promise<void> {
  tags.value = await assets.loadWorkTags(props.workId)
}

function reloadFromStart(): void {
  assets.applyFilters()
  void load()
}

function clearFilters(): void {
  assets.resetFilters()
  void load()
}

function onSearchInput(): void {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => {
    void reloadFromStart()
  }, 300)
}

function toggleDirection(): void {
  assets.sortDirection = assets.sortDirection === 'desc' ? 'asc' : 'desc'
  void reloadFromStart()
}

function onPageChange(page: number): void {
  assets.setPage(page)
  void load()
}

function onPageSizeChange(size: number): void {
  assets.pageSize = size
  assets.page = 1
  void load()
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
  await handlePaths(Array.isArray(selected) ? selected : [selected])
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

async function retryMetadata(asset: Asset): Promise<void> {
  const error = await assets.retryMetadata(asset.id)
  if (error) ElMessage.error(error)
  else ElMessage.success('已重新提取媒体信息')
}

function onRowDoubleClick(row: Asset): void {
  if (row.status === 'ready') void openAsset(row)
}

async function onTagsChange(asset: Asset, value: unknown): Promise<void> {
  const names = Array.isArray(value) ? value.map((item) => String(item).trim()) : []
  const error = await assets.updateAssetTags(asset, names.filter(Boolean))
  if (error) ElMessage.error(error)
  else await loadTags()
}

async function relocate(asset: Asset): Promise<void> {
  if (!isTauriRuntime()) {
    ElMessage.warning('重新定位需要在桌面端运行')
    return
  }
  const selected = await open({
    multiple: false,
    title: `为「${asset.displayName}」选择文件的新位置`,
    directory: false,
  })
  if (!selected || Array.isArray(selected)) return

  try {
    let result = await assets.relocate(asset.id, selected, false)
    if (result.requiresConfirmation) {
      await ElMessageBox.confirm(result.message, '文件内容不一致', {
        confirmButtonText: '仍然关联',
        cancelButtonText: '取消',
        type: 'warning',
      })
      result = await assets.relocate(asset.id, selected, true)
    }
    if (result.applied) {
      ElMessage.success(result.message)
      await load()
      await loadTags()
    }
  } catch (cause) {
    if (cause === 'cancel' || cause === 'close') return
    ElMessage.error(toIpcError(cause).message)
  }
}

watch(
  () => [props.workId, props.sectionKey],
  () => {
    assets.reset()
    void loadTags()
    void load()
  },
)

watch(
  () => imports.completedSignal,
  () => {
    void load()
    void loadTags()
  },
)

onMounted(async () => {
  if (!isTauriRuntime()) return
  assets.page = 1
  await Promise.all([load(), loadTags(), assets.loadCapabilities()])

  unlistenMetadata = await onAssetMetadata((event) => {
    if (event.workId !== props.workId) return
    if (metadataTimer) clearTimeout(metadataTimer)
    metadataTimer = setTimeout(() => {
      void load()
    }, 500)
  })

  unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === 'over' || event.payload.type === 'enter') {
      dragging.value = true
    } else if (event.payload.type === 'drop') {
      dragging.value = false
      void handlePaths(event.payload.paths.filter((path) => !path.endsWith('\\')))
    } else {
      dragging.value = false
    }
  })
})

onUnmounted(() => {
  // 浏览器预览模式下没有真实的事件后端，取消订阅可能抛错，这里不应影响页面卸载
  if (unlistenDragDrop) {
    try {
      unlistenDragDrop()
    } catch {
      // 忽略：预览环境无原生拖拽订阅
    }
    unlistenDragDrop = null
  }
  if (unlistenMetadata) {
    try {
      unlistenMetadata()
    } catch {
      // 忽略：预览环境无原生事件订阅
    }
    unlistenMetadata = null
  }
  if (searchTimer) clearTimeout(searchTimer)
  if (metadataTimer) clearTimeout(metadataTimer)
})
</script>

<style scoped>
.asset-page {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 4px 28px 16px;
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

.filters {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.actions {
  display: flex;
  gap: 8px;
}

.search {
  width: 220px;
}

.filter {
  width: 124px;
}

.notice {
  margin-bottom: 10px;
}

.batch-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  margin-bottom: 10px;
}

.batch-count {
  font-size: 13px;
  font-weight: 500;
  color: var(--cf-text);
}

.batch-tag {
  width: 160px;
}

.asset-list {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.thumb {
  width: 44px;
  height: 32px;
  border-radius: 4px;
  background: #f1f3f7;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #9aa3b2;
  overflow: hidden;
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.name-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta-text {
  color: var(--cf-text-muted);
  font-size: 12px;
}

.tag-select {
  width: 100%;
}

.tag-select :deep(.el-select__wrapper) {
  min-height: 26px;
  padding: 2px 8px;
}

.footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 10px;
}

.drop-tip {
  margin: 8px 2px 0;
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
