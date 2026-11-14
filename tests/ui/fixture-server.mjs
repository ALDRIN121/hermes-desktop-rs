// Read-only, local visual-review host for the actual production renderer.
// It never reads HERMES_HOME, invokes tools, or contacts a provider.
import http from 'node:http'
import { createRequire } from 'node:module'
import { readFileSync, existsSync, mkdtempSync, chmodSync, rmSync } from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import net from 'node:net'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const output = path.join(root, 'renderer/dist')
const nativeHome = process.argv.includes('--native') ? mkdtempSync(path.join(os.tmpdir(), 'hermes-ui-fixture-')) : null
const require = createRequire(path.join(root, 'renderer/upstream/apps/desktop/package.json'))
const { WebSocketServer } = require('ws')
const platform = (id, name, description, enabled, configured, state, env) => ({
  configured, description, docs_url: 'https://hermes.nousresearch.com/docs', enabled,
  env_vars: env.map(([key, prompt]) => ({ advanced: false, description: prompt, is_password: true,
    is_set: configured, key, prompt, redacted_value: configured ? 'fixture••••' : null, required: true })),
  gateway_running: false, id, name, state, updated_at: null
})
const platforms = [
  platform('telegram', 'Telegram', 'Chat with Hermes from a Telegram bot.', true, true, 'connected',
    [['TELEGRAM_BOT_TOKEN', 'Bot token from @BotFather']]),
  platform('discord', 'Discord', 'Add Hermes to a Discord server.', false, true, 'stopped',
    [['DISCORD_BOT_TOKEN', 'Bot token from the Discord developer portal']]),
  platform('slack', 'Slack', 'Mention Hermes in a Slack workspace.', false, false, null,
    [['SLACK_BOT_TOKEN', 'Bot token from your Slack app'], ['SLACK_APP_TOKEN', 'App-level token for socket mode']])
]
const config = {
  model: { provider: 'fixture', default: 'Review model' },
  display: { theme: 'hermes-rust', language: 'en', personality: '' },
  voice: { voice_chat_mode: 'chained', auto_tts: false, max_recording_seconds: 120, client_direct: false },
  stt: { provider: 'openai', model: 'whisper-1' },
  tts: { provider: 'edge', edge: { voice: 'en-US-AriaNeural' } },
  memory: { memory_enabled: true, user_profile_enabled: true },
  agent: { max_turns: 90 }, terminal: { cwd: '/fixture/workspace' },
  checkpoints: { enabled: false }
}
const fields = Object.fromEntries([
  ['voice.voice_chat_mode', { type: 'select', options: ['chained', 'gpt_live'], description: 'Choose standard speech or live conversation.' }],
  ['voice.auto_tts', { type: 'boolean' }], ['voice.max_recording_seconds', { type: 'number' }],
  ['voice.client_direct', { type: 'boolean' }], ['stt.provider', { type: 'select', options: ['openai', 'local'] }],
  ['stt.model', { type: 'string' }], ['tts.provider', { type: 'select', options: ['edge', 'openai'] }],
  ['tts.edge.voice', { type: 'string' }], ['agent.max_turns', { type: 'number' }],
  ['checkpoints.enabled', { type: 'boolean' }]
])
const emptySessions = { sessions: [], total: 0, offset: 0, limit: 100, profile_totals: {}, errors: [], groups: [] }
const responses = {
  '/api/config': config, '/api/config/defaults': config,
  '/api/config/schema': { fields }, '/api/env': {},
  '/api/health': { ok: true, status: 'ok' },
  '/api/fs/default-cwd': { cwd: '/fixture/workspace' },
  '/api/desktop-plugins/root': { path: '/fixture/plugins', files: [] },
  '/api/ui-preferences': { values: {}, revision: 0 },
  '/api/hermes/update/check': { update_available: false, current_version: 'visual-fixture' },
  '/api/audio/voice-live/status': { available: false, configured: false, reason: 'Visual fixture' },
  '/api/tools/terminal/backends': { backends: [], current: 'local' },
  '/api/profiles': { profiles: [{ name: 'default', description: 'Personal', is_active: true, is_default: true, path: '/fixture' }] },
  '/api/profiles/active': { profile: 'default', name: 'default' },
  '/api/sessions': emptySessions, '/api/profiles/sessions': emptySessions, '/api/profiles/sessions/sidebar': emptySessions,
  '/api/model/info': { model: 'Review model', provider: 'fixture' },
  '/api/model/options': { models: [], providers: [] }, '/api/model/auxiliary': { tasks: [], providers: [] },
  '/api/providers/oauth': { providers: [] }, '/api/providers/custom-endpoints': { endpoints: [] },
  '/api/cron/jobs': [], '/api/cron/blueprints': { blueprints: [] }, '/api/cron/delivery-targets': { targets: [] },
  '/api/skills': [
    { category: 'Research', description: 'Watch named companies for material news and produce cited digests.', enabled: true, name: 'competitor-news-monitor', usage: 3 },
    { category: 'Productivity', description: 'Turn meeting notes into cited decisions, owners and tickets.', enabled: true, name: 'meeting-action-items', usage: 1 },
    { category: 'Software', description: 'GitHub via gh CLI: PRs, issues, reviews, repos, auth.', enabled: false, name: 'github' }
  ],
  '/api/tools/toolsets': [
    { configured: true, description: 'Filesystem read/write tools, scoped to the session workspace.', enabled: true, label: 'Files', name: 'files', tools: ['read_file', 'write_file', 'search_files'] },
    { configured: true, description: 'Run commands in a local or remote shell.', enabled: true, label: 'Terminal', name: 'terminal', tools: ['terminal', 'process_manage'] }
  ],
  '/api/toolsets': [], '/api/plugins': [],
  '/api/mcp/servers': { servers: [] }, '/api/artifacts': { artifacts: [], total: 0 },
  '/api/messaging/platforms': { platforms }, '/api/pairing': { approved: [], pending: [] },
  '/api/skills/hub/official': { skills: [] }, '/api/skills/hub/sources': { sources: [] },
  '/api/git/status': { branch: 'main' }, '/api/git/worktrees': { worktrees: [] },
  '/api/audio/elevenlabs/voices': { available: false, voices: [] },
  '/api/status': { active_sessions: 0, gateway_running: true, gateway_state: 'running', gateway_platforms: {},
    gateway_pid: null, gateway_exit_reason: null, gateway_health_url: null, gateway_updated_at: null,
    hermes_home: '/fixture', config_path: '/fixture/config.yaml', env_path: '/fixture/.env', config_version: 1,
    latest_config_version: 1, release_date: '', version: 'visual-fixture' },
  '/api/webapp/preferences': { values: {}, revision: 0 },
  '/api/desktop/preferences': { values: {}, revision: 0 }
}
const json = (res, value, status = 200) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(value)) }
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2', '.json': 'application/json' }
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://fixture.invalid')
  if (url.pathname.startsWith('/api/')) {
    console.log(`${req.method} ${url.pathname}`)
    if (req.method === 'POST' && url.pathname === '/api/webapp/socket-ticket') return json(res, { ticket: 'fixture-only' })
    if (req.method !== 'GET') return json(res, { error: 'This visual fixture is read-only.' }, 405)
    if (Object.hasOwn(responses, url.pathname)) return json(res, responses[url.pathname])
    return json(res, { error: 'This endpoint is not supplied by the visual fixture.' }, 404)
  }
  const file = path.resolve(output, '.' + decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname))
  if (!file.startsWith(output + path.sep) || !existsSync(file)) { res.writeHead(404); res.end(); return }
  let bytes = readFileSync(file)
  if (file.endsWith('index.html')) bytes = Buffer.from(bytes.toString().replace('<head>', `<head><script>window.__HERMES_AUTH_REQUIRED__=true;window.__hermesRustShell=true;localStorage.setItem('hermes-theme-mode','dark');</script>`))
  res.writeHead(200, { 'content-type': mime[path.extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-store' })
  res.end(bytes)
})
const sockets = new WebSocketServer({ server, path: '/api/ws' })
sockets.on('connection', socket => socket.on('message', bytes => {
  const frame = JSON.parse(bytes.toString())
  console.log(`RPC ${frame.method}`)
  const results = { ping: { ok: true }, 'runtime.describe': { capabilities: [], session_create: { sources: ['desktop'], parameters: [] } },
    'config.get': { config, value: frame.params?.key === 'skin' ? 'hermes-rust' : '', cwd: '/fixture/workspace' },
    'setup.status': { provider_configured: true, provider: 'fixture', model: 'Review model' },
    'setup.runtime_check': { ok: true, provider: 'fixture', model: 'Review model' },
    'shared_metrics.status': { enabled: false, consent: false },
    'profiles.list': responses['/api/profiles'], 'session.active_list': { sessions: [] },
    'projects.tree': { projects: [] }, 'i18n.languages': { languages: [] }, 'i18n.catalog': { messages: {} },
    'session.list': { sessions: [] }, 'model.info': responses['/api/model/info'], 'process.list': { processes: [] },
    'subagent.list': { subagents: [] }, 'todo.list': { todos: [] } }
  socket.send(JSON.stringify(Object.hasOwn(results, frame.method)
    ? { jsonrpc: '2.0', id: frame.id, result: results[frame.method] }
    : { jsonrpc: '2.0', id: frame.id, error: { code: -32601, message: 'Unavailable in the read-only visual fixture' } }))
}))
let control
server.listen(Number(process.env.HERMES_FIXTURE_PORT ?? 0), '127.0.0.1', () => {
  const origin = `http://127.0.0.1:${server.address().port}`
  console.log(`HERMES_FIXTURE_URL=${origin}`)
  if (nativeHome) {
    control = net.createServer(socket => {
      let buffer = ''
      socket.on('data', bytes => {
        buffer += bytes.toString()
        if (!buffer.includes('\n')) return
        const request = JSON.parse(buffer.slice(0, buffer.indexOf('\n')))
        const result = request.verb === 'resolve-profile' ? {
          resolve_protocol: 1, state: 'ready', attachable: true, update_fenced: false,
          endpoint: { runtime_protocol: 1, api_origin: origin, ws_url: origin.replace('http:', 'ws:') + '/api/ws', profile_id: nativeHome, control_home: nativeHome, instance_id: 'native-ui-fixture' }
        } : request.verb === 'session-ticket' ? { ticket: 'fixture-only' } : null
        socket.end(JSON.stringify({ protocol: 1, id: request.id, ok: result !== null, result }) + '\n')
      })
    })
    control.listen(path.join(nativeHome, 'gateway.sock'), () => {
      chmodSync(path.join(nativeHome, 'gateway.sock'), 0o600)
      console.log(`HERMES_FIXTURE_HOME=${nativeHome}`)
    })
  }
})
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => {
  for (const socket of sockets.clients) socket.terminate()
  control?.close()
  sockets.close(); server.close(() => { if (nativeHome) rmSync(nativeHome, { recursive: true, force: true }); process.exit(0) })
})
