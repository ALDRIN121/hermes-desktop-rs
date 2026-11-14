import { useStore } from '@nanostores/react'
import { useState } from 'react'

import { $processEvidence, processEvidenceKey } from '@/store/process-evidence'
import { $gatewayState } from '@/store/session'
import { $focusedRuntimeId, $focusedSessionState, knownOwnerForSession } from '@/store/session-states'
import { $subagentsBySession, type SubagentProgress } from '@/store/subagents'

import { executionEvidence, type ExecutionEvidence, mergeProcessEvidence } from './run-evidence'

export function RunInspector() {
  const state = useStore($focusedSessionState)
  const runtimeId = useStore($focusedRuntimeId)
  const agents = useStore($subagentsBySession)
  const gateway = useStore($gatewayState)
  const processes = useStore($processEvidence)
  const live = Boolean(state?.turnLive && gateway === 'open')
  const scope = processEvidenceKey(runtimeId, knownOwnerForSession(runtimeId))

  return <RunInspectorView current={live}
    rows={mergeProcessEvidence(executionEvidence(state?.messages ?? [], live), scope ? processes[scope] ?? [] : [])}
    stale={gateway !== 'open'} subagents={runtimeId ? agents[runtimeId] ?? [] : []} />
}

function executionLabel(row: ExecutionEvidence): string {
  if (row.state === 'running') {return /^(?:cargo\s+(?:test|nextest)|pytest\b|python[\d.]*\s+-m\s+pytest\b|node\s+--test\b|(?:npx\s+|pnpm\s+exec\s+)?jest\b)/.test(row.command)
    ? 'Running tests' : 'Running command'}

  if (row.state === 'background') {return 'Background process · awaiting exit'}

  if (row.state === 'unknown') {return 'Result unknown'}

  if (row.state === 'cancelled') {return 'Cancelled'}

  return row.tests ? row.state === 'failed' ? 'Tests failed' : 'Tests passed'
    : row.state === 'failed' ? 'Command failed' : 'Command succeeded'
}

export function RunInspectorView({ current, rows, subagents, stale = false }: {
  current: boolean; rows: ExecutionEvidence[]; subagents: SubagentProgress[]; stale?: boolean
}) {
  const [shown, setShown] = useState(20)

  return <section aria-label="Run details" className="flex min-h-0 flex-1 flex-col overflow-y-auto p-4 text-sm">
    <h2 className="mb-1 font-semibold">{current ? 'Live session activity' : 'Session activity'}</h2>
    <p className="mb-4 text-xs text-muted-foreground">Recent execution evidence from the selected session</p>
    {stale && <p className="mb-4 text-amber-400" role="status">Disconnected · showing last known results</p>}
    {!rows.length && <p className="text-muted-foreground">No execution evidence in this session.</p>}
    {rows.length > shown && <button className="mb-3 rounded border px-3 py-2" onClick={() => setShown(shown + 20)} type="button">Show earlier executions ({rows.length - shown})</button>}
    <ol className="divide-y divide-border">
      {rows.slice(-shown).map(row => <li className="py-3" key={row.id}>
        <div className="flex items-center justify-between gap-2">
          <b className={row.state === 'failed' ? 'text-destructive' : ''}>{executionLabel(row)}</b>
          {row.exitCode !== null && <span className="text-xs text-muted-foreground">exit {row.exitCode}</span>}
        </div>
        <code className="mt-1 block overflow-x-auto text-xs">{row.command || row.tool}</code>
        {row.tests ? <>
          <p className="mt-2">{row.tests.passed} passed · {row.tests.failed} failed · {row.tests.skipped} skipped</p>
          <p className="text-xs text-muted-foreground">{row.tests.source}</p>
        </> : <p className="mt-2 text-xs text-muted-foreground">Test counts unavailable</p>}
        {row.processId && <p className="text-xs text-muted-foreground">Process: {row.processId}</p>}
        {row.source && <p className="text-xs text-muted-foreground">{row.source} · complete test report unavailable</p>}
        {!!row.output && <details className="mt-2">
          <summary className="cursor-pointer text-xs">View captured output</summary>
          {row.output.length > 20_000 && <p className="mt-2 text-xs text-muted-foreground">Showing the last 20,000 characters of captured output.</p>}
          <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-words text-xs">{row.output.slice(-20_000)}</pre>
        </details>}
      </li>)}
    </ol>
    <h3 className="mt-5 border-t pt-4 font-semibold">Subagents ({subagents.length})</h3>
    {subagents.length ? <ul className="divide-y divide-border">{subagents.map(agent => <li className="py-3" key={agent.id}>
      <div className="flex justify-between gap-2"><span>{agent.goal}</span><span className="text-xs text-muted-foreground">{agent.status}</span></div>
      {agent.currentTool && <code className="text-xs">{agent.currentTool}</code>}
      {agent.summary && <details className="mt-2"><summary className="cursor-pointer text-xs">View result</summary><p className="mt-2 whitespace-pre-wrap">{agent.summary}</p></details>}
    </li>)}</ul> : <p className="mt-2 text-xs text-muted-foreground">No delegated tasks recorded.</p>}
  </section>
}
