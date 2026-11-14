import { expect, it } from 'vitest'

import type { ChatMessage } from '@/lib/chat-messages'

import { executionEvidence, mergeProcessEvidence } from './run-evidence'

const message = (id: string, command: string, result?: unknown): ChatMessage => ({
  id, role: 'assistant', parts: [{ type: 'tool-call', toolCallId: id, toolName: 'terminal',
    args: { command }, argsText: JSON.stringify({ command }),
    ...(result === undefined ? {} : { result }) }]
})

it('shows actual Node failure counts from this execution, never user prose', () => {
  const user: ChatMessage = { id: 'user', role: 'user', parts: [{ type: 'text', text: '99 passed in 1s' }] }

  const rows = executionEvidence([user, message('test', 'node --test', {
    output: '# tests 2\n# pass 1\n# fail 1\n# skipped 0', exit_code: 1
  })], false)

  expect(rows).toHaveLength(1)
  expect(rows[0]).toMatchObject({ id: 'test', state: 'failed', tests: { passed: 1, failed: 1, skipped: 0 } })
})

it('keeps failed-only pytest and Rust output failed without historical maxima', () => {
  const rows = executionEvidence([
    message('old', 'pytest', { output: '99 passed in 1.2s', exit_code: 0 }),
    message('new', 'pytest', { output: '2 failed in 0.42s', exit_code: 1 }),
    message('rust', 'cargo test', { output: 'test result: FAILED. 0 passed; 2 failed; 0 ignored;', exit_code: 101 })
  ], false)

  expect(rows[1].tests).toMatchObject({ passed: 0, failed: 2 })
  expect(rows[2].tests).toMatchObject({ passed: 0, failed: 2 })
})

it('does not turn a background-launch receipt or missing exit into success', () => {
  const rows = executionEvidence([
    message('bg', 'pytest', { status: 'yielded_to_background', session_id: 'proc_1', output: '1 passed in 1s' }),
    message('lost', 'pytest', { output: '1 passed in 1s' }),
    message('wrapper', 'echo "99 passed in 1s"', { output: '99 passed in 1s', exit_code: 0 })
  ], false)

  expect(rows[0]).toMatchObject({ state: 'background', processId: 'proc_1', tests: null })
  expect(rows[1]).toMatchObject({ state: 'unknown', tests: null })
  expect(rows[2]).toMatchObject({ state: 'succeeded', tests: null })
})

it('only marks pending tools running while a turn is known live', () => {
  expect(executionEvidence([message('a', 'pytest')], true)[0].state).toBe('running')
  expect(executionEvidence([message('a', 'pytest')], false)[0].state).toBe('unknown')
})

it('uses a real turn anchor and deduplicates replayed tool IDs', () => {
  const rows = executionEvidence([
    message('old', 'pytest'), { id: 'anchor', role: 'user', parts: [] },
    message('new', 'pytest'), message('new', 'pytest', { output: '2 passed in 1s', exit_code: 0 })
  ], true, 'anchor')

  expect(rows).toHaveLength(1)
  expect(rows[0]).toMatchObject({ id: 'new', state: 'succeeded', tests: { passed: 2, failed: 0 } })
})

it('retains interruption and truncated-report uncertainty', () => {
  const interrupted = message('stopped', 'pytest'); interrupted.parts[0].interrupted = true

  const rows = executionEvidence([interrupted,
    message('truncated', 'pytest', { output: '8 passed in 1s', exit_code: 0, full_output_path: '/tmp/output' })
  ], false)

  expect(rows[0].state).toBe('cancelled')
  expect(rows[1].tests).toBe(null)
})

it('settles the original background launch when its owned process later fails, without parsing an output tail as a full report', () => {
  const launch = executionEvidence([message('bg', 'pytest', { status: 'yielded_to_background', session_id: 'proc_1' })], false)
  const rows = mergeProcessEvidence(launch, [{ id: 'proc_1', state: 'failed', command: 'pytest', exitCode: 1, output: '1 passed in 1s', source: 'process.list output tail' }])
  expect(rows).toHaveLength(1)
  expect(rows[0]).toMatchObject({ id: 'bg', processId: 'proc_1', state: 'failed', exitCode: 1, tests: null })
})
