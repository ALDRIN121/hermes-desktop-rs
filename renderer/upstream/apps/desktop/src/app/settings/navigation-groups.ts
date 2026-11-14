import type { OverlayNavGroup } from '../overlays/overlay-split-layout'

const CATEGORIES = [
  ['general', 'General', ['config:appearance', 'config:chat', 'notifications', 'keybinds', 'sessions']],
  ['models', 'Models & providers', ['config:model', 'providers']],
  ['voice', 'Voice & audio', ['config:voice']],
  ['connections', 'Connections & accounts', ['gateway', 'connections', 'web-access', 'billing', 'keys']],
  ['workspace', 'Workspace & browser', ['config:workspace', 'config:browser']],
  ['memory', 'Memory & safety', ['config:memory', 'config:safety', 'vault']],
  ['extensions', 'Skills & extensions', ['plugins']],
  ['advanced', 'Advanced & diagnostics', ['config:advanced']],
  ['about', 'About & updates', ['about']]
] as const

/** Group presentation only: original actions, page IDs and plugin children survive intact. */
export function groupSettingsNavigation(
  entries: OverlayNavGroup[],
  labels: Record<string, string> = {}
): OverlayNavGroup[] {
  const known = new Set<string>(CATEGORIES.flatMap(([, , ids]) => [...ids]))

  return CATEGORIES.flatMap(([id, label, ids]) => {
    const children = entries.filter(entry => (ids as readonly string[]).includes(entry.id) ||
      (id === 'advanced' && !known.has(entry.id)))

    if (!children.length) {return []}
    const selected = children.find(child => child.active) ?? children[0]

    return [{
      id: `settings-group:${id}`,
      label: labels[id] ?? label,
      icon: selected.icon,
      active: children.some(child => child.active),
      children,
      childrenMode: id === 'voice' ? 'tabs' as const : undefined,
      onSelect: selected.onSelect
    }]
  })
}
