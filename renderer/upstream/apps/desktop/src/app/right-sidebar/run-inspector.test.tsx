import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'

import type { ChatMessage } from '@/lib/chat-messages'
import { $processEvidence, processEvidenceKey, recordProcessSnapshot } from '@/store/process-evidence'

import { executionEvidence, mergeProcessEvidence } from './run-evidence'
import { RunInspectorView } from './run-inspector'

afterEach(cleanup)

it('updates from a running tool to the real output of a failing test process', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'hermes-inspector-'))

  try {
    const file = path.join(dir, 'sample.test.cjs')
    writeFileSync(file, "const t=require('node:test'); const a=require('node:assert/strict'); t('passes',()=>a.equal(1,1)); t('fails',()=>a.equal(1,2));")

    const message: ChatMessage = { id: 'm1', role: 'assistant', parts: [{
      type: 'tool-call', toolCallId: 'call1', toolName: 'terminal', args: { command: 'node --test' }, argsText: '{}'
    }] }

    const view = render(<RunInspectorView current rows={executionEvidence([message], true)} subagents={[]} />)
    expect(screen.getByText('Running tests')).toBeDefined()
    const result = spawnSync(process.execPath, ['--test', file], { encoding: 'utf8' })
    expect(result.status).toBe(1)
    const part = message.parts[0]

    if (part.type !== 'tool-call') {throw new Error('fixture')}
    const completed: ChatMessage = { ...message, parts: [{ ...part, result: { output: result.stdout, exit_code: result.status } }] }
    view.rerender(<RunInspectorView current rows={executionEvidence([completed], false)} subagents={[]} />)
    expect(screen.getByText('Tests failed')).toBeDefined()
    expect(screen.getByText('1 passed · 1 failed · 0 skipped')).toBeDefined()
    expect(screen.queryByText('Running tests')).toBeNull()
  } finally { rmSync(dir, { recursive: true, force: true }) }
})

it('does not claim a new session ran tests', () => {
  render(<RunInspectorView current={false} rows={[]} subagents={[]} />)
  expect(screen.getByText('No execution evidence in this session.')).toBeDefined()
  expect(screen.queryByText('Tests passed')).toBeNull()
})

it('updates a background launch after its real failing process exits, even after the parent turn is idle', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'hermes-background-inspector-'))

  try {
    const file = path.join(dir, 'background.test.cjs')
    writeFileSync(file, "require('node:test')('fails',()=>{throw Error('fixture failure')})")
    const scope = processEvidenceKey('runtime', { connectionId: 'fixture', profile: 'default' })!
    const message: ChatMessage = { id: 'background', role: 'assistant', parts: [{ type: 'tool-call', toolCallId: 'launch', toolName: 'terminal', args: { command: 'node --test' }, argsText: '{}', result: { status: 'yielded_to_background', session_id: 'process' } }] }
    const launch = executionEvidence([message], false)
    const view = render(<RunInspectorView current={false} rows={launch} subagents={[]} />)
    expect(screen.getByText('Background process · awaiting exit')).toBeDefined()
    const result = spawnSync(process.execPath, ['--test', file], { encoding: 'utf8' })
    expect(result.status).toBe(1)
    recordProcessSnapshot(scope, [{ session_id: 'process', status: 'exited', exit_code: result.status!, output_tail: result.stdout }])
    view.rerender(<RunInspectorView current={false} rows={mergeProcessEvidence(launch, $processEvidence.get()[scope])} subagents={[]} />)
    expect(screen.getByText('Command failed')).toBeDefined()
    expect(screen.getByText('exit 1')).toBeDefined()
    expect(screen.getByText('Test counts unavailable')).toBeDefined()
    expect(screen.queryByText('Tests passed')).toBeNull()
    expect(screen.queryByText('Background process · awaiting exit')).toBeNull()
  } finally { rmSync(dir, { recursive: true, force: true }); $processEvidence.set({}) }
})
