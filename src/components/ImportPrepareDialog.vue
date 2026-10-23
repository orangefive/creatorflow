<template>
  <el-dialog
    v-model="visible"
    title="确认导入"
    width="640px"
    :close-on-click-modal="false"
    append-to-body
  >
    <p class="hint">
      文件会被<strong>复制</strong>到作品目录，源文件保持不动。可在此调整每个文件的目标分区。
    </p>

    <el-table :data="rows" max-height="360" size="small" class="table">
      <el-table-column label="文件" min-width="220">
        <template #default="{ row }">
          <div class="file-cell">
            <span class="file-name" :title="row.sourcePath">{{ row.fileName }}</span>
            <el-tag v-if="!row.originalSuggested" size="small" type="warning" effect="plain">
              类型未识别
            </el-tag>
          </div>
        </template>
      </el-table-column>
      <el-table-column label="大小" width="100" align="right">
        <template #default="{ row }">{{ formatBytes(row.sizeBytes) }}</template>
      </el-table-column>
      <el-table-column label="目标分区" width="150">
        <template #default="{ row }">
          <el-select v-model="row.sectionKey" size="small">
            <el-option
              v-for="section in SECTION_LIST"
              :key="section.key"
              :label="section.name"
              :value="section.key"
            />
          </el-select>
        </template>
      </el-table-column>
    </el-table>

    <template #footer>
      <div class="footer">
        <span class="cf-muted">{{ rows.length }} 个文件</span>
        <div>
          <el-button @click="visible = false">取消</el-button>
          <el-button type="primary" :loading="submitting" @click="confirm">
            开始导入
          </el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { SECTION_LIST } from '@/constants/sections'
import type { ImportCandidate, SectionKey } from '@/types'
import { formatBytes } from '@/utils/format'

interface Row {
  sourcePath: string
  fileName: string
  sizeBytes: number
  originalSuggested: SectionKey | null
  sectionKey: SectionKey
}

const props = defineProps<{
  modelValue: boolean
  candidates: ImportCandidate[]
  fallbackSectionKey: SectionKey
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  submit: [groups: Array<{ sectionKey: SectionKey; sourcePaths: string[] }>]
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit('update:modelValue', value),
})

const rows = ref<Row[]>([])
const submitting = ref(false)

watch(
  () => props.candidates,
  (candidates) => {
    rows.value = candidates.map((candidate) => ({
      sourcePath: candidate.sourcePath,
      fileName: candidate.fileName,
      sizeBytes: candidate.sizeBytes,
      originalSuggested: candidate.suggestedSectionKey,
      sectionKey: candidate.suggestedSectionKey ?? props.fallbackSectionKey,
    }))
  },
  { immediate: true, deep: true },
)

function confirm(): void {
  const grouped = new Map<SectionKey, string[]>()
  for (const row of rows.value) {
    const list = grouped.get(row.sectionKey) ?? []
    list.push(row.sourcePath)
    grouped.set(row.sectionKey, list)
  }
  submitting.value = true
  emit(
    'submit',
    Array.from(grouped.entries()).map(([sectionKey, sourcePaths]) => ({
      sectionKey,
      sourcePaths,
    })),
  )
  submitting.value = false
  visible.value = false
}
</script>

<style scoped>
.hint {
  margin: 0 0 12px;
  font-size: 13px;
  color: var(--cf-text-muted);
}

.table {
  border: 1px solid var(--cf-border);
  border-radius: 8px;
}

.file-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}

.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
</style>
