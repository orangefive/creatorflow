<template>
  <AppLayout>
    <router-view v-slot="{ Component }">
      <component :is="Component" />
    </router-view>
  </AppLayout>
  <ImportQueuePanel />
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { ElNotification } from 'element-plus'
import AppLayout from '@/layouts/AppLayout.vue'
import ImportQueuePanel from '@/components/ImportQueuePanel.vue'
import { importApi } from '@/services/api'
import { isTauriRuntime } from '@/services/ipc'
import { useImportsStore } from '@/stores/imports'

const imports = useImportsStore()

onMounted(async () => {
  if (!isTauriRuntime()) return
  try {
    const report = await importApi.startupReport()
    if (report && report.messages.length > 0) {
      ElNotification({
        title: '启动检查',
        message: report.messages.join('；'),
        type: 'warning',
        duration: 8000,
      })
    }
    await imports.refresh(null)
  } catch {
    // 启动检查失败不阻断使用，具体错误已写入后端日志
  }
})
</script>
