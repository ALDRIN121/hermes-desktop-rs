import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { afterEach, expect, it, vi } from 'vitest'

import { VoiceSetup } from './voice-setup'

afterEach(() => { cleanup(); vi.unstubAllGlobals() })

const show = (config: Record<string, unknown>, onChange = vi.fn()) => render(<MemoryRouter><VoiceSetup config={config} onChange={onChange} schema={{}} /></MemoryRouter>)

it('presents standard/live conversation without changing an unknown existing value on load', () => {
  const change = vi.fn()
  show({ voice: { voice_chat_mode: 'future-mode' }, untouched: { provider: 'custom' } }, change)
  expect(change).not.toHaveBeenCalled()
  expect(screen.getByText(/future-mode/)).toBeTruthy()
  fireEvent.click(screen.getByRole('radio', { name: /Standard/ }))
  expect(change).toHaveBeenCalledWith(expect.objectContaining({ voice: { voice_chat_mode: 'chained' }, untouched: { provider: 'custom' } }))
  expect(screen.getByText('Not tested')).toBeTruthy()
})

it('releases a microphone that resolves after the settings scope unmounts', async () => {
  let resolve!: (stream: MediaStream) => void
  const stop = vi.fn()
  vi.stubGlobal('navigator', { mediaDevices: { getUserMedia: vi.fn(() => new Promise<MediaStream>(done => { resolve = done })) } })
  const view = show({})
  fireEvent.click(screen.getByRole('button', { name: 'Test microphone…' }))
  expect(screen.getByRole('button', { name: 'Cancel microphone test' })).toBeTruthy()
  view.unmount()
  await act(async () => { resolve({ getTracks: () => [{ stop }] } as unknown as MediaStream); await Promise.resolve() })
  expect(stop).toHaveBeenCalledOnce()
})

it('reports missing microphone support without claiming a successful test', () => {
  vi.stubGlobal('navigator', { mediaDevices: undefined })
  show({})
  fireEvent.click(screen.getByRole('button', { name: 'Test microphone…' }))
  expect(screen.getByText(/Microphone capture is unavailable/)).toBeTruthy()
  expect(screen.queryByText('Audio detected')).toBeNull()
})
