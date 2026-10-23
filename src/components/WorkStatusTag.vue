<template>
  <el-tag :type="meta.type" size="small" effect="light" disable-transitions>
    {{ meta.label }}
  </el-tag>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { WORK_STATUS_META } from '@/constants/sections'
import type { WorkStatus } from '@/types'

const props = defineProps<{ status: WorkStatus }>()

const meta = computed(() => WORK_STATUS_META[props.status] ?? { label: props.status, type: 'info' as const })
</script>
