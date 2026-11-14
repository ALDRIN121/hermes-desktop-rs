import { atom } from 'nanostores'

export interface ProcessEvidence {
  id: string
  command: string
  state: 'background' | 'succeeded' | 'failed' | 'unknown'
  exitCode: number | null
  output: string
  source: string
}

export interface ProcessSnapshot {
  session_id?: string
  command?: string
  status?: string
  exit_code?: number
  output_tail?: string
}

// Recent observed evidence outlives the composer's four-second success toast.
// No secrets or process output are written to persistent browser storage.
export const $processEvidence = atom<Record<string, ProcessEvidence[]>>({})
const MAX_SCOPES = 24
const MAX_PROCESSES = 128
const MAX_OUTPUT = 12_000

/** Bare profile names are insufficient when two gateways expose that name. */
export function processEvidenceKey(runtime: string | null | undefined, owner: unknown): string | null {
  if (!runtime || !owner || typeof owner !== 'object') {return null}
  const route = owner as { connectionId?: unknown; profile?: unknown }

  return typeof route.connectionId === 'string' && route.connectionId && typeof route.profile === 'string' && route.profile
    ? JSON.stringify([route.connectionId, route.profile, runtime]) : null
}

export function clearProcessEvidence(scope: string | null) {
  if (!scope || !$processEvidence.get()[scope]) {return}
  const next = { ...$processEvidence.get() }; delete next[scope]; $processEvidence.set(next)
}

export function recordProcessSnapshot(scope: string, processes: readonly ProcessSnapshot[]) {
  const previous = $processEvidence.get()[scope] ?? []
  const rows = new Map(previous.map(row => [row.id, row]))

  for (const proc of processes) {
    if (!proc.session_id) {continue}
    const old = rows.get(proc.session_id)
    const exitCode = proc.status === 'exited' && typeof proc.exit_code === 'number' && Number.isInteger(proc.exit_code) ? proc.exit_code : null

    const state = proc.status === 'exited' ? exitCode === null ? 'unknown' : exitCode === 0 ? 'succeeded' : 'failed'
      : ['running', 'queued'].includes(proc.status ?? '') ? 'background' : 'unknown'

    // A delayed/replayed running snapshot cannot erase a confirmed exit.
    if (old?.exitCode !== null && old?.exitCode !== undefined && exitCode === null) {continue}

    const row: ProcessEvidence = { id: proc.session_id, command: proc.command || old?.command || '',
      state, exitCode, output: (proc.output_tail || old?.output || '').slice(-MAX_OUTPUT), source: 'process.list output tail' }

    rows.set(proc.session_id, old && old.command === row.command && old.state === row.state && old.exitCode === row.exitCode && old.output === row.output ? old : row)
  }

  const next = [...rows.values()].slice(-MAX_PROCESSES)

  if (next.length === previous.length && next.every((row, index) => row === previous[index])) {return}
  const scopes = new Map(Object.entries($processEvidence.get()))
  scopes.delete(scope); scopes.set(scope, next)
  $processEvidence.set(Object.fromEntries([...scopes].slice(-MAX_SCOPES)))
}
