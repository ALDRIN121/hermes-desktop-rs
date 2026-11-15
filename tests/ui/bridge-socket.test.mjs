import { readFileSync } from 'node:fs'
import vm from 'node:vm'
import test from 'node:test'
import assert from 'node:assert/strict'

const source = readFileSync(new URL('../../src-tauri/src/bridge.js', import.meta.url), 'utf8')
const settle = () => new Promise(resolve => setImmediate(resolve))

function harness() {
  const listeners = new Map(), calls = [], pending = [], timers = []
  class CloseEvent extends Event { constructor(type, init) { super(type); Object.assign(this, init) } }
  class MessageEvent extends Event { constructor(type, init) { super(type); Object.assign(this, init) } }
  const window = { addEventListener() {}, WebSocket: class {}, __TAURI__: {
    core: { invoke(command, args) {
      calls.push({ command, args })
      if (command === 'hermes_ws_open') return new Promise(resolve => pending.push(resolve))
      return Promise.resolve()
    } },
    event: { listen(name, callback) {
      const group = listeners.get(name) ?? new Set(); group.add(callback); listeners.set(name, group)
      return Promise.resolve(() => group.delete(callback))
    } }
  } }
  const context = { window, EventTarget, Event, CloseEvent, MessageEvent, Blob,
    ArrayBuffer, Uint8Array, URL, console: { ...console }, setTimeout(callback) { timers.push(callback) },
    document: { addEventListener() {} }, location: { href: 'tauri://localhost' } }
  vm.runInNewContext(source.replace('__HERMES_BOOTSTRAP__', '{}'), context)
  return { window, calls, listeners, pending, timers,
    emit(name, payload) { for (const callback of listeners.get(name) ?? []) callback({ payload }) } }
}

test('open is dispatched only after listeners can receive the first frame', async () => {
  const h = harness(), socket = new h.window.WebSocket('ws://localhost:1234')
  await settle()
  const data = []
  socket.onmessage = event => data.push(event.data)
  socket.onopen = () => h.emit('hermes-ws-message', { id: 1, data: 'first' })
  h.pending[0](1); await settle()
  assert.deepEqual(data, ['first'])
})

test('close during connecting cannot reopen and closes the late native handle once', async () => {
  const h = harness(), socket = new h.window.WebSocket('ws://localhost:1234')
  const events = []; socket.onopen = () => events.push('open'); socket.onclose = () => events.push('close')
  await settle(); socket.close(); h.pending[0](1); await settle()
  assert.equal(socket.readyState, 3)
  assert.deepEqual(events, ['close'])
  assert.equal(h.calls.filter(x => x.command === 'hermes_ws_close').length, 1)
  assert.equal([...h.listeners.values()].reduce((n, set) => n + set.size, 0), 0)
})

test('binary sends preserve the view bytes and inbound arraybuffer bytes', async () => {
  const h = harness(), socket = new h.window.WebSocket('ws://localhost:1234')
  socket.binaryType = 'arraybuffer'; await settle(); h.pending[0](1); await settle()
  socket.send(new Uint8Array([9, 0, 128, 255, 9]).subarray(1, 4)); await settle()
  const sent = h.calls.find(x => x.command === 'hermes_ws_send')
  assert.deepEqual(Array.from(sent.args.data), [0, 128, 255])
  let received; socket.onmessage = event => { received = event.data }
  h.emit('hermes-ws-message', { id: 1, data: [0, 128, 255], binary: true })
  assert.ok(received instanceof ArrayBuffer)
  assert.deepEqual(Array.from(new Uint8Array(received)), [0, 128, 255])
})

test('peer close retires subscriptions and emits only one close', async () => {
  const h = harness(), socket = new h.window.WebSocket('ws://localhost:1234')
  await settle(); h.pending[0](1); await settle()
  let closed = 0; socket.onclose = () => closed++
  h.emit('hermes-ws-closed', { id: 1, code: 1000, reason: '' }); socket.close()
  assert.equal(closed, 1)
  assert.equal([...h.listeners.values()].reduce((n, set) => n + set.size, 0), 0)
})

test('native settings receives canonical zoom onChanged events and cleans the subscription', async () => {
  const h = harness(), changes = []
  const stop = h.window.hermesDesktop.zoom.onChanged(value => changes.push(value.percent))
  await settle()
  h.emit('hermes:zoom:changed', { percent: 125, level: 1 })
  assert.deepEqual(changes, [125])
  stop()
  h.emit('hermes:zoom:changed', { percent: 100, level: 0 })
  assert.deepEqual(changes, [125])
})

test('ordinary app launches never schedule automatic transcript capture', () => {
  assert.equal(harness().timers.length, 0)
})

test('the shell provides requestIdleCallback where WebKit omits it', async () => {
  const h = harness(), fired = []
  assert.equal(typeof h.window.requestIdleCallback, 'function')
  assert.equal(typeof h.window.cancelIdleCallback, 'function')
  const cancelled = h.window.requestIdleCallback(() => fired.push('cancelled'))
  h.window.requestIdleCallback(deadline => fired.push(deadline.didTimeout === false ? 'ran' : 'timeout'))
  h.window.cancelIdleCallback(cancelled)
  // Harness timers are captured, not run: deliver them the way a macrotask would.
  for (const callback of h.timers) callback()
  assert.deepEqual(fired, ['ran'])
})

test('the desktop surface the renderer calls is present and routes to its channel', async () => {
  const h = harness(), desktop = h.window.hermesDesktop
  // Members whose absence silently disabled a feature: a link that would not
  // open, a locale probe that never answered, media with no save path.
  for (const member of ['openExternal', 'getMachineProfile', 'sshResolveHost',
    'saveGatewayFile', 'saveImageFromUrl', 'normalizePreviewTarget', 'watchPreviewFile',
    'repairBootstrap']) {
    assert.equal(typeof desktop[member], 'function', `missing ${member}`)
  }
  for (const member of ['getDefaultProjectDir', 'setDefaultProjectDir', 'pickDefaultProjectDir']) {
    assert.equal(typeof desktop.settings[member], 'function', `missing settings.${member}`)
  }
  assert.equal(typeof desktop.themes.fetchMarketplace, 'function')
  assert.equal(typeof desktop.preparedSubmissions.read, 'function')

  // Open External must forward the url verbatim, not a rebuilt one.
  await desktop.openExternal('https://example.com/a?b=c')
  const open = h.calls.find(x => x.args.channel === 'hermes:openExternal')
  assert.equal(open.args.args[0], 'https://example.com/a?b=c')
})
