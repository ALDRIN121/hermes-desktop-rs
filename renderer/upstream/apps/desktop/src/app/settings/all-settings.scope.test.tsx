import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { atom, type WritableAtom } from 'nanostores'
import { afterEach, expect, it, vi } from 'vitest'

const fixture = vi.hoisted(() => ({
  record: { data: { limit: 1 }, writeScope: { connectionId: 'gateway-a', profile: 'same' } },
  save: vi.fn(async () => ({ ok: true })),
  invalidate: vi.fn(async () => undefined)
}))

vi.mock('@/api/config', () => ({ saveHermesConfigRecord: fixture.save }))
vi.mock('@/app/hooks/use-config-record', () => ({
  useHermesConfigRecord: () => fixture.record,
  hermesConfigKey: (_profile: unknown, connection: string) => ['fixture-config', connection]
}))
vi.mock('@/store/settings-scope', () => ({ $settingsRequestProfile: atom('same') }))
vi.mock('@/store/connections', () => ({ $activeConnectionId: atom('gateway-a') }))
vi.mock('@/lib/query-client', () => ({ queryClient: { invalidateQueries: fixture.invalidate } }))
vi.mock('@/store/notifications', () => ({ notify: vi.fn(), notifyError: vi.fn() }))

import { $activeConnectionId } from '@/store/connections'

import { AllSettings } from './all-settings'

// This test replaces the production computed read-only store with a writable fixture.
const connection = $activeConnectionId as WritableAtom<string | null>

afterEach(() => {
  cleanup()
  fixture.save.mockClear()
  fixture.invalidate.mockClear()
})

it('writes using the captured read owner rather than a bare profile name', async () => {
  fixture.record = { data: { limit: 1 }, writeScope: { connectionId: 'gateway-a', profile: 'same' } }
  render(<AllSettings />)
  fireEvent.change(screen.getByLabelText('limit'), { target: { value: '2' } })
  fireEvent.blur(screen.getByLabelText('limit'))
  await waitFor(() =>
    expect(fixture.save).toHaveBeenCalledWith({ limit: 2 }, { connectionId: 'gateway-a', profile: 'same' })
  )
})

it('does not carry a gateway A field draft into the same-named profile on B', () => {
  fixture.record = { data: { limit: 1 }, writeScope: { connectionId: 'gateway-a', profile: 'same' } }
  const view = render(<AllSettings />)
  fireEvent.change(screen.getByLabelText('limit'), { target: { value: '99' } })
  fixture.record = { data: { limit: 7 }, writeScope: { connectionId: 'gateway-b', profile: 'same' } }
  act(() => connection.set('gateway-b'))
  view.rerender(<AllSettings />)
  expect((screen.getByLabelText('limit') as HTMLInputElement).value).toBe('7')
  expect(fixture.save).not.toHaveBeenCalled()
})

it('finishes an A save against the A cache after the visible owner switches to B', async () => {
  const pending = Promise.withResolvers<{ ok: boolean }>()
  fixture.save.mockReturnValueOnce(pending.promise)
  fixture.record = { data: { limit: 1 }, writeScope: { connectionId: 'gateway-a', profile: 'same' } }
  act(() => connection.set('gateway-a'))
  const view = render(<AllSettings />)
  fireEvent.change(screen.getByLabelText('limit'), { target: { value: '2' } })
  fireEvent.blur(screen.getByLabelText('limit'))
  fixture.record = { data: { limit: 7 }, writeScope: { connectionId: 'gateway-b', profile: 'same' } }
  act(() => connection.set('gateway-b'))
  view.rerender(<AllSettings />)
  await act(async () => {
    pending.resolve({ ok: true })
    await pending.promise
  })
  expect(fixture.invalidate).toHaveBeenCalledWith({ queryKey: ['fixture-config', 'gateway-a'] })
  expect((screen.getByLabelText('limit') as HTMLInputElement).value).toBe('7')
})
