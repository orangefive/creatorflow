<template>
  <div class="cf-page">
    <div class="cf-page-header">
      <div>
        <h1 class="cf-page-title">{{ showAll ? '所有作品' : '首页' }}</h1>
        <p class="cf-page-subtitle">
          {{ showAll ? '查看全部作品与归档状态' : '以作品为单位，管理你的素材与制作进度' }}
        </p>
      </div>
      <div class="header-actions">
        <el-button :icon="RefreshCw" :loading="works.loading" @click="reload">刷新</el-button>
        <el-button type="primary" :icon="Plus" @click="openCreateDialog">新建作品</el-button>
      </div>
    </div>

    <el-alert
      v-if="!settings.hasWorkspaceRoot"
      class="notice"
      type="info"
      :closable="false"
      show-icon
      title="还没有设置作品根目录"
    >
      <div class="notice__body">
        <span>新建作品前需要先选择一个存放所有作品的目录。</span>
        <el-button size="small" type="primary" plain @click="chooseRoot">选择目录</el-button>
      </div>
    </el-alert>

    <el-alert
      v-if="works.error"
      class="notice"
      type="error"
      :closable="false"
      show-icon
      :title="works.error"
    />

    <div v-if="!showAll" class="recent-block">
      <div class="section-head">
        <h2 class="section-title">最近作品</h2>
        <router-link to="/?view=all" class="section-link">查看全部</router-link>
      </div>

      <el-skeleton v-if="works.loading && works.works.length === 0" :rows="3" animated />

      <div v-else-if="works.recentWorks.length === 0" class="cf-card">
        <div class="cf-empty">
          <component :is="FolderOpen" :size="30" :stroke-width="1.5" />
          <p class="empty-title">还没有作品</p>
          <p>创建第一个作品，应用会自动建立目录结构和素材分区。</p>
          <el-button type="primary" :disabled="!settings.hasWorkspaceRoot" @click="openCreateDialog">
            新建作品
          </el-button>
        </div>
      </div>

      <div v-else class="work-grid">
        <div v-for="work in works.recentWorks" :key="work.id" class="work-card-wrap">
          <button type="button" class="work-card" @click="openWork(work.id)">
            <div class="work-card__top">
              <span class="work-card__name">{{ work.name }}</span>
              <WorkStatusTag :status="work.status" />
            </div>
            <p class="work-card__desc">{{ work.description || '暂无描述' }}</p>
            <div class="work-card__meta">
              <span>{{ work.assetCount }} 个素材</span>
              <span class="work-card__time">{{ formatDateTime(work.updatedAt) }}</span>
            </div>
          </button>
          <button
            type="button"
            class="work-card__delete"
            title="删除作品（移到回收站）"
            aria-label="删除作品"
            @click.stop="remove(work)"
          >
            <component :is="Trash2" :size="15" :stroke-width="1.8" />
          </button>
        </div>
      </div>
    </div>

    <div v-else>
      <div class="cf-toolbar">
        <el-switch v-model="works.includeArchived" active-text="包含已归档" @change="reload" />
        <span class="cf-spacer" />
        <span class="cf-muted">共 {{ works.works.length }} 个作品</span>
      </div>

      <el-table
        v-loading="works.loading"
        :data="works.works"
        class="cf-card"
        row-key="id"
        empty-text="暂无作品"
      >
        <el-table-column prop="name" label="作品名称" min-width="200">
          <template #default="{ row }">
            <span class="table-name">{{ row.name }}</span>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="110">
          <template #default="{ row }">
            <WorkStatusTag :status="row.status" />
          </template>
        </el-table-column>
        <el-table-column prop="assetCount" label="素材" width="90" align="right" />
        <el-table-column label="创建时间" width="150">
          <template #default="{ row }">{{ formatDateTime(row.createdAt) }}</template>
        </el-table-column>
        <el-table-column label="更新时间" width="150">
          <template #default="{ row }">{{ formatDateTime(row.updatedAt) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="272" fixed="right">
          <template #default="{ row }">
            <el-button size="small" text type="primary" @click="openWork(row.id)">打开</el-button>
            <el-button size="small" text @click="rename(row)">重命名</el-button>
            <el-button
              v-if="row.status === 'archived'"
              size="small"
              text
              @click="unarchive(row)"
            >
              恢复
            </el-button>
            <el-button v-else size="small" text @click="archive(row)">归档</el-button>
            <el-button size="small" text type="danger" @click="remove(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <CreateWorkDialog v-model="createDialogVisible" @created="onWorkCreated" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { FolderOpen, Plus, RefreshCw, Trash2 } from '@lucide/vue'
import CreateWorkDialog from '@/components/CreateWorkDialog.vue'
import WorkStatusTag from '@/components/WorkStatusTag.vue'
import { useSettingsStore } from '@/stores/settings'
import { useWorksStore } from '@/stores/works'
import { isTauriRuntime } from '@/services/ipc'
import type { Work } from '@/types'
import { formatDateTime } from '@/utils/format'
import { validateWorkName } from '@/utils/validation'

const route = useRoute()
const router = useRouter()
const works = useWorksStore()
const settings = useSettingsStore()
const createDialogVisible = ref(false)

const showAll = computed(() => route.query.view === 'all')

async function reload(): Promise<void> {
  await works.fetchWorks()
}

function openCreateDialog(): void {
  if (!settings.hasWorkspaceRoot) {
    ElMessage.warning('请先设置作品根目录')
    return
  }
  createDialogVisible.value = true
}

async function chooseRoot(): Promise<void> {
  const ok = await settings.chooseWorkspaceRoot()
  if (ok) ElMessage.success('作品根目录已保存')
  else if (settings.error) ElMessage.error(settings.error)
}

function openWork(workId: string): void {
  router.push(`/works/${workId}/documents`)
}

function onWorkCreated(work: Work): void {
  openWork(work.id)
}

async function rename(work: Work): Promise<void> {
  try {
    const result = await ElMessageBox.prompt('输入新的作品名称，作品文件夹会一并重命名。', '重命名作品', {
      inputValue: work.name,
      confirmButtonText: '保存',
      cancelButtonText: '取消',
      inputValidator: (value: string) => validateWorkName(value) ?? true,
    })
    const newName = result.value.trim()
    if (newName === work.name) return
    const ok = await works.renameWork(work.id, newName)
    if (ok) {
      ElMessage.success('作品已重命名')
    } else {
      ElMessage.error(works.error ?? '重命名失败')
    }
  } catch {
    // 用户取消
  }
}

async function archive(work: Work): Promise<void> {
  try {
    await ElMessageBox.confirm(
      `归档「${work.name}」？归档不会移动或删除磁盘文件，之后仍可在包含归档的列表中打开。`,
      '归档作品',
      { confirmButtonText: '归档', cancelButtonText: '取消', type: 'warning' },
    )
  } catch {
    return
  }
  const ok = await works.archiveWork(work.id)
  if (ok) ElMessage.success('作品已归档')
  else ElMessage.error(works.error ?? '归档失败')
}

async function unarchive(work: Work): Promise<void> {
  const ok = await works.unarchiveWork(work.id)
  if (ok) {
    const status = work.archivedFromStatus
    ElMessage.success(status ? '已取消归档，状态已恢复' : '已取消归档')
  } else {
    ElMessage.error(works.error ?? '取消归档失败')
  }
}

async function remove(work: Work): Promise<void> {
  try {
    await ElMessageBox.confirm(
      `删除作品「${work.name}」？\n\n整个作品文件夹（${work.assetCount} 个素材）会被移到系统回收站，` +
        '应用内的记录也会移除。文件仍可从回收站还原。',
      '删除作品',
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
  const ok = await works.deleteWork(work.id)
  if (ok) ElMessage.success('作品已移入回收站')
  else ElMessage.error(works.error ?? '删除失败')
}

watch(showAll, () => {
  void reload()
})

onMounted(async () => {
  // 浏览器预览（无 Tauri 容器）时没有后端，跳过加载以免展示无意义的错误。
  if (!isTauriRuntime()) return
  if (!settings.info) await settings.load()
  await reload()
})
</script>

<style scoped>
.header-actions {
  display: flex;
  gap: 8px;
}

.notice {
  margin-bottom: 16px;
}

.notice__body {
  display: flex;
  align-items: center;
  gap: 12px;
}

.section-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: 12px;
}

.section-title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}

.section-link {
  font-size: 13px;
}

.work-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 14px;
}

.work-card-wrap {
  position: relative;
  display: flex;
}

.work-card {
  flex: 1;
  text-align: left;
  padding: 16px;
  border: 1px solid var(--cf-border);
  border-radius: var(--cf-radius);
  background: var(--cf-surface);
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    box-shadow 0.15s ease;
  font-family: inherit;
}

.work-card:hover {
  border-color: #c7d3ee;
  box-shadow: 0 4px 14px rgba(31, 41, 55, 0.06);
}

.work-card__top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.work-card__name {
  font-weight: 600;
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.work-card__desc {
  margin: 8px 0 14px;
  font-size: 12px;
  color: var(--cf-text-muted);
  min-height: 18px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.work-card__meta {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: #9ca3af;
}

.work-card__time {
  transition: opacity 0.12s ease;
}

/* 删除按钮与日期占同一位置：悬停时日期淡出、按钮淡入，避免布局抖动 */
.work-card__delete {
  position: absolute;
  right: 9px;
  bottom: 9px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: #9ca3af;
  cursor: pointer;
  opacity: 0;
  pointer-events: none;
  transition:
    opacity 0.12s ease,
    color 0.12s ease,
    background 0.12s ease;
}

.work-card-wrap:hover .work-card__delete,
.work-card__delete:focus-visible {
  opacity: 1;
  pointer-events: auto;
}

.work-card-wrap:hover .work-card__time {
  opacity: 0;
}

.work-card__delete:hover {
  color: #e5484d;
  background: rgb(229 72 77 / 10%);
}

.empty-title {
  margin: 10px 0 4px;
  font-weight: 600;
  color: var(--cf-text);
}

.table-name {
  font-weight: 500;
}
</style>
