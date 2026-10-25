<template>
  <div class="cf-page">
    <div class="cf-page-header">
      <div>
        <h1 class="cf-page-title">全局搜索</h1>
        <p class="cf-page-subtitle">跨全部作品检索素材，按作品、分区、类型、状态与标签筛选</p>
      </div>
    </div>

    <div class="cf-card search-panel">
      <div class="search-row">
        <el-input
          v-model="keywordInput"
          class="search-input"
          placeholder="搜索素材名称、原始文件名或作品名称"
          clearable
          :prefix-icon="Search"
        />
        <el-select
          :model-value="search.query.workId ?? ''"
          class="filter"
          placeholder="全部作品"
          clearable
          filterable
          @update:model-value="onWorkChange"
        >
          <el-option
            v-for="work in works.works"
            :key="work.id"
            :label="work.name"
            :value="work.id"
          />
        </el-select>
        <el-select
          :model-value="search.query.sectionKey ?? ''"
          class="filter filter--sm"
          placeholder="全部分区"
          clearable
          @update:model-value="onSectionChange"
        >
          <el-option
            v-for="key in SECTION_KEYS"
            :key="key"
            :label="SECTION_META[key].name"
            :value="key"
          />
        </el-select>
      </div>

      <div class="search-row">
        <el-select
          :model-value="search.query.mediaKind ?? ''"
          class="filter filter--sm"
          placeholder="全部类型"
          clearable
          @update:model-value="onKindChange"
        >
          <el-option
            v-for="option in MEDIA_KIND_OPTIONS"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-select
          :model-value="search.query.status ?? ''"
          class="filter filter--sm"
          placeholder="全部状态"
          clearable
          @update:model-value="onStatusChange"
        >
          <el-option
            v-for="(meta, value) in ASSET_STATUS_META"
            :key="value"
            :label="meta.label"
            :value="value"
          />
        </el-select>
        <el-select
          :model-value="search.query.tagId ?? ''"
          class="filter filter--sm"
          placeholder="全部标签"
          clearable
          filterable
          @update:model-value="onTagChange"
        >
          <el-option
            v-for="tag in tags"
            :key="tag.id"
            :label="`${tag.name}（${tag.assetCount}）`"
            :value="tag.id"
          />
        </el-select>
        <el-select
          :model-value="sortValue"
          class="filter filter--sm"
          @update:model-value="onSortChange"
        >
          <el-option label="按导入时间" value="created_at:desc" />
          <el-option label="按名称" value="name:asc" />
          <el-option label="按大小" value="size:desc" />
          <el-option label="按时长" value="duration:desc" />
        </el-select>
        <span class="cf-spacer" />
        <el-button :disabled="!search.hasFilters" @click="clearFilters">清除筛选</el-button>
        <el-button :loading="search.loading" @click="search.fetchResults()">刷新</el-button>
      </div>
    </div>

    <el-alert
      v-if="search.error"
      class="notice"
      type="error"
      :closable="false"
      show-icon
      :title="search.error"
    />

    <div class="cf-card table-card">
      <div class="summary">
        <span class="cf-muted">
          共 {{ search.total }} 条结果<span v-if="search.missingCount > 0">
            ，当前页 {{ search.missingCount }} 条文件缺失</span
          >
        </span>
      </div>

      <el-table
        v-loading="search.loading"
        :data="search.items"
        row-key="id"
        size="default"
        :empty-text="search.loaded ? '没有匹配的素材' : '输入关键词或选择筛选条件开始搜索'"
      >
        <el-table-column label="" width="66">
          <template #default="{ row }">
            <div class="thumb">
              <img v-if="row.thumbnailPath" :src="thumbnailUrl(row.thumbnailPath) ?? undefined" alt="" />
              <component v-else :is="iconFor(row)" :size="18" :stroke-width="1.6" />
            </div>
          </template>
        </el-table-column>
        <el-table-column label="名称" min-width="200">
          <template #default="{ row }">
            <div class="name-cell">
              <span class="name">{{ row.displayName }}</span>
              <span v-if="row.originalName !== row.displayName" class="name-original">
                {{ row.originalName }}
              </span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="作品" width="150" show-overflow-tooltip>
          <template #default="{ row }">{{ row.workName ?? '—' }}</template>
        </el-table-column>
        <el-table-column label="分区" width="88">
          <template #default="{ row }">
            {{ sectionName(row.sectionKey) }}
          </template>
        </el-table-column>
        <el-table-column label="大小" width="92" align="right">
          <template #default="{ row }">{{ formatBytes(row.sizeBytes) }}</template>
        </el-table-column>
        <el-table-column label="时长" width="88" align="right">
          <template #default="{ row }">
            {{ row.durationMs ? formatDuration(row.durationMs) : '—' }}
          </template>
        </el-table-column>
        <el-table-column label="状态" width="96">
          <template #default="{ row }">
            <el-tag :type="ASSET_STATUS_META[row.status as AssetStatus].type" size="small">
              {{ ASSET_STATUS_META[row.status as AssetStatus].label }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="标签" width="150">
          <template #default="{ row }">
            <div class="tag-list">
              <el-tag v-for="tag in row.tags" :key="tag.id" size="small" type="info">
                {{ tag.name }}
              </el-tag>
              <span v-if="row.tags.length === 0" class="cf-muted">—</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="254" fixed="right" align="right">
          <template #default="{ row }">
            <el-button size="small" text type="primary" @click="openWork(row)">打开作品</el-button>
            <el-button
              size="small"
              text
              :disabled="row.status !== 'ready'"
              @click="reveal(row)"
            >
              定位
            </el-button>
            <el-button size="small" text @click="copyPath(row)">复制路径</el-button>
          </template>
        </el-table-column>
      </el-table>

      <div class="footer">
        <el-pagination
          :current-page="search.page"
          :page-size="search.query.limit ?? 50"
          :total="search.total"
          :page-sizes="[20, 50, 100, 200]"
          layout="total, sizes, prev, pager, next"
          background
          @current-change="search.setPage"
          @size-change="search.setPageSize"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { AudioLines, FileText, Image as ImageIcon, Search, Video } from '@lucide/vue'
import {
  ASSET_STATUS_META,
  MEDIA_KIND_OPTIONS,
  SECTION_KEYS,
  SECTION_META,
} from '@/constants/sections'
import { assetApi, tagApi, thumbnailUrl } from '@/services/api'
import { isTauriRuntime, toIpcError } from '@/services/ipc'
import { useGlobalSearchStore } from '@/stores/globalSearch'
import { useWorksStore } from '@/stores/works'
import type { Asset, AssetSortKey, AssetStatus, MediaKind, SectionKey, Tag } from '@/types'
import { formatBytes, formatDuration } from '@/utils/format'

const router = useRouter()
const search = useGlobalSearchStore()
const works = useWorksStore()

const keywordInput = ref('')
const tags = ref<Tag[]>([])
let debounceTimer: ReturnType<typeof setTimeout> | null = null

const sortValue = ref('created_at:desc')

watch(
  () => [search.query.sortKey, search.query.sortDirection],
  () => {
    sortValue.value = `${search.query.sortKey ?? 'created_at'}:${search.query.sortDirection ?? 'desc'}`
  },
)

function onKeywordInput(value: string): void {
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => search.setKeyword(value), 320)
}

watch(keywordInput, (value) => onKeywordInput(value))

function onWorkChange(value: string): void {
  search.setWork(value || null)
}

function onSectionChange(value: string): void {
  search.setSection((value || null) as SectionKey | null)
}

function onKindChange(value: string): void {
  search.setMediaKind((value || null) as MediaKind | null)
}

function onStatusChange(value: string): void {
  search.setStatus((value || null) as AssetStatus | null)
}

function onTagChange(value: string): void {
  search.setTag(value || null)
}

function onSortChange(value: string): void {
  const [key, direction] = value.split(':')
  search.setSort(key as AssetSortKey, direction === 'asc' ? 'asc' : 'desc')
}

function clearFilters(): void {
  keywordInput.value = ''
  search.resetFilters()
  void search.fetchResults()
}

function sectionName(value: string | null): string {
  if (!value) return '—'
  const meta = SECTION_META[value as SectionKey]
  return meta ? meta.name : value
}

function iconFor(asset: Asset) {
  switch (asset.mediaKind) {
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

function openWork(asset: Asset): void {
  const section = asset.sectionKey ?? 'documents'
  router.push(`/works/${asset.workId}/${section}`)
}

async function reveal(asset: Asset): Promise<void> {
  try {
    await assetApi.reveal(asset.id)
  } catch (cause) {
    ElMessage.error(toIpcError(cause).message)
  }
}

async function copyPath(asset: Asset): Promise<void> {
  try {
    await navigator.clipboard.writeText(asset.absolutePath)
    ElMessage.success('已复制文件路径')
  } catch {
    ElMessage.warning('无法写入剪贴板')
  }
}

async function loadMeta(): Promise<void> {
  await works.fetchWorks()
  try {
    tags.value = await tagApi.listAll()
  } catch {
    tags.value = []
  }
}

onMounted(async () => {
  if (!isTauriRuntime()) return
  await loadMeta()
  await search.fetchResults()
})

onUnmounted(() => {
  if (debounceTimer) clearTimeout(debounceTimer)
})
</script>

<style scoped>
.search-panel {
  padding: 16px;
  margin-bottom: 14px;
}

.search-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.search-row + .search-row {
  margin-top: 10px;
}

.search-input {
  width: 340px;
}

.filter {
  width: 190px;
}

.filter--sm {
  width: 132px;
}

.notice {
  margin-bottom: 14px;
}

.table-card {
  padding: 4px 4px 12px;
}

.summary {
  padding: 10px 14px 6px;
}

.thumb {
  width: 42px;
  height: 42px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  background: #f3f4f6;
  color: #9ca3af;
  overflow: hidden;
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.name-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.name {
  font-weight: 500;
  color: var(--cf-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.name-original {
  font-size: 12px;
  color: var(--cf-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.footer {
  display: flex;
  justify-content: flex-end;
  padding: 12px 14px 0;
}
</style>
