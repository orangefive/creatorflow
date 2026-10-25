<template>
  <el-dialog
    v-model="visible"
    :title="task ? '编辑任务' : '新建任务'"
    width="520px"
    :close-on-click-modal="false"
    append-to-body
    @closed="reset"
  >
    <el-form label-position="top" @submit.prevent>
      <el-form-item label="任务标题" required>
        <el-input
          v-model="form.title"
          maxlength="120"
          show-word-limit
          placeholder="例如：完成粗剪"
          @input="titleError = null"
        />
        <div v-if="titleError" class="field-error">{{ titleError }}</div>
      </el-form-item>

      <el-form-item label="描述（可选）">
        <el-input
          v-model="form.description"
          type="textarea"
          :rows="3"
          maxlength="2000"
          placeholder="补充要求、参考链接或验收标准"
        />
      </el-form-item>

      <div class="form-row">
        <el-form-item label="优先级">
          <el-select v-model="form.priority">
            <el-option
              v-for="(meta, value) in TASK_PRIORITY_META"
              :key="value"
              :label="meta.label"
              :value="value"
            />
          </el-select>
        </el-form-item>
        <el-form-item label="截止时间（可选）">
          <el-date-picker
            v-model="form.dueAt"
            type="datetime"
            placeholder="不设置"
            :clearable="true"
          />
        </el-form-item>
      </div>

      <el-form-item v-if="task" label="状态">
        <el-select v-model="form.status">
          <el-option
            v-for="(meta, value) in TASK_STATUS_META"
            :key="value"
            :label="meta.label"
            :value="value"
          />
        </el-select>
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="submitting" @click="submit">
        {{ task ? '保存' : '创建任务' }}
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { TASK_PRIORITY_META, TASK_STATUS_META } from '@/constants/tasks'
import { taskApi } from '@/services/api'
import { toIpcError } from '@/services/ipc'
import type { Task, TaskPriority, TaskStatus } from '@/types'

const props = defineProps<{
  modelValue: boolean
  workId: string
  task: Task | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  saved: []
}>()

const submitting = ref(false)
const titleError = ref<string | null>(null)

const visible = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit('update:modelValue', value),
})

const form = reactive({
  title: '',
  description: '',
  priority: 'normal' as TaskPriority,
  dueAt: null as Date | null,
  status: 'todo' as TaskStatus,
})

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    const task = props.task
    form.title = task?.title ?? ''
    form.description = task?.description ?? ''
    form.priority = task?.priority ?? 'normal'
    form.dueAt = task?.dueAt ? new Date(task.dueAt) : null
    form.status = task?.status ?? 'todo'
    titleError.value = null
  },
)

function reset(): void {
  submitting.value = false
}

async function submit(): Promise<void> {
  const title = form.title.trim()
  if (!title) {
    titleError.value = '请填写任务标题'
    return
  }

  submitting.value = true
  const payload = {
    title,
    description: form.description.trim() || null,
    priority: form.priority,
    dueAt: form.dueAt ? form.dueAt.toISOString() : null,
    ...(props.task ? { status: form.status } : {}),
  }

  try {
    if (props.task) await taskApi.update(props.task.id, payload)
    else await taskApi.create(props.workId, payload)
    ElMessage.success(props.task ? '任务已保存' : '任务已创建')
    emit('saved')
    visible.value = false
  } catch (cause) {
    ElMessage.error(toIpcError(cause).message)
  } finally {
    submitting.value = false
  }
}
</script>

<style scoped>
.form-row {
  display: flex;
  gap: 16px;
}

.form-row :deep(.el-form-item) {
  flex: 1;
}

.field-error {
  color: #c45656;
  font-size: 12px;
  line-height: 1.6;
  margin-top: 2px;
}
</style>
