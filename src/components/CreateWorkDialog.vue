<template>
  <el-dialog
    v-model="visible"
    title="新建作品"
    width="520px"
    :close-on-click-modal="false"
    append-to-body
    @closed="resetForm"
  >
    <el-form label-position="top" @submit.prevent>
      <el-form-item label="作品名称" required>
        <el-input
          v-model="form.name"
          maxlength="80"
          show-word-limit
          placeholder="例如：我的第一支视频"
          autofocus
          @input="nameError = null"
        />
        <div v-if="nameError" class="field-error">{{ nameError }}</div>
      </el-form-item>

      <el-form-item label="描述（可选）">
        <el-input
          v-model="form.description"
          type="textarea"
          :rows="2"
          maxlength="500"
          placeholder="这支作品想表达什么？"
        />
      </el-form-item>

      <el-form-item label="作品模板">
        <el-select v-model="form.templateId" class="template-select" placeholder="选择模板">
          <el-option label="空白作品（不生成任务）" value="" />
          <el-option
            v-for="template in templates.items"
            :key="template.id"
            :label="template.isBuiltin ? `${template.name}（内置）` : template.name"
            :value="template.id"
          />
        </el-select>
        <div v-if="selectedTemplate" class="template-preview">
          <div class="template-preview__title">
            {{ selectedTemplate.name }}
            <span class="cf-muted">· {{ selectedTemplate.tasks.length }} 个初始任务</span>
          </div>
          <p v-if="selectedTemplate.description" class="template-preview__desc">
            {{ selectedTemplate.description }}
          </p>
          <ol class="template-preview__list">
            <li v-for="task in selectedTemplate.tasks" :key="task.id">{{ task.title }}</li>
          </ol>
        </div>
        <p v-else class="field-hint">不套用模板时，作品创建后只有四个空分区，可以稍后手动添加任务。</p>
      </el-form-item>

      <el-form-item label="作品存放目录" required>
        <div class="path-row">
          <el-input :model-value="form.rootDirectory" readonly placeholder="请选择作品根目录" />
          <el-button @click="chooseRoot">选择目录</el-button>
        </div>
        <p class="field-hint">
          应用会在该目录下创建「作品名称」文件夹，并自动建立文档、音频、视频、图片四个分区。
        </p>
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="submitting" @click="submit">
        创建作品
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useSettingsStore } from '@/stores/settings'
import { useTemplatesStore } from '@/stores/templates'
import { useWorksStore } from '@/stores/works'
import type { Work } from '@/types'
import { validateWorkName } from '@/utils/validation'

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  created: [work: Work]
}>()

const settings = useSettingsStore()
const works = useWorksStore()
const templates = useTemplatesStore()

const visible = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit('update:modelValue', value),
})

const form = reactive({
  name: '',
  description: '',
  rootDirectory: '',
  templateId: '',
})

const selectedTemplate = computed(
  () => templates.items.find((template) => template.id === form.templateId) ?? null,
)

const nameError = ref<string | null>(null)
const submitting = ref(false)

watch(
  () => props.modelValue,
  async (open) => {
    if (!open) return
    if (!form.rootDirectory) {
      form.rootDirectory = settings.settings.workspaceRoot ?? ''
    }
    if (templates.items.length === 0) {
      await templates.load()
    }
    if (!form.templateId) {
      form.templateId = templates.defaultId ?? ''
    }
  },
)

async function chooseRoot(): Promise<void> {
  const selected = await settings.pickDirectory('选择作品根目录')
  if (selected) form.rootDirectory = selected
}

function resetForm(): void {
  form.name = ''
  form.description = ''
  nameError.value = null
  submitting.value = false
}

async function submit(): Promise<void> {
  const validationError = validateWorkName(form.name.trim())
  if (validationError) {
    nameError.value = validationError
    return
  }
  if (!form.rootDirectory) {
    ElMessage.warning('请先选择作品存放目录')
    return
  }

  submitting.value = true
  const created = await works.createWork({
    name: form.name.trim(),
    description: form.description.trim() ? form.description.trim() : null,
    rootDirectory: form.rootDirectory,
    templateKey: form.templateId || null,
  })
  submitting.value = false

  if (created) {
    ElMessage.success(`作品「${created.name}」已创建`)
    emit('created', created)
    visible.value = false
  } else {
    ElMessage.error(works.error ?? '创建作品失败')
  }
}
</script>

<style scoped>
.path-row {
  display: flex;
  gap: 8px;
  width: 100%;
}

.field-error {
  color: #c45656;
  font-size: 12px;
  line-height: 1.6;
  margin-top: 2px;
}

.template-select {
  width: 100%;
}

.template-preview {
  width: 100%;
  margin-top: 8px;
  padding: 10px 12px;
  border-radius: 8px;
  background: #f8f9fb;
  border: 1px solid var(--cf-border);
}

.template-preview__title {
  font-size: 13px;
  font-weight: 500;
}

.template-preview__desc {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--cf-text-muted);
}

.template-preview__list {
  margin: 6px 0 0;
  padding-left: 18px;
  font-size: 12px;
  color: var(--cf-text-muted);
  line-height: 1.7;
}

.field-hint {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--cf-text-muted);
  line-height: 1.6;
}
</style>
