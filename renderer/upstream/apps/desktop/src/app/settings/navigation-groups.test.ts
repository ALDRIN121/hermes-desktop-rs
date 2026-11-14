import { describe, expect, it, vi } from 'vitest'

import { groupSettingsNavigation } from './navigation-groups'

const entry = (id: string, active = false) => ({
  id, label: id, active, icon: () => null, onSelect: vi.fn(),
  children: [{ id: `${id}:detail`, label: 'Detail', active, icon: () => null, onSelect: vi.fn() }]
})

describe('simplified settings navigation', () => {
  it('keeps every original destination and nested field route reachable', () => {
    const original = ['config:appearance', 'config:chat', 'providers', 'config:model',
      'config:voice', 'gateway', 'web-access', 'billing', 'keys', 'config:workspace',
      'config:browser', 'config:memory', 'config:safety', 'vault', 'plugins',
      'config:advanced', 'notifications', 'keybinds', 'sessions', 'about'].map(id => entry(id))

    const groups = groupSettingsNavigation(original)
    expect(groups).toHaveLength(9)
    expect(groups.flatMap(group => group.children).map(child => child?.id).sort())
      .toEqual(original.map(item => item.id).sort())
    expect(groups.flatMap(group => group.children).find(item => item?.id === 'providers')?.children)
      .toBe(original.find(item => item.id === 'providers')?.children)
  })

  it('reveals the active voice section and retains its original action', () => {
    const voice = entry('config:voice', true)
    const group = groupSettingsNavigation([voice])[0]
    expect(group.label).toBe('Voice & audio')
    expect(group.active).toBe(true)
    group.onSelect()
    expect(voice.onSelect).toHaveBeenCalledOnce()
  })

  it('keeps unknown contributed destinations under Advanced instead of dropping them', () => {
    const extra = entry('custom:future', true)
    const group = groupSettingsNavigation([extra])[0]
    expect(group.label).toBe('Advanced & diagnostics')
    expect(group.children).toEqual([extra])
    expect(group.active).toBe(true)
  })
})
