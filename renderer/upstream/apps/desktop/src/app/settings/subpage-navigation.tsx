import { useI18n } from '@/i18n'

import { OverlayBreadcrumbHeader } from '../overlays/overlay-breadcrumb-header'
import type { OverlayNavGroup, OverlayNavLink } from '../overlays/overlay-split-layout'

export function SettingsSubpageHeader({ group, child, category }: { group: OverlayNavGroup; child?: OverlayNavLink; category?: OverlayNavGroup }) {
  const { t } = useI18n()

  // A third rail level (a plugin's sub-page under Settings ▸ Plugins).
  const grandchild = child?.children?.find(link => link.active)

  if ((window as Window & { __hermesRustShell?: boolean }).__hermesRustShell) {
    const pages = group.id === 'config:voice' ? [...(group.children ?? [])].sort((a, b) => {
      const order = ['transcription', 'conversation', 'speech']

      return order.indexOf(a.id.split(':').at(-1)!) - order.indexOf(b.id.split(':').at(-1)!)
    }) : group.children

    const pageLabel = (page: OverlayNavLink) => group.id !== 'config:voice' ? page.label
      : page.id.endsWith(':transcription') ? 'Dictation' : page.id.endsWith(':speech') ? 'Spoken replies' : page.label

    return <header className="mb-5 px-[clamp(1.25rem,4vw,4rem)] pt-4">
      <h2 className="text-2xl font-semibold tracking-tight">{category?.label ?? group.label}</h2>
      {group.id === 'config:voice' && <p className="mt-2 text-sm text-muted-foreground">Talk to Hermes and choose how it listens and speaks.</p>}
      {!!group.children?.length && <nav aria-label={group.label} className="mt-5 flex flex-wrap gap-1 rounded-lg border border-border bg-card p-1">
        {pages?.map(page => <button aria-current={page.active ? 'page' : undefined} className={`rounded-md px-4 py-2 text-sm ${page.active ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-accent hover:text-foreground'}`} key={page.id} onClick={page.onSelect} type="button">{pageLabel(page)}</button>)}
      </nav>}
    </header>
  }

  return (
    <OverlayBreadcrumbHeader child={child} grandchild={grandchild} group={group} rootLabel={t.commandCenter.settings} />
  )
}
