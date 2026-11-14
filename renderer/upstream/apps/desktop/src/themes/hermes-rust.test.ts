import { afterEach, expect, it, vi } from 'vitest'

afterEach(() => { Reflect.deleteProperty(window, '__hermesRustShell'); vi.resetModules() })

it('uses the agreed indigo palette for the Rust host without removing existing themes', async () => {
  Object.assign(window, { __hermesRustShell: true })
  const { BUILTIN_THEMES, DEFAULT_SKIN_NAME } = await import('./presets')
  expect(DEFAULT_SKIN_NAME).toBe('hermes-rust')
  expect(BUILTIN_THEMES['hermes-rust'].darkColors?.background).toBe('#10141C')
  expect(BUILTIN_THEMES['hermes-rust'].darkColors?.primary).toBe('#7878F5')
  expect(BUILTIN_THEMES['hermes-rust'].typography?.baseSize).toBe('16px')
  expect(BUILTIN_THEMES.nous).toBeDefined()
})

it('keeps the original default when running outside the Rust host', async () => {
  const { DEFAULT_SKIN_NAME } = await import('./presets')
  expect(DEFAULT_SKIN_NAME).toBe('nous')
})
