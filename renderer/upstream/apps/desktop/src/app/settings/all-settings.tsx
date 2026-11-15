import { useStore } from '@nanostores/react'
import { useMemo, useState } from 'react'

import { saveHermesConfigRecord } from '@/api/config'
import { hermesConfigKey, useHermesConfigRecord } from '@/app/hooks/use-config-record'
import { Input } from '@/components/ui/input'
import { useI18n } from '@/i18n'
import { FileText, FolderOpen } from '@/lib/icons'
import { queryClient } from '@/lib/query-client'
import { $activeConnectionId } from '@/store/connections'
import { notify, notifyError } from '@/store/notifications'
import { $settingsRequestProfile } from '@/store/settings-scope'
import type { HermesConfigRecord } from '@/types/hermes'

import { ListRow, SectionHeading, SettingsContent, ToggleRow } from './primitives'

export type ConfigLeafKind = 'array' | 'boolean' | 'null' | 'number' | 'object' | 'string'

export interface ConfigLeaf {
  /** Dotted path, exactly as config.yaml spells it. */
  path: string
  segments: string[]
  secret: boolean
  /** Top-level section, used for the group headings. */
  section: string
  value: unknown
  kind: ConfigLeafKind
}

const kindOf = (value: unknown): ConfigLeafKind => {
  if (value === null) {
    return 'null'
  }

  if (Array.isArray(value)) {
    return 'array'
  }

  const type = typeof value

  if (type === 'boolean' || type === 'number' || type === 'string') {
    return type
  }

  return 'object'
}

const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

/**
 * Every leaf of the config record, in yaml order. A container with no children
 * is still a leaf — an empty section is something a user can fill in, not
 * something to hide.
 */
export function flattenConfig(record: HermesConfigRecord, segments: string[] = [], out: ConfigLeaf[] = []): ConfigLeaf[] {
  for (const [key, value] of Object.entries(record ?? {})) {
    const parts = [...segments, key]
    const path = parts.join('.')
    const section = parts[0]!

    if (isPlainObject(value) && Object.keys(value).length > 0) {
      flattenConfig(value, parts, out)
    } else {
      const secret = parts.some(part => /(?:^|[_-])(?:api[_-]?key|token|secret|password|credential|authorization|private[_-]?key)(?:$|[_-])/i.test(part.replace(/([a-z])([A-Z])/g, '$1_$2')))
      out.push({ kind: kindOf(value), path, section, secret, segments: parts, value })
    }
  }

  return out
}

/** Keep literal dots in dynamic keys; never re-interpret a display label as a path. */
export function patchForLeaf(leaf: ConfigLeaf, value: unknown): HermesConfigRecord {
  let patch = value

  for (const segment of [...leaf.segments].reverse()) {
    if (['__proto__', 'constructor', 'prototype'].includes(segment)) {
      throw new Error('Unsafe config key')
    }

    patch = Object.fromEntries([[segment, patch]])
  }

  return patch as HermesConfigRecord
}

export function parseLeafInput(leaf: ConfigLeaf, text: string): unknown {
  if (leaf.kind === 'string') {
    return text
  }

  const parsed: unknown = leaf.kind === 'number' ? Number(text) : JSON.parse(text)

  const finite = (value: unknown): boolean => {
    if (typeof value === 'number') {
      return Number.isFinite(value)
    }

    return value === null || typeof value !== 'object' || Object.values(value).every(finite)
  }

  if (text.trim() === '' || !finite(parsed) || (leaf.kind !== 'null' && kindOf(parsed) !== leaf.kind)) {
    throw new Error('Invalid value type')
  }

  return parsed
}

function displayValue(leaf: ConfigLeaf): string {
  if (leaf.kind === 'array' || leaf.kind === 'object') {
    return JSON.stringify(leaf.value)
  }

  if (leaf.kind === 'null') {
    return 'null'
  }

  return String(leaf.value)
}

/** Case-insensitive match on the key path or the rendered value. */
export function filterLeaves(leaves: readonly ConfigLeaf[], query: string): ConfigLeaf[] {
  const needle = query.trim().toLowerCase()

  if (!needle) {
    return [...leaves]
  }

  return leaves.filter(
    leaf => leaf.path.toLowerCase().includes(needle) || (!leaf.secret && displayValue(leaf).toLowerCase().includes(needle))
  )
}

function LeafEditor({ leaf, onCommit }: { leaf: ConfigLeaf; onCommit: (value: unknown) => void }) {
  const { t } = useI18n()
  const copy = t.settings.allSettings
  const [text, setText] = useState(() => displayValue(leaf))
  const [error, setError] = useState<null | string>(null)

  if (leaf.kind === 'boolean') {
    return <ToggleRow checked={leaf.value === true} label="" onChange={next => onCommit(next)} />
  }

  const commit = () => {
    if (text === displayValue(leaf)) {
      setError(null)

      return
    }

    try {
      onCommit(parseLeafInput(leaf, text))
      setError(null)
    } catch {
      setError(copy.invalidJson)
    }
  }

  return (
    <div className="min-w-0">
      <Input
        aria-invalid={error ? true : undefined}
        aria-label={leaf.path}
        autoComplete="off"
        className="font-mono text-[0.75rem]"
        onBlur={commit}
        onChange={event => {
          setText(event.target.value)
          setError(null)
        }}
        onKeyDown={event => {
          if (event.key === 'Enter') {
            event.currentTarget.blur()
          }
        }}
        spellCheck={false}
        type={leaf.secret ? 'password' : 'text'}
        value={text}
      />
      {error && <p className="mt-1 text-[0.7rem] text-destructive">{error}</p>}
    </div>
  )
}

/** Every config key in the profile, searchable, editable in place. */
export function AllSettings() {
  const { t } = useI18n()
  const copy = t.settings.allSettings
  const profile = useStore($settingsRequestProfile)
  const connectionId = useStore($activeConnectionId)
  const [query, setQuery] = useState('')
  const record = useHermesConfigRecord(profile ?? undefined)
  // Capture the read's owner and cache slot now. A later scope switch must
  // not retarget a save or its completion invalidation to another gateway.
  const writeScope = record.writeScope
  const cacheKey = hermesConfigKey(profile ?? undefined, connectionId)
  const editorScope = JSON.stringify([connectionId, profile])

  const config = useMemo(() => (record.data ?? {}) as HermesConfigRecord, [record.data])
  const leaves = useMemo(() => flattenConfig(config), [config])
  const shown = useMemo(() => filterLeaves(leaves, query), [leaves, query])

  const groups = useMemo(() => {
    const bySection = new Map<string, ConfigLeaf[]>()

    for (const leaf of shown) {
      const list = bySection.get(leaf.section) ?? []

      list.push(leaf)
      bySection.set(leaf.section, list)
    }

    return [...bySection.entries()]
  }, [shown])

  const save = async (leaf: ConfigLeaf, value: unknown) => {
    try {
      if (JSON.stringify(value) === JSON.stringify(leaf.value)) {
        return
      }

      const patch = patchForLeaf(leaf, value)
      const result = await saveHermesConfigRecord(patch, writeScope ?? profile ?? undefined)

      if (!result.ok) {
        throw new Error(copy.saveFailed)
      }

      notify({ kind: 'info', message: copy.saved })
      await queryClient.invalidateQueries({ queryKey: cacheKey })
    } catch (error) {
      notifyError(error, copy.saveFailed)
    }
  }

  return (
    <SettingsContent>
      <SectionHeading icon={FileText} title={copy.title} />
      <p className="mb-1 text-[length:var(--conversation-caption-font-size)] text-(--ui-text-tertiary)">{copy.intro}</p>
      <p className="mb-3 text-[0.7rem] text-muted-foreground/60">{copy.appliesTo(profile ?? 'default')}</p>

      <div className="sticky top-0 z-10 flex items-center gap-3 bg-(--ui-chat-surface-background) py-2">
        <Input
          aria-label={copy.search}
          onChange={event => setQuery(event.target.value)}
          placeholder={copy.search}
          value={query}
        />
        <span className="shrink-0 text-[0.7rem] tabular-nums text-muted-foreground">
          {copy.count(shown.length, leaves.length)}
        </span>
      </div>

      {shown.length === 0 ? (
        <p className="py-6 text-center text-[0.8125rem] text-muted-foreground">{copy.empty}</p>
      ) : (
        groups.map(([section, list]) => (
          <section className="pb-4" key={section}>
            <SectionHeading icon={FolderOpen} title={section} />
            {list.map(leaf => (
              <ListRow
                action={<LeafEditor leaf={leaf} onCommit={value => void save(leaf, value)} />}
                hint={leaf.kind}
                key={JSON.stringify([editorScope, leaf.segments, leaf.value])}
                title={<span className="font-mono text-[0.78rem]">{leaf.path}</span>}
              />
            ))}
          </section>
        ))
      )}
    </SettingsContent>
  )
}
