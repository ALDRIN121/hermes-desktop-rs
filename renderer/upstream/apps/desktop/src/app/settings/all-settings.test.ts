import { expect, it } from 'vitest'

import { filterLeaves, flattenConfig, parseLeafInput, patchForLeaf } from './all-settings'

const record = {
  display: { density: 'comfortable', language: 'en', show_tokens: true },
  model: { max_tokens: 4096, temperature: 0.7 },
  toolsets: ['terminal', 'files'],
  voice: { enabled: false, provider: null },
  workspace: {}
}

it('lists every leaf in yaml order, including empty sections and arrays', () => {
  const leaves = flattenConfig(record)

  expect(leaves.map(leaf => leaf.path)).toEqual([
    'display.density',
    'display.language',
    'display.show_tokens',
    'model.max_tokens',
    'model.temperature',
    'toolsets',
    'voice.enabled',
    'voice.provider',
    'workspace'
  ])
  expect(leaves.find(leaf => leaf.path === 'toolsets')?.kind).toBe('array')
  expect(leaves.find(leaf => leaf.path === 'voice.provider')?.kind).toBe('null')
  expect(leaves.find(leaf => leaf.path === 'workspace')?.kind).toBe('object')
})

it('keeps the top-level section for grouping', () => {
  const sections = new Set(flattenConfig(record).map(leaf => leaf.section))

  expect(sections).toEqual(new Set(['display', 'model', 'toolsets', 'voice', 'workspace']))
})

it('searches key paths and values, and an empty query keeps everything', () => {
  const leaves = flattenConfig(record)

  expect(filterLeaves(leaves, '')).toHaveLength(leaves.length)
  expect(filterLeaves(leaves, 'MAX_TOKEN').map(leaf => leaf.path)).toEqual(['model.max_tokens'])
  expect(filterLeaves(leaves, 'terminal').map(leaf => leaf.path)).toEqual(['toolsets'])
  expect(filterLeaves(leaves, 'nothing-here')).toEqual([])
})

it('preserves literal dots in dynamic provider and connector keys', () => {
  const leaf = flattenConfig({ providers: { 'my.local': { limit: 2 } } })[0]!
  expect(patchForLeaf(leaf, 3)).toEqual({ providers: { 'my.local': { limit: 3 } } })
})

it('rejects non-finite numbers and accidental JSON type changes', () => {
  const leaf = (value: unknown) => flattenConfig({ value })[0]!

  for (const text of ['Infinity', '-Infinity', '1e999', '']) {
    expect(() => parseLeafInput(leaf(1), text)).toThrow()
  }

  expect(() => parseLeafInput(leaf([]), '{}')).toThrow()
  expect(() => parseLeafInput(leaf({}), '[]')).toThrow()
  expect(parseLeafInput(leaf(null), 'null')).toBeNull()
  expect(parseLeafInput(leaf(null), '"provider"')).toBe('provider')
  expect(() => parseLeafInput(leaf(null), '1e999')).toThrow()
})

it('does not match secrets through value search', () => {
  const leaves = flattenConfig({ provider: { api_key: 'sensitive-value', model: 'searchable' } })
  expect(filterLeaves(leaves, 'sensitive-value')).toEqual([])
  expect(filterLeaves(leaves, 'api_key')).toHaveLength(1)
  expect(filterLeaves(leaves, 'searchable')).toHaveLength(1)
})
