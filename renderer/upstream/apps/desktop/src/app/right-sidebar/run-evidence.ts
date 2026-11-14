import type { ChatMessage } from '@/lib/chat-messages'
import type { ProcessEvidence } from '@/store/process-evidence'

// Built from a string so the ESC byte is a regex escape rather than a literal control character in source.
// eslint-disable-next-line no-control-regex -- matching the ESC control byte is the purpose of the pattern
const ANSI_ESCAPE = new RegExp('\\u001b\\[[0-?]*[ -/]*[@-~]', 'g')

export interface ExecutionEvidence {
  id: string
  tool: string
  command: string
  state: 'running' | 'succeeded' | 'failed' | 'unknown' | 'cancelled' | 'background'
  processId?: string
  exitCode: number | null
  output: string
  tests: { passed: number; failed: number; skipped: number; source: string } | null
  source?: string
}

/** Registry tails establish process exit, never complete test-report counts. */
export function mergeProcessEvidence(rows: ExecutionEvidence[], processes: readonly ProcessEvidence[]): ExecutionEvidence[] {
  const pending = new Map(processes.map(process => [process.id, process]))

  const merged = rows.map(row => {
    const process = row.processId ? pending.get(row.processId) : undefined

    if (!process) {return row}
    pending.delete(process.id)

    if (!['background', 'unknown'].includes(row.state)) {return row}

    return { ...row, command: row.command || process.command, state: process.state, exitCode: process.exitCode,
      output: process.output, tests: null, source: process.source }
  })

  for (const process of pending.values()) {merged.push({ id: `process:${process.id}`, processId: process.id,
    tool: 'process', command: process.command, state: process.state, exitCode: process.exitCode,
    output: process.output, tests: null, source: process.source })}

  return merged
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value === 'string') {
    try { value = JSON.parse(value) } catch { return {} }
  }

  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {}
}

function testSummary(command: string, output: string): ExecutionEvidence['tests'] {
  let summary: string | undefined
  let source: string

  if (/^(?:\S*\/)?cargo\s+(?:test|nextest\s+run)\b/.test(command)) {
    summary = output.match(/^test result:.*$/gm)?.at(-1)
    source = 'Rust last reported suite'
  } else if (/^(?:(?:\S*\/)?pytest\b|python[\d.]*\s+-m\s+pytest\b)/.test(command)) {
    summary = output.split('\n').filter(line => /\b\d+\s+(?:passed|failed|skipped|error)/.test(line) && /\bin\s+[\d.]+s/.test(line)).at(-1)
    source = 'pytest summary'
  } else if (/^(?:\S*\/)?node\s+--test\b/.test(command)) {
    const passed = output.match(/^[#ℹ] pass (\d+)\s*$/m)
    const failed = output.match(/^[#ℹ] fail (\d+)\s*$/m)

    if (!passed || !failed) {return null}

    return { passed: Number(passed[1]), failed: Number(failed[1]),
      skipped: Number(output.match(/^[#ℹ] skipped (\d+)\s*$/m)?.[1] ?? 0), source: 'Node test summary' }
  } else if (/^(?:(?:npx|pnpm exec)\s+)?jest\b/.test(command)) {
    summary = output.match(/^Tests:.*$/gm)?.at(-1)
    source = 'Jest summary'
  } else {return null}

  if (!summary) {return null}
  const counts = [...summary.matchAll(/(\d+)\s+(passed|failed|skipped|ignored|errors?)/g)]

  if (!counts.length) {return null}
  const values: Record<string, number> = Object.fromEntries(counts.map(match => [match[2], Number(match[1])]))

  return { passed: values.passed ?? 0, failed: (values.failed ?? 0) + (values.error ?? values.errors ?? 0),
    skipped: (values.skipped ?? 0) + (values.ignored ?? 0), source }
}

/** Read the gateway's reconciled tool parts, never assistant/user prose or display summaries. */
export function executionEvidence(messages: ChatMessage[], turnLive: boolean, anchorId?: string | null): ExecutionEvidence[] {
  const anchor = anchorId ? messages.findIndex(message => message.id === anchorId) : -1
  const rows = new Map<string, ExecutionEvidence>()

  for (const message of messages.slice(anchor + 1)) {
    if (message.role !== 'assistant') {continue}

    for (const part of message.parts) {
      if (part.type !== 'tool-call' || !part.toolCallId) {continue}
      const result = record(part.result)
      const args = record(part.args)
      const command = typeof args.command === 'string' ? args.command.trim() : ''
      const output = typeof result.output === 'string' ? result.output : ''
      const exitCode = typeof result.exit_code === 'number' && Number.isInteger(result.exit_code) ? result.exit_code : null
      const processId = typeof result.session_id === 'string' ? result.session_id : undefined
      const background = processId && ['running', 'yielded_to_background', 'background'].includes(String(result.status))
      const pending = !Object.hasOwn(part, 'result') && part.completedAt === undefined

      const state = part.interrupted ? 'cancelled' : background ? 'background' : exitCode !== null
        ? exitCode === 0 ? 'succeeded' : 'failed'
        : result.error || part.isError ? 'failed' : pending && turnLive ? 'running' : 'unknown'

      const truncated = Boolean(result.full_output_path || result.truncated || result.output_truncated)

      const tests = exitCode !== null && !background && !truncated && part.toolName === 'terminal'
        ? testSummary(command, output.replace(ANSI_ESCAPE, '')) : null

      const row: ExecutionEvidence = { id: part.toolCallId, tool: part.toolName, command,
        state: tests?.failed ? 'failed' : state, processId, exitCode, output, tests }

      // A replayed start never erases an already terminal result for this call.
      const previous = rows.get(row.id)

      if (!previous || !['running', 'unknown'].includes(row.state) || ['running', 'unknown'].includes(previous.state)) {
        rows.set(row.id, row)
      }
    }
  }

  return [...rows.values()]
}
