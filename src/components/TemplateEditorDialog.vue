<template>
  <el-dialog
    v-model="visible"
    :title="isEdit ? '编辑模板' : '新建模板'"
    width="640px"
    :close-on-click-modal="false"
    append-to-body
    @closed="reset"
  >
    <el-form label-position="top" @submit.prevent>
      <el-form-item label="模板名称" required>
        <el-input v-model="form.name" maxlength="60" show-word-limit placeholder="例如：口播短视频" />
        <div v-if="nameError" class="field-error">{{ nameError }}</div>
      </el-form-item>

      <el-form-item label="说明（可选）">
        <el-input
          v-model="form.description"
          type="textarea"
          :rows="2"
          maxlength="500"
          placeholder="这个模板适合什么类型的作品"
        />
      </el-form-item>

      <el-form-item label="初始任务">
        <div class="task-editor">
          <div v-for="(task, index) in form.tasks" :key="index" class="task-row">
            <span class="task-index">{{ index + 1 }}</span>
            <el-input v-model="task.title" maxlength="120" placeholder="任务标题" />
            <el-select v-model="task.priority" class="task-priority">
              <el-option
                v-for="option in TASK_PRIORITY_OPTIONS"
                :key="option.value"
                :label="option.label"
                :value="option.value"
              />
            </el-select>
            <el-button text type="danger" @click="removeTask(index)">删除</el-button>
          </div>
          <el-button text type="primary" @click="addTask">添加任务</el-button>
          <p v-if="form.tasks.length === 0" class="field-hint">
            没有任务也可以保存，套用该模板创建的作品不会自带任务。
          </p>
        </div>
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="submitting" @click="submit">
        {{ isEdit ? '保存模板' : '创建模板' }}
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { TASK_PRIORITY_OPTIONS } from '@/constants/tasks'
import { useTemplatesStore } from '@/stores/templates'
import type { TaskPriority, WorkTemplate } from '@/types'

const props = defineProps<{
  modelValue: boolean
  template: WorkTemplate | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  saved: []
}>()

const templates = useTemplatesStore()
const submitting = ref(false)
const nameError = ref<string | null>(null)

const visible = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit('update:modelValue', value),
})

const isEdit = computed(() => Boolean(props.template))

const form = reactive({
  name: '',
  description: '',
  tasks: [] as { title: string; description: string | null; priority: TaskPriority }[],
})

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    const template = props.template
    form.name = template?.name ?? ''
    form.description = template?.description ?? ''
    form.tasks = (template?.tasks ?? []).map((task) => ({
      title: task.title,
      description: task.description,
      priority: task.priority,
    }))
    nameError.value = null
  },
)

function addTask(): void {
  form.tasks.push({ title: '', description: null, priority: 'normal' })
}

function removeTask(index: number): void {
  form.tasks.splice(index, 1)
}

function reset(): void {
  submitting.value = false
}

async function submit(): Promise<void> {
  const name = form.name.trim()
  if (!name) {
    nameError.value = '请填写模板名称'
    return
  }
  const tasks = form.tasks
    .map((task) => ({ ...task, title: task.title.trim() }))
    .filter((task) => task.title.length > 0)

  if (tasks.length !== form.tasks.length) {
    ElMessage.warning('已忽略标题为空的任务')
  }

  submitting.value = true
  const payload = {
    name,
    description: form.description.trim() || null,
    tasks,
  }
  const ok = props.template
    ? await templates.update(props.template.id, payload)
    : Boolean(await templates.create(payload))
  submitting.value = false

  if (ok) {
    ElMessage.success(props.template ? '模板已保存' : '模板已创建')
    emit('saved')
    visible.value = false
  } else {
    ElMessage.error(templates.error ?? '保存模板失败')
  }
}
</script>

<style scoped>
.task-editor {
  width: 100%;
}

.task-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.task-index {
  width: 18px;
  color: var(--cf-text-muted);
  font-size: 12px;
  text-align: right;
}

.task-priority {
  width: 96px;
  flex: 0 0 96px;
}

.field-error {
  color: #c45656;
  font-size: 12px;
}

.field-hint {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--cf-text-muted);
}
</style>
