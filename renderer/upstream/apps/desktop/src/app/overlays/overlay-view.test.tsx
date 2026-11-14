import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'

import { OverlayView } from './overlay-view'

afterEach(() => { cleanup(); delete (window as Window & { __hermesRustShell?: boolean }).__hermesRustShell })

it('docks a Rust destination beside the actual global navigation', () => {
  (window as Window & { __hermesRustShell?: boolean }).__hermesRustShell = true
  const sidebar = document.createElement('aside')
  sidebar.dataset.tour = 'sessions-sidebar'
  sidebar.getBoundingClientRect = () => ({ x: 0, y: 40, width: 260, height: 800, top: 40, bottom: 840, left: 0, right: 260, toJSON: () => ({}) })
  document.body.append(sidebar)

  try {
    render(<OverlayView onClose={vi.fn()}><h1>Settings fixture</h1></OverlayView>)
    const page = screen.getByRole('heading', { name: 'Settings fixture' }).closest('[data-overlay-surface]') as HTMLElement
    expect(page.dataset.hermesPage).toBe('docked')
    expect(page.style.getPropertyValue('--hermes-page-left')).toBe('260px')
    expect(page.style.getPropertyValue('--hermes-page-right')).toBe('0px')
  } finally { sidebar.remove() }
})

it('retains the original floating layout for other hosts', () => {
  render(<OverlayView onClose={vi.fn()}><h1>Settings fixture</h1></OverlayView>)
  expect(screen.getByRole('heading').closest('[data-overlay-surface]')?.getAttribute('data-hermes-page')).toBeNull()
})
