import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import WorkStatusTag from '@/components/WorkStatusTag.vue'

const ElTagStub = {
  name: 'ElTag',
  props: ['type', 'size', 'effect'],
  template: '<span class="el-tag" :data-type="type"><slot /></span>',
}

function render(status: 'idea' | 'in_production' | 'archived') {
  return mount(WorkStatusTag, {
    props: { status },
    global: { components: { ElTag: ElTagStub } },
  })
}

describe('WorkStatusTag', () => {
  it('renders the Chinese label for a status', () => {
    expect(render('in_production').text()).toBe('制作中')
  })

  it('maps archived to its own label', () => {
    expect(render('archived').text()).toBe('已归档')
  })

  it('passes a tag type to Element Plus', () => {
    const wrapper = render('idea')
    expect(wrapper.find('.el-tag').attributes('data-type')).toBe('info')
  })
})
