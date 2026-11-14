import { beforeEach, expect, it } from 'vitest'

import { $processEvidence, clearProcessEvidence, processEvidenceKey, recordProcessSnapshot } from './process-evidence'

beforeEach(() => $processEvidence.set({}))

it('retains a real terminal exit after ephemeral status rows disappear, without trusting a missing exit', () => {
  const scope = processEvidenceKey('r', { connectionId: 'local', profile: 'work' })!
  recordProcessSnapshot(scope, [{ session_id: 'p', status: 'running', command: 'pytest' }])
  recordProcessSnapshot(scope, [{ session_id: 'p', status: 'exited', exit_code: 1, output_tail: '2 failed in 1s' }])
  recordProcessSnapshot(scope, [])
  recordProcessSnapshot(scope, [{ session_id: 'p', status: 'running' }])
  expect($processEvidence.get()[scope][0]).toMatchObject({ id: 'p', exitCode: 1, state: 'failed', command: 'pytest' })
  recordProcessSnapshot(scope, [{ session_id: 'missing', status: 'exited' }])
  expect($processEvidence.get()[scope].find(row => row.id === 'missing')?.state).toBe('unknown')
})

it('separates identical runtime/process IDs on different gateway/profile owners', () => {
  const a = processEvidenceKey('r', { connectionId: 'remote-a', profile: 'same' })!
  const b = processEvidenceKey('r', { connectionId: 'remote-b', profile: 'same' })!
  expect(a).not.toBe(b)
  recordProcessSnapshot(a, [{ session_id: 'p', status: 'exited', exit_code: 1 }])
  recordProcessSnapshot(b, [{ session_id: 'p', status: 'running' }])
  expect($processEvidence.get()[b][0].state).toBe('background')
  clearProcessEvidence(a)
  expect($processEvidence.get()[a]).toBeUndefined()
  expect($processEvidence.get()[b]).toHaveLength(1)
  expect(processEvidenceKey('r', undefined)).toBeNull()
})
