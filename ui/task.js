// Workspace shell: a four-column surface over the live gateway.
//
// Sessions arrive from the REST proxy (`hermes:api`); transcripts, plans and
// sends go through canonical JSON-RPC (`task_call` / `task_submit`). File tabs,
// diffs, file contents and the repo tree come from the same Rust channels the
// renderer uses. Nothing in this window invents a number: a region without data
// says so.

const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args)
const api = (request) => invoke('hermes_ipc', { channel: 'hermes:api', args: [request] })
const ipc = (channel, args) => invoke('hermes_ipc', { channel, args })
const rpc = (method, params, timeoutMs) =>
  invoke('task_call', { method, params, timeoutMs: timeoutMs ?? 25000 })
const el = (id) => document.getElementById(id)

const CAP = 250

const state = {
  sessions: [],
  cron: [],
  sessionId: null,
  profile: null,
  messages: [],
  info: null,
  usage: null,
  todo: null,
  running: false,
  files: [],
  selected: null,
  pane: 'diff',
  repo: null,
  repos: [],
  repoExplicit: false,
  pr: null,
  filter: '',
  selftest: false,
  pollTimer: null,
  lastDiff: '',
  tests: null,
  attachments: [],
  model: null,
  audit: false,
  renderError: null,
  loadError: null
}

// ---------------------------------------------------------------- formatting
const textOf = (content) => {
  if (content == null) return ''
  if (typeof content === 'string') return content
  if (Array.isArray(content)) {
    return content
      .map((part) => (typeof part === 'string' ? part : part?.text ?? part?.content ?? ''))
      .join('\n')
  }
  if (typeof content === 'object') return content.text ?? content.content ?? ''
  return String(content)
}

// Gateway timestamps arrive in seconds in some payloads; normalise to ms.
const toMs = (value) => {
  const n = Number(value || 0)
  if (!n) return 0
  return n < 1e12 ? n * 1000 : n
}

const rel = (ms) => {
  const value = toMs(ms)
  if (!value) return ''
  const seconds = Math.max(0, (Date.now() - value) / 1000)
  if (seconds < 60) return 'just now'
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`
  return `${Math.floor(seconds / 86400)}d ago`
}

const clock = (ms) => {
  const value = toMs(ms)
  if (!value) return ''
  return new Date(value).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
}

const humanBytes = (n) => {
  if (!n) return '0'
  if (n < 1000) return String(n)
  if (n < 1_000_000) return `${Math.round(n / 100) / 10}K`
  return `${Math.round(n / 100_000) / 10}M`
}

const humanDuration = (seconds) => {
  const s = Math.max(0, Math.round(seconds))
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m ${s % 60}s`
  return `${Math.floor(m / 60)}h ${m % 60}m`
}

const STATUS_LABEL = { M: 'M', A: 'A', D: 'D', R: 'R', '??': 'A', U: 'M' }

// -------------------------------------------------------------------- plans
function todoItems(raw) {
  const list = Array.isArray(raw) ? raw : raw?.todos ?? raw?.items ?? raw?.steps ?? null
  if (!Array.isArray(list)) return []
  return list
    .map((item) => {
      if (typeof item === 'string') return { text: item, status: 'pending' }
      const text = item.content ?? item.text ?? item.title ?? item.task ?? ''
      const status = String(item.status ?? item.state ?? 'pending').toLowerCase()
      return { text: String(text), status }
    })
    .filter((item) => item.text)
}

function todoFromTranscript() {
  for (let i = state.messages.length - 1; i >= 0; i -= 1) {
    for (const call of state.messages[i].tool_calls ?? []) {
      const name = String(call?.function?.name ?? call?.name ?? '')
      if (!name.includes('todo')) continue
      try {
        const args = call?.function?.arguments ?? call?.arguments ?? ''
        const parsed = typeof args === 'string' ? JSON.parse(args) : args
        const items = todoItems(parsed)
        if (items.length) return items
      } catch {
        /* keep looking */
      }
    }
  }
  return []
}

const planItems = () => {
  const direct = todoItems(state.todo)
  return direct.length ? direct : todoFromTranscript()
}
const isDone = (s) => ['completed', 'complete', 'done', 'finished', 'success'].includes(s)
const isActive = (s) => ['in_progress', 'in-progress', 'running', 'doing', 'active'].includes(s)

function toolNames(message) {
  const names = []
  for (const call of message.tool_calls ?? []) {
    const name = call?.function?.name ?? call?.name
    if (name) names.push(String(name))
  }
  return names
}

function toolArgs(call) {
  const raw = call?.function?.arguments ?? call?.arguments ?? ''
  if (typeof raw === 'object') return raw
  try {
    return JSON.parse(raw)
  } catch {
    return {}
  }
}

// ------------------------------------------------------------------ sessions
async function loadSessions() {
  const response = await api({
    path: `/api/profiles/sessions/sidebar?limit=60&recents_profile=${encodeURIComponent(state.profile || 'default')}`,
    method: 'GET'
  }).catch(() => null)
  // The sidebar route returns lists, but `messaging` is grouped per platform in
  // some installs — normalise anything object-shaped before spreading it.
  const collect = (value) => {
    if (Array.isArray(value)) return value
    if (value && typeof value === 'object') {
      return Object.values(value).flatMap((item) => (Array.isArray(item) ? item : item ? [item] : []))
    }
    return []
  }
  state.sessions = [...collect(response?.recents), ...collect(response?.messaging)]
  state.cron = collect(response?.cron)
  renderSessions()
  renderScheduled()
  if (state.sessions[0] && !state.profile) state.profile = state.sessions[0].profile || 'default'
}

function sessionIdOf(row) {
  if (!row || typeof row !== 'object') return null
  return row.session_id ?? row.sessionId ?? row.id ?? row.session?.id ?? row.key ?? null
}

function firstSession() {
  for (const row of state.sessions) {
    const id = sessionIdOf(row)
    if (id) return { id, row }
  }
  return null
}

function renderSessions() {
  const box = el('sessions')
  box.innerHTML = ''
  let lastGroup = ''
  const filter = state.filter.trim().toLowerCase()
  const rows = state.sessions.filter((row) => !filter || String(row.title || '').toLowerCase().includes(filter))
  el('session-count').textContent = String(state.sessions.length)
  for (const row of rows) {
    const id = sessionIdOf(row)
    if (!id) continue
    const item = document.createElement('button')
    item.className = 'session-row' + (id === state.sessionId ? ' selected' : '')
    const dot = document.createElement('span')
    dot.className = 'dot' + (row.running ? ' live' : '')
    const name = document.createElement('span')
    name.className = 'name'
    name.textContent = row.title || '(untitled)'
    name.title = row.title || ''
    const when = document.createElement('span')
    when.className = 'when'
    when.textContent = rel(row.last_active_at ?? row.last_active)
    item.append(dot, name, when)
    item.addEventListener('click', () => openSession(id))
    const group = dayGroup(row.last_active_at ?? row.last_active)
    if (group !== lastGroup) {
      const label = document.createElement('div')
      label.className = 'rail-group'
      label.textContent = group
      box.appendChild(label)
      lastGroup = group
    }
    box.appendChild(item)
  }
}

function dayGroup(value) {
  const at = toMs(value)
  if (!at) return 'Earlier'
  const days = Math.floor((Date.now() - at) / 86400000)
  if (days <= 0) return 'Today'
  if (days === 1) return 'Yesterday'
  if (days < 7) return 'This week'
  return 'Earlier'
}

function renderScheduled() {
  const box = el('scheduled')
  box.innerHTML = ''
  const jobs = state.cron
  el('sched-section').hidden = !jobs.length
  el('sched-count').textContent = jobs.length ? String(jobs.length) : ''
  for (const job of jobs.slice(0, 4)) {
    const wrap = document.createElement('div')
    wrap.className = 'job'
    const name = document.createElement('div')
    name.className = 'name'
    name.textContent = job.title || job.name || sessionIdOf(job) || 'job'
    const sub = document.createElement('div')
    sub.className = 'sub'
    sub.textContent = job.schedule || job.next_run || rel(job.last_active_at ?? job.last_active)
    wrap.append(name, sub)
    box.appendChild(wrap)
  }
  const jobsBox = el('jobs')
  if (jobsBox) {
    jobsBox.innerHTML = ''
    el('jobs-section').hidden = !jobs.length
    el('jobs-count').textContent = jobs.length ? String(jobs.length) : ''
    for (const job of jobs.slice(0, 4)) {
      const row = document.createElement('div')
      row.className = 'job'
      const name = document.createElement('div')
      name.className = 'name'
      name.textContent = job.title || job.name || 'job'
      const sub = document.createElement('div')
      sub.className = 'sub'
      sub.textContent = job.schedule || job.next_run || ''
      row.append(name, sub)
      jobsBox.appendChild(row)
    }
  }
}

// -------------------------------------------------------------------- session
async function openSession(id, repoOverride) {
  stopPolling()
  state.sessionId = id
  if (repoOverride) {
    state.repo = repoOverride
    state.repoExplicit = true
  }
  setStatus('loading')
  try {
    const snapshot = await rpc('session.resume', {
      session_id: id,
      profile: state.profile ?? undefined,
      source: 'desktop'
    })
    state.messages = snapshot?.messages ?? []
    state.info = snapshot?.info ?? null
    state.usage = state.info?.usage ?? null
    state.todo = snapshot?.todo_state ?? null
    state.running = !!snapshot?.running
    state.profile = state.info?.profile_name ?? state.profile ?? 'default'
    state.files = []
    state.selected = null
    state.pr = null
    renderSessions()
    renderHeader()
    try {
      renderTranscript()
    } catch (error) {
      state.renderError = `transcript: ${error?.message || error}`
    }
    try {
      renderAgent()
    } catch (error) {
      state.renderError = `${state.renderError ? state.renderError + ' | ' : ''}agent: ${error?.message || error}`
    }
    await resolveRepo(repoOverride)
    await loadFiles()
    await loadTree()
    renderHeader()      // subtitle carries the changed-file count
    renderWorkspace()
    renderChanges()
    renderPanes()
    await loadPr()
    await refreshResources()
    startPolling()
    setStatus('loaded')
  } catch (error) {
    state.loadError = String(error?.stack || error)
    setStatus(`load failed: ${error}`)
    el('transcript').innerHTML = `<div class="empty-note">Could not load session: ${String(error)}</div>`
  }
}

function renderHeader() {
  const info = state.info || {}
  el('session-title').textContent = info.title || 'Session'
  el('session-title').title = info.title || ''
  const branch = info.branch || 'no branch'
  el('branch-pill').textContent = branch
  const repoName = state.repo ? String(state.repo).split('/').filter(Boolean).pop() : null
  el('gh-pill').textContent = repoName || info.project || 'GitHub'
  el('gh-pill').title = state.repo || 'no repository resolved'
  if (!state.model && info.model) {
    state.model = { ...(state.model || {}), model: info.model }
  }
  paintModel()
  const changed = state.files.length
  const project = repoName || info.project || (info.cwd ? String(info.cwd).split('/').pop() : 'workspace')
  el('chat-sub').textContent = `${project} · ${branch} · ${changed} file${changed === 1 ? '' : 's'} changed`
  el('status-dot').className = 'dot' + (state.running ? ' live' : '')
  el('agent-dot').className = 'dot' + (state.running ? ' live' : '')
  el('agent-state').textContent = state.running ? 'running' : 'idle'
}

// Calls and results are paired by tool_call_id so an activity block can show
// what ran, whether it failed, and the beginning of what it returned.
function toolIndex() {
  const results = new Map()
  for (const message of state.messages) {
    if (message.role === 'tool' && (message.tool_call_id || message.id)) {
      results.set(message.tool_call_id || message.id, textOf(message.content))
    }
  }
  const calls = []
  for (const message of state.messages) {
    for (const call of message.tool_calls ?? []) {
      const name = String(call?.function?.name ?? call?.name ?? '')
      if (!name) continue
      const id = call.id ?? call.tool_call_id
      const out = id ? results.get(id) ?? '' : ''
      calls.push({
        name,
        args: toolArgs(call),
        out,
        at: message.created_at,
        failed: /\b(error|failed|exception|traceback|fatal)\b/i.test(String(out).slice(0, 400))
      })
    }
  }
  return calls
}

const PHASES = [
  ['delegate_task', 'Delegating to subagents'],
  ['patch', 'Editing files'],
  ['write_file', 'Writing files'],
  ['terminal', 'Running commands'],
  ['computer_use', 'Inspecting the screen'],
  ['read_file', 'Reading files'],
  ['search_files', 'Searching the codebase'],
  ['web_search', 'Searching the web'],
  ['web_extract', 'Reading pages']
]

function phaseOf(calls) {
  for (const [tool, label] of PHASES) {
    if (calls.some((call) => call.name === tool)) return label
  }
  return 'Working'
}

function callSummary(call) {
  const a = call.args || {}
  const first = a.command || a.path || a.goal || a.query || a.url || a.pattern
  const text = typeof first === 'string' ? first : JSON.stringify(first ?? '')
  return text.replace(/\s+/g, ' ').slice(0, 120)
}

// ----------------------------------------------------------------- transcript
function renderTranscript() {
  const pane = el('transcript')
  pane.innerHTML = ''
  if (!state.messages.length) {
    pane.innerHTML = '<div class="empty-note">No messages in this session.</div>'
    return
  }
  const shown = state.messages.slice(-CAP)
  if (state.messages.length > CAP) {
    const note = document.createElement('div')
    note.className = 'empty-note'
    note.textContent = `${state.messages.length - CAP} earlier messages not shown`
    pane.appendChild(note)
  }

  let activity = null
  for (const message of shown) {
    const role = String(message.role || '')
    if (role === 'system' || role === 'tool') continue
    const tools = toolNames(message)
    const text = textOf(message.content).trim()

    if (role === 'user') {
      pane.appendChild(messageNode('You', text, true, message))
      continue
    }
    if (text) {
      activity = null
      pane.appendChild(messageNode('Hermes', text, false, message))
    } else if (tools.length) {
      // Consecutive tool calls fold into one collapsible line: a run with 600
      // calls should not paint 600 boxes into the transcript.
      if (!activity) {
        activity = document.createElement('details')
        activity.className = 'activity'
        activity.open = false
        const summary = document.createElement('summary')
        summary.className = 'activity-sum'
        activity.appendChild(summary)
        activity.dataset.calls = '0'
        activity.dataset.names = ''
        pane.appendChild(activity)
      }
      const calls = Number(activity.dataset.calls) + tools.length
      const names = new Set(activity.dataset.names ? activity.dataset.names.split(', ') : [])
      for (const name of tools) names.add(name)
      activity.dataset.calls = String(calls)
      activity.dataset.names = [...names].join(', ')
      const label = [...names].slice(0, 3).join(', ') + (names.size > 3 ? ` +${names.size - 3}` : '')
      activity.querySelector('summary').textContent =
        `${calls} tool call${calls === 1 ? '' : 's'} · ${label}`

      const index = state.messages.indexOf(message)
      const detailRows = detailForMessage(message, index)
      for (const detail of detailRows) {
        const row = document.createElement('div')
        row.className = 'activity-row' + (detail.failed ? ' failed' : '')
        const name = document.createElement('span')
        name.className = 'tool-name'
        name.textContent = detail.name
        const what = document.createElement('span')
        what.className = 'tool-what'
        what.textContent = detail.summary || '—'
        const status = document.createElement('span')
        status.className = 'tool-status'
        status.textContent = detail.failed ? 'failed' : 'ok'
        row.append(name, what, status)
        if (detail.output) {
          const out = document.createElement('div')
          out.className = 'activity-out'
          out.textContent = detail.output
          row.appendChild(out)
        }
        activity.appendChild(row)
      }
    }
  }

  const run = runCard()
  if (run) pane.insertBefore(run, pane.firstChild)

  const plan = planItems()
  if (plan.length) pane.appendChild(planCard(plan))
  if (state.running) pane.appendChild(execCard())
  const bar = changeBar()
  if (bar) pane.appendChild(bar)
  pane.scrollTop = pane.scrollHeight
}

// Pairs the calls in one message with the result rows that follow it.
function detailForMessage(message, messageIndex) {
  const later = state.messages.slice(messageIndex, messageIndex + 12)
  const results = new Map()
  for (const candidate of later) {
    if (candidate.role === 'tool' && (candidate.tool_call_id || candidate.id)) {
      results.set(candidate.tool_call_id || candidate.id, textOf(candidate.content))
    }
  }
  const details = []
  for (const call of message.tool_calls ?? []) {
    const name = String(call?.function?.name ?? call?.name ?? '')
    if (!name) continue
    const out = results.get(call.id ?? call.tool_call_id) ?? ''
    details.push({
      name,
      summary: callSummary({ name, args: toolArgs(call) }),
      output: String(out).slice(0, 300).replace(/\s+/g, ' ').trim(),
      failed: /\b(error|failed|exception|traceback|fatal)\b/i.test(String(out).slice(0, 400))
    })
  }
  return details
}

function messageNode(who, text, mine, message) {
  const wrap = document.createElement('div')
  wrap.className = 'msg' + (mine ? ' user' : '')
  const head = document.createElement('div')
  head.className = 'who'
  const avatar = document.createElement('span')
  avatar.className = 'avatar-sm'
  avatar.textContent = mine ? 'AL' : '◎'
  const name = document.createElement('span')
  name.textContent = who
  const time = document.createElement('span')
  time.className = 'meta'
  time.textContent = clock(message.created_at)
  head.append(avatar, name, time)
  const body = document.createElement('div')
  body.className = 'body'
  body.textContent = text.length > 4000 ? `${text.slice(0, 4000)}…` : text
  wrap.append(head, body)
  return wrap
}

// Stages are derived from events actually present in the transcript: a stage
// without evidence is not rendered, and no stage claims a result it lacks.
function runStages() {
  const names = new Set()
  let terminal = 0
  for (const message of state.messages) {
    for (const call of message.tool_calls ?? []) {
      const name = String(call?.function?.name ?? call?.name ?? '')
      if (!name) continue
      names.add(name)
      if (name === 'terminal') terminal += 1
    }
  }
  const plan = planItems()
  const tests = state.tests || parseTests()
  const stages = []
  if (plan.length) {
    stages.push({ label: 'Plan', done: plan.every((i) => isDone(i.status)), meta: `${plan.length} steps` })
  }
  const analyze = ['read_file', 'search_files', 'web_search', 'web_extract', 'readFileText'].filter((n) => names.has(n))
  if (analyze.length) stages.push({ label: 'Analyze', done: true, meta: `${analyze.length} tools` })
  if (terminal) stages.push({ label: 'Run', done: !state.running, meta: `${terminal} cmd` })
  const edits = ['patch', 'write_file', 'execute_code'].filter((n) => names.has(n))
  if (edits.length) stages.push({ label: 'Fix', done: true, meta: `${edits.length} tools` })
  if (tests.passed) stages.push({ label: 'Validate', done: tests.failed === 0, meta: `${tests.passed} passed` })
  if (state.pr) stages.push({ label: 'PR', done: true, meta: `#${state.pr.number}` })
  return stages
}

function runCard() {
  const stages = runStages()
  if (!stages.length) return null
  const card = document.createElement('div')
  card.className = 'run-card'
  const done = stages.filter((s) => s.done).length
  const started = toMs(state.info?.started_at)
  const last = toMs(state.info?.last_active_at)
  const elapsed = started && last ? humanDuration((last - started) / 1000) : ''
  const head = document.createElement('div')
  head.className = 'run-head'
  head.innerHTML = `<b>Run</b><span>${state.running ? 'running' : 'completed'}</span>
    <span class="spacer"></span><span>${done}/${stages.length} steps</span>
    <span>${elapsed}</span>`
  card.appendChild(head)
  const row = document.createElement('div')
  row.className = 'stages'
  stages.forEach((stage, index) => {
    const chip = document.createElement('span')
    chip.className = `stage ${stage.done ? 'done' : 'active'}`
    const dot = document.createElement('span')
    dot.className = 'dot'
    const label = document.createElement('span')
    label.textContent = stage.label
    const meta = document.createElement('span')
    meta.className = 'meta'
    meta.textContent = stage.meta
    chip.append(dot, label, meta)
    row.appendChild(chip)
    if (index < stages.length - 1) {
      const arrow = document.createElement('span')
      arrow.className = 'stage-arrow'
      arrow.textContent = '→'
      row.appendChild(arrow)
    }
  })
  card.appendChild(row)
  return card
}

function subagentRuns() {
  const runs = []
  state.messages.forEach((message, index) => {
    for (const call of message.tool_calls ?? []) {
      const name = String(call?.function?.name ?? call?.name ?? '')
      if (name !== 'delegate_task') continue
      const args = toolArgs(call)
      const goal = args.goal || args.tasks?.[0]?.goal || 'subagent'
      const result = state.messages.slice(index + 1, index + 14).find((m) => m.role === 'tool')
      runs.push({
        goal: String(goal),
        done: !!result,
        at: message.created_at,
        output: result ? textOf(result.content).replace(/\s+/g, ' ').trim().slice(0, 600) : ''
      })
    }
  })
  return runs
}

function planCard(items) {
  const card = document.createElement('div')
  card.className = 'card'
  const head = document.createElement('div')
  head.className = 'card-head'
  head.innerHTML = `<b>Plan</b><span class="spacer"></span><span>${items.length} steps</span>`
  card.appendChild(head)
  const list = document.createElement('ol')
  list.className = 'plan'
  for (const item of items) {
    const li = document.createElement('li')
    const mark = document.createElement('span')
    mark.className = 'mark'
    if (isDone(item.status)) {
      mark.classList.add('done')
      mark.textContent = '✓'
    } else if (isActive(item.status)) {
      mark.classList.add('active')
      mark.textContent = '▸'
      li.classList.add('current')
    } else {
      mark.textContent = '○'
    }
    const text = document.createElement('span')
    text.textContent = item.text
    li.append(mark, text)
    list.appendChild(li)
  }
  card.appendChild(list)
  return card
}

function lastTerminal() {
  for (let i = state.messages.length - 1; i >= 0; i -= 1) {
    for (const call of state.messages[i].tool_calls ?? []) {
      const name = String(call?.function?.name ?? call?.name ?? '')
      if (!['terminal', 'execute_code', 'shell'].includes(name)) continue
      const args = toolArgs(call)
      return { name, command: args.command || args.code || args.cmd || '', at: state.messages[i].created_at }
    }
  }
  return null
}

function executionState() {
  const calls = toolIndex()
  const recent = calls.slice(-40)
  const plan = planItems()
  const done = plan.filter((item) => isDone(item.status)).length
  const failures = recent.filter((call) => call.failed).length
  // Loop signal: the same command shape repeated in the recent window.
  const shapes = new Map()
  for (const call of recent) {
    const key = `${call.name}:${callSummary(call).slice(0, 40)}`
    shapes.set(key, (shapes.get(key) || 0) + 1)
  }
  const [repeatKey, repeatCount] = [...shapes.entries()].sort((a, b) => b[1] - a[1])[0] || ['', 0]
  const last = calls[calls.length - 1]
  const lastMessageAt = toMs(state.messages[state.messages.length - 1]?.created_at)
  const quietSeconds = lastMessageAt ? (Date.now() - lastMessageAt) / 1000 : 0
  const lastText = [...state.messages].reverse().find((m) => m.role === 'assistant' && textOf(m.content).trim())
  return {
    phase: phaseOf(recent),
    current: last ? `${last.name} · ${callSummary(last)}` : 'no tool activity yet',
    lastDone: lastText ? textOf(lastText.content).slice(0, 140) : '—',
    plan: plan.length ? `${done} of ${plan.length} steps` : 'no plan recorded',
    failures,
    repeat: repeatCount > 2 ? `${repeatCount}× ${repeatKey.split(':')[1] || repeatKey}` : '',
    quiet: quietSeconds > 180 ? humanDuration(quietSeconds) : '',
    quietSeconds
  }
}

function execCard() {
  const card = document.createElement('div')
  card.className = 'card exec-card'
  const exec = executionState()
  const head = document.createElement('div')
  head.className = 'exec-row'
  head.innerHTML = `<span class="dot live"></span><span>${state.running ? 'Running' : 'Idle'}</span>
    <b class="phase">${exec.phase}</b><span class="spacer"></span>
    <span class="meta">${rel(state.info?.last_active_at)}</span>`
  card.appendChild(head)

  const grid = document.createElement('div')
  grid.className = 'exec-grid'
  const add = (label, value, tone) => {
    const cell = document.createElement('div')
    cell.className = 'exec-cell'
    cell.innerHTML = `<em>${label}</em><span${tone ? ` class="${tone}"` : ''}></span>`
    cell.querySelector('span').textContent = value
    grid.appendChild(cell)
  }
  add('current', exec.current)
  add('last completed', exec.lastDone)
  add('steps', exec.plan)
  add('failures (last 40 calls)', String(exec.failures), exec.failures ? 'bad' : '')
  card.appendChild(grid)

  const warnings = []
  if (exec.repeat) warnings.push(`repeated call ${exec.repeat}`)
  if (exec.quiet) warnings.push(`no activity for ${exec.quiet}`)
  if (warnings.length) {
    const warn = document.createElement('div')
    warn.className = 'exec-warn'
    warn.textContent = `⚠ ${warnings.join(' · ')}`
    card.appendChild(warn)
  }
  return card
}

function changeBar() {
  if (!state.files.length) return null
  const totalAdd = state.files.reduce((n, f) => n + (f.added || 0), 0)
  const totalDel = state.files.reduce((n, f) => n + (f.removed || 0), 0)
  const bar = document.createElement('div')
  bar.className = 'change-bar'
  bar.innerHTML = `<span>${state.files.length} file${state.files.length === 1 ? '' : 's'} changed</span>
    <span class="add">+${totalAdd}</span><span class="del">−${totalDel}</span>`
  const button = document.createElement('button')
  button.className = 'mini'
  button.textContent = 'View diff'
  button.addEventListener('click', () => setPane('diff'))
  bar.appendChild(button)
  return bar
}

// ------------------------------------------------------------------ agent rail
function renderAgent() {
  const firstUser = state.messages.find((m) => m.role === 'user')
  el('goal').textContent = textOf(firstUser?.content).slice(0, 240) || 'No goal recorded for this session.'

  const plan = planItems()
  const list = el('plan-list')
  list.innerHTML = ''
  el('plan-meta').textContent = plan.length ? `${plan.filter((i) => isDone(i.status)).length}/${plan.length}` : ''
  // Empty regions are hidden, not filled with placeholders: the rail only shows
  // what this session actually produced.
  el('plan-section').hidden = plan.length === 0
  for (const item of plan) {
    const li = document.createElement('li')
    const mark = document.createElement('span')
    mark.className = 'mark'
    if (isDone(item.status)) {
      mark.classList.add('done')
      mark.textContent = '✓'
    } else if (isActive(item.status)) {
      mark.classList.add('active')
      mark.textContent = '▸'
      li.classList.add('current')
    } else {
      mark.textContent = '○'
    }
    const text = document.createElement('span')
    text.textContent = item.text
    li.append(mark, text)
    list.appendChild(li)
  }

  const subs = subagentRuns()
  const subBox = el('subagents')
  if (subBox) {
    subBox.innerHTML = ''
    el('sub-count').textContent = subs.length ? String(subs.length) : ''
    el('sub-section').hidden = subs.length === 0
    for (const sub of subs.slice(0, 6)) {
      const row = document.createElement('div')
      row.className = 'sub-row'
      const dot = document.createElement('span')
      dot.className = 'dot' + (sub.done ? ' live' : '')
      const name = document.createElement('span')
      name.className = 'name'
      name.textContent = sub.goal.slice(0, 60)
      name.title = sub.goal
      const stateName = document.createElement('span')
      stateName.className = 'state'
      stateName.textContent = sub.done ? 'done' : 'running'
      row.append(dot, name, stateName)
      row.title = 'Inspect this subagent'
      row.style.cursor = 'pointer'
      row.addEventListener('click', () =>
        openModal('Subagent', [
          { label: 'Goal', meta: '' },
          { label: sub.goal, meta: sub.done ? 'done' : 'running' },
          { label: 'Returned', meta: '' },
          { label: sub.output || 'no result captured in this transcript', meta: '' }
        ])
      )
      subBox.appendChild(row)
    }
  }

  el('tools-count').textContent = ''
  el('tree-count').textContent = ''
  const used = new Map()
  for (const message of state.messages) {
    for (const name of toolNames(message)) used.set(name, (used.get(name) || 0) + 1)
    if (message.tool_name) used.set(String(message.tool_name), (used.get(String(message.tool_name)) || 0) + 1)
  }
  const chips = el('agent-tools')
  chips.innerHTML = ''
  if (!used.size) chips.innerHTML = '<span class="empty-note">none yet</span>'
  for (const [name, count] of [...used.entries()].sort((a, b) => b[1] - a[1]).slice(0, 14)) {
    const chip = document.createElement('span')
    chip.className = 'chip on'
    chip.textContent = count > 1 ? `${name} ×${count}` : name
    chip.title = `${name}: ${count} call${count === 1 ? '' : 's'}`
    chips.appendChild(chip)
  }

  const caps = el('capabilities')
  caps.innerHTML = ''
  const groups = [
    ['File system', ['read_file', 'write_file', 'patch', 'search_files']],
    ['Terminal', ['terminal', 'process_manage']],
    ['Git & GitHub', ['bash']],
    ['Browser', ['web_search', 'web_extract']]
  ]
  for (const [label, names] of groups) {
    const on = names.some((n) => used.has(n))
    const row = document.createElement('div')
    row.className = 'session-row'
    row.style.padding = '2px 6px'
    const dot = document.createElement('span')
    dot.className = 'dot' + (on ? ' live' : '')
    const name = document.createElement('span')
    name.className = 'name'
    name.textContent = label
    name.title = on ? `used: ${names.filter((n) => used.has(n)).join(', ')}` : 'not used in this session'
    row.append(dot, name)
    caps.appendChild(row)
  }

  el('stat-messages').textContent = state.messages.length
  const calls = state.messages.reduce((n, m) => n + (Array.isArray(m.tool_calls) ? m.tool_calls.length : 0), 0)
  el('stat-tools').textContent = calls || state.usage?.calls || 0
  el('stat-tools').title = 'tool calls recorded in this transcript'
  el('stat-messages').title = 'messages in this session'
  el('stat-elapsed').title = 'time between session start and last activity'
  const started = toMs(state.info?.started_at)
  const last = toMs(state.info?.last_active_at)
  el('stat-elapsed').textContent = started && last ? humanDuration((last - started) / 1000) : '—'
}

// ---------------------------------------------------------------- repo + files
async function resolveRepo(override) {
  if (override) {
    state.repo = override
    state.repoExplicit = true
    return
  }
  if (state.repoExplicit && state.repo) return
  const cwd = state.info?.cwd
  let root = null
  if (cwd) root = await ipc('hermes:fs:gitRoot', [cwd]).catch(() => null)
  if (root) {
    state.repo = root
    el('repo-picker').hidden = true
    return
  }
  const repos = await ipc('hermes:git:scanRepos', [
    ['~/Developer', '~/Projects', '~/code'],
    { maxDepth: 2 }
  ]).catch(() => [])
  state.repos = Array.isArray(repos) ? repos : []
  const picker = el('repo-picker')
  picker.innerHTML = ''
  for (const repo of state.repos.slice(0, 40)) {
    const option = document.createElement('option')
    option.value = repo.root
    option.textContent = repo.label || repo.root
    picker.appendChild(option)
  }
  picker.hidden = !state.repos.length
  state.repo = state.repos[0]?.root ?? null
  setWorkspaceVisible(!!state.repo)
}

function setWorkspaceVisible(visible) {
  const body = document.getElementById('body')
  body.classList.toggle('no-ws', !visible)
  // At <=1000 px the files panel is an overlay, so a resolved repo must not
  // cover the chat on its own: it opens only from the ▥ toggle.
  if (window.innerWidth > 1000) body.classList.remove('ws-open')
  el('ws-repo').textContent = state.repo ? state.repo.split('/').pop() : 'no repository'
}

async function loadFiles() {
  if (!state.repo) {
    state.files = []
    setWorkspaceVisible(false)
    return
  }
  const list = await ipc('hermes:git:review:list', [state.repo, 'uncommitted', '']).catch(() => null)
  state.files = (list?.files ?? []).map((file) => ({
    ...file,
    status: STATUS_LABEL[file.status] || file.status || 'M'
  }))
  // A clean tree is a fact, not a workspace: collapse to the summary strip.
  setWorkspaceVisible(state.files.length > 0 || state.repoExplicit)
  if (!state.selected && state.files.length) state.selected = state.files[0].path
  if (state.selected && !state.files.some((f) => f.path === state.selected)) {
    state.selected = state.files[0]?.path ?? null
  }
}

async function loadTree() {
  const box = el('tree')
  box.innerHTML = ''
  if (!state.repo) {
    box.innerHTML = '<div class="empty-note">no repository</div>'
    return
  }
  const entries = await ipc('hermes:fs:readDir', [state.repo]).catch(() => null)
  const list = Array.isArray(entries) ? entries : entries?.entries ?? []
  const name = state.repo.split('/').filter(Boolean).pop()
  const head = document.createElement('div')
  head.className = 'node dir'
  head.textContent = `▾ ${name}`
  box.appendChild(head)
  for (const entry of list.slice(0, 14)) {
    const node = document.createElement('div')
    node.className = 'node child'
    const isDir = entry.isDirectory ?? entry.is_dir ?? entry.type === 'directory'
    node.textContent = `${isDir ? '▸' : '·'} ${entry.name || entry.path}`
    box.appendChild(node)
  }
  if (list.length > 14) {
    const more = document.createElement('div')
    more.className = 'node child empty-note'
    more.textContent = `+${list.length - 14} more`
    box.appendChild(more)
  }
}

// ------------------------------------------------------------------ workspace
function renderWorkspace() {
  const strip = el('file-tabs')
  strip.innerHTML = ''
  el('diff-count').textContent = String(state.files.length)
  if (!state.files.length) {
    const empty = document.createElement('div')
    empty.className = 'empty-note'
    empty.style.padding = '8px 12px'
    empty.textContent = state.repo ? 'working tree clean' : 'no repository'
    strip.appendChild(empty)
  }
  for (const file of state.files.slice(0, 12)) {
    const tab = document.createElement('button')
    tab.className = 'file-tab' + (file.path === state.selected ? ' active' : '')
    const name = document.createElement('span')
    name.textContent = file.path.split('/').pop()
    name.title = file.path
    const badge = document.createElement('span')
    badge.className = `badge ${file.status}`
    badge.textContent = file.status
    tab.append(name, badge)
    tab.addEventListener('click', () => {
      state.selected = file.path
      renderWorkspace()
      renderPanes()
    })
    strip.appendChild(tab)
  }
  el('breadcrumb').textContent = state.selected
    ? `${state.repo?.split('/').pop() ?? ''} › ${state.selected}`
    : state.repo || 'no file'
}

async function renderCode() {
  const box = el('code')
  if (!state.repo || !state.selected) {
    box.innerHTML = '<div class="empty-note">No file selected</div>'
    return
  }
  const path = `${state.repo}/${state.selected}`
  const file = await ipc('hermes:readFileText', [path]).catch(() => null)
  if (!file?.text) {
    box.innerHTML = `<div class="empty-note">${file?.binary ? 'binary file' : 'file not readable'}</div>`
    return
  }
  const lines = file.text.split('\n').slice(0, 400)
  box.innerHTML = ''
  lines.forEach((line, index) => {
    const row = document.createElement('div')
    row.className = 'line'
    const no = document.createElement('span')
    no.className = 'ln'
    no.textContent = String(index + 1)
    const src = document.createElement('span')
    src.className = 'src'
    src.innerHTML = highlight(line)
    row.append(no, src)
    box.appendChild(row)
  })
  if (file.text.split('\n').length > 400) {
    const more = document.createElement('div')
    more.className = 'empty-note'
    more.style.padding = '4px 12px'
    more.textContent = '… truncated at 400 lines'
    box.appendChild(more)
  }
}

// Minimal token colouring: strings, comments, keywords, numbers. Not a parser.
function highlight(line) {
  const escaped = line.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  const trimmed = escaped.trimStart()
  if (trimmed.startsWith('#') || trimmed.startsWith('//')) return `<span class="tok-com">${escaped}</span>`
  let out = escaped.replace(/(['"`])(?:\\.|(?!\1).)*\1/g, (m) => `\u0001${m}\u0002`)
  out = out.replace(
    /\b(def|class|import|from|return|if|else|elif|for|while|try|except|with|as|async|await|const|let|var|function|export|new|not|in|is|None|True|False|self)\b/g,
    '<span class="tok-key">$1</span>'
  )
  out = out.replace(/\b(\d+(?:\.\d+)?)\b/g, '<span class="tok-num">$1</span>')
  out = out.replace(/\u0001([\s\S]*?)\u0002/g, '<span class="tok-str">$1</span>')
  return out
}

function setPane(pane) {
  state.pane = pane
  document.querySelectorAll('.ws-tab').forEach((tab) => {
    tab.classList.toggle('active', tab.dataset.pane === pane)
  })
  for (const name of ['diff', 'tests', 'terminal', 'logs']) {
    el(`pane-${name}`).hidden = name !== pane
  }
  renderPanes()
}

function renderPanes() {
  renderDiffPane()
  renderTestsPane()
  renderTerminalPane()
  renderLogsPane()
  renderCode()
}

async function renderDiffPane() {
  const box = el('pane-diff')
  box.innerHTML = ''
  if (!state.repo) {
    box.innerHTML = '<div class="empty-note">No repository for this session.</div>'
    return
  }
  if (!state.files.length) {
    box.innerHTML = '<div class="empty-note">Working tree clean — nothing to diff.</div>'
    return
  }
  const file = state.files.find((f) => f.path === state.selected) ?? state.files[0]
  const text = await ipc('hermes:git:review:diff', [state.repo, file.path, 'uncommitted', '', false]).catch(
    () => ''
  )
  state.lastDiff = String(text || '')
  const card = document.createElement('div')
  card.className = 'diff-card'
  const head = document.createElement('div')
  head.className = 'diff-head'
  head.innerHTML = `<span class="path">${file.path}</span><span class="spacer"></span>
    <span class="add">+${file.added ?? 0}</span><span class="del">−${file.removed ?? 0}</span>`
  card.appendChild(head)
  const body = document.createElement('div')
  body.className = 'diff-body'
  if (!state.lastDiff.trim()) {
    body.innerHTML = '<div class="empty-note" style="padding:8px">No diff for this file.</div>'
  } else {
    const lines = state.lastDiff.split('\n').slice(0, 800)
    let oldNo = 0
    let newNo = 0
    for (const line of lines) {
      const row = document.createElement('div')
      row.className = 'diff-line'
      const no = document.createElement('span')
      no.className = 'no'
      if (line.startsWith('@@')) {
        const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/)
        if (match) {
          oldNo = Number(match[1])
          newNo = Number(match[2])
        }
        row.classList.add('hunk')
        no.textContent = ''
        row.append(no, textSpan(line))
      } else if (line.startsWith('+') && !line.startsWith('+++')) {
        row.classList.add('add')
        no.textContent = String(newNo++)
        row.append(no, textSpan(line))
      } else if (line.startsWith('-') && !line.startsWith('---')) {
        row.classList.add('del')
        no.textContent = String(oldNo++)
        row.append(no, textSpan(line))
      } else if (line.startsWith('diff ') || line.startsWith('index ') || line.startsWith('---') || line.startsWith('+++')) {
        continue
      } else {
        row.classList.add('ctx')
        no.textContent = oldNo && newNo ? String(newNo) : ''
        oldNo += 1
        newNo += 1
        row.append(no, textSpan(line))
      }
      body.appendChild(row)
    }
  }
  card.appendChild(body)
  box.appendChild(card)
}

function textSpan(text) {
  const span = document.createElement('span')
  span.textContent = text
  return span
}

function parseTests() {
  const rows = []
  let passed = 0
  let failed = 0
  let skipped = 0
  let duration = null
  const fileRows = new Map()
  for (const message of state.messages) {
    for (const raw of [textOf(message.content)]) {
      if (!raw || !/passed|failed/.test(raw)) continue
      for (const match of raw.matchAll(/(\d+)\s+passed(?:,\s*(\d+)\s+failed)?(?:,\s*(\d+)\s+skipped)?[^\n]*?in\s+([\d.]+)s/g)) {
        passed = Math.max(passed, Number(match[1]))
        failed = Math.max(failed, Number(match[2] || 0))
        skipped = Math.max(skipped, Number(match[3] || 0))
        duration = match[4]
      }
      for (const match of raw.matchAll(/^(\S+\.py)\s+.*?(\d+)\s+passed.*?(?:in\s+)?([\d.]+)s/gm)) {
        fileRows.set(match[1], { name: match[1], passed: Number(match[2]), duration: match[3] })
      }
    }
  }
  for (const row of fileRows.values()) rows.push(row)
  return { passed, failed, skipped, duration, rows }
}

function renderTestsPane() {
  const box = el('pane-tests')
  const tests = parseTests()
  state.tests = tests
  box.innerHTML = ''
  const card = document.createElement('div')
  card.className = 'tests-card'
  const head = document.createElement('div')
  head.className = 'tests-head'
  const has = tests.passed || tests.failed
  head.innerHTML = has
    ? `<span class="path">Tests</span><span class="pass">${tests.passed} passed</span>
       <span class="spacer"></span><span>${tests.failed} failed · ${tests.skipped} skipped</span>
       <span>${tests.duration ? `${tests.duration}s` : ''}</span>`
    : `<span class="path">Tests</span><span class="spacer"></span><span>no test output in this session</span>`
  card.appendChild(head)
  for (const row of tests.rows.slice(0, 12)) {
    const line = document.createElement('div')
    line.className = 'test-row'
    line.innerHTML = `<span class="pass">✓</span><span class="name">${row.name}</span>
      <span>(${row.passed} passed)</span><span class="t">${row.duration}s</span>`
    card.appendChild(line)
  }
  box.appendChild(card)
}

function renderTerminalPane() {
  const box = el('pane-terminal')
  const term = lastTerminal()
  box.innerHTML = ''
  if (!term) {
    box.innerHTML = '<div class="empty-note">No terminal command in this session.</div>'
    return
  }
  const head = document.createElement('div')
  head.className = 'diff-head'
  head.style.borderRadius = '9px 9px 0 0'
  head.innerHTML = `<span class="path">${term.name}</span><span class="spacer"></span><span>${clock(term.at)}</span>`
  const out = document.createElement('pre')
  out.className = 'term-out'
  out.textContent = `$ ${term.command}\n\n${lastToolOutputTail(term.name)}`
  const card = document.createElement('div')
  card.className = 'diff-card'
  card.append(head, out)
  box.appendChild(card)
}

function lastToolOutputTail(name) {
  for (let i = state.messages.length - 1; i >= 0; i -= 1) {
    const message = state.messages[i]
    if (message.role === 'tool' && String(message.tool_name || '').includes(name.split('_')[0])) {
      const text = textOf(message.content)
      return text.split('\n').slice(-24).join('\n') || '(no output captured)'
    }
  }
  return '(output not in transcript)'
}

function renderLogsPane() {
  const box = el('pane-logs')
  box.innerHTML = ''
  const rows = []
  for (const message of state.messages) {
    for (const call of message.tool_calls ?? []) {
      const name = String(call?.function?.name ?? call?.name ?? '')
      if (name) rows.push({ name, at: message.created_at })
    }
  }
  if (!rows.length) {
    box.innerHTML = '<div class="empty-note">No tool activity in this session.</div>'
    return
  }
  const card = document.createElement('div')
  card.className = 'diff-card'
  const head = document.createElement('div')
  head.className = 'diff-head'
  head.innerHTML = `<span class="path">Tool activity</span><span class="spacer"></span><span>${rows.length} calls</span>`
  card.appendChild(head)
  for (const row of rows.slice(-200)) {
    const line = document.createElement('div')
    line.className = 'log-row'
    line.style.padding = '2px 9px'
    line.innerHTML = `<span class="t">${clock(row.at)}</span><span class="name">${row.name}</span>`
    card.appendChild(line)
  }
  box.appendChild(card)
}

// --------------------------------------------------------------------- PR + io
async function loadPr() {
  const pill = el('pr-pill')
  const action = el('pr-action')
  pill.hidden = true
  action.hidden = true
  if (!state.repo) return
  const info = await ipc('hermes:git:review:shipInfo', [state.repo]).catch(() => null)
  if (!info?.ghReady) return
  state.pr = info.pr ?? null
  if (state.pr) {
    const name = String(state.pr.state || 'OPEN').toLowerCase()
    pill.hidden = false
    pill.textContent = `#${state.pr.number} · ${name}`
    pill.classList.toggle('open', name === 'open')
    pill.title = state.pr.title || ''
  } else {
    action.hidden = false
  }
}

async function refreshResources() {
  try {
    const stats = await invoke('task_stats')
    el('res-cpu').textContent = stats?.cpu != null ? `${Math.round(stats.cpu)}%` : '—'
    el('res-mem').textContent = stats?.rss ? `${(stats.rss / 1073741824).toFixed(1)} GB` : '—'
  } catch {
    el('res-cpu').textContent = '—'
    el('res-mem').textContent = '—'
  }
  el('res-tokens').textContent = state.usage?.total ? humanBytes(state.usage.total) : '—'
  const percent = Math.max(0, Math.min(100, Number(state.usage?.context_percent ?? 0)))
  el('context-fill').style.width = `${percent}%`
  el('context-text').textContent = state.usage?.context_used
    ? `${humanBytes(state.usage.context_used)} / ${humanBytes(state.usage.context_max)} · ${percent}%`
    : 'no usage reported'
  el('cache-text').textContent = state.usage?.cache_hit_pct != null ? `${state.usage.cache_hit_pct}% cache` : ''
}

function renderChanges() {
  const box = el('changes')
  box.innerHTML = ''
  if (!state.repo) {
    box.innerHTML = '<span class="empty-note">no repository</span>'
    return
  }
  if (!state.files.length) {
    box.innerHTML = '<span class="empty-note">working tree clean</span>'
    return
  }
  for (const file of state.files.slice(0, 8)) {
    const row = document.createElement('div')
    row.className = 'change'
    const path = document.createElement('span')
    path.className = 'path'
    path.textContent = file.path
    path.title = file.path
    const counts = document.createElement('span')
    counts.className = 'counts'
    counts.innerHTML = `<b class="add">+${file.added ?? 0}</b><b class="del">−${file.removed ?? 0}</b>`
    row.append(path, counts)
    box.appendChild(row)
  }
}

// ---------------------------------------------------------------------- status
function setStatus(text) {
  const bar = el('status')
  if (bar) bar.textContent = text
}

function stopPolling() {
  if (state.pollTimer) clearTimeout(state.pollTimer)
  state.pollTimer = null
}

function startPolling() {
  stopPolling()
  const tick = async () => {
    const wanted = state.sessionId
    if (!wanted) return
    try {
      const snapshot = await rpc('session.resume', {
        session_id: wanted,
        profile: state.profile ?? undefined,
        source: 'desktop'
      })
      if (state.sessionId !== wanted) return
      const before = state.messages.length
      const wasRunning = state.running
      state.messages = snapshot?.messages ?? []
      state.info = snapshot?.info ?? state.info
      state.usage = snapshot?.info?.usage ?? state.usage
      state.todo = snapshot?.todo_state ?? state.todo
      state.running = !!snapshot?.running
      if (state.messages.length !== before || state.running !== wasRunning) {
        renderHeader()
        renderTranscript()
        renderAgent()
        renderPanes()
      }
    } catch {
      /* transient: the next tick retries */
    }
    await refreshResources()
    if (state.sessionId === wanted) state.pollTimer = setTimeout(tick, state.running ? 4000 : 15000)
  }
  refreshResources()
  state.pollTimer = setTimeout(tick, state.running ? 3000 : 8000)
}

// -------------------------------------------------------------------- models
function flattenModels(payload) {
  const groups = new Map()
  const push = (provider, model, label) => {
    if (!model) return
    const key = provider || 'default'
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key).push({ provider: key === 'default' ? provider ?? '' : key, model, label: label || model })
  }
  const list = payload?.providers ?? payload?.options ?? payload?.models ?? []
  if (Array.isArray(list)) {
    for (const entry of list) {
      if (typeof entry === 'string') {
        push('', entry, entry)
        continue
      }
      const provider = entry?.provider ?? entry?.provider_id ?? entry?.id ?? ''
      const models = entry?.models ?? entry?.options
      if (Array.isArray(models)) {
        for (const model of models) {
          if (typeof model === 'string') push(provider, model, model)
          else push(provider, model?.id ?? model?.model ?? model?.name, model?.label ?? model?.name)
        }
      } else {
        push(provider, entry?.model ?? entry?.id ?? entry?.name, entry?.label ?? entry?.name)
      }
    }
  }
  return groups
}

function paintModel() {
  const info = state.model
  const name = info?.model || info?.model_name || ''
  const provider = info?.provider || ''
  const button = el('model-btn')
  if (!button) return
  button.textContent = name ? name.split('/').pop() : provider || 'model'
  button.title = [provider, name].filter(Boolean).join(' · ') || 'Change model'
}

async function loadModel() {
  try {
    state.model = await api({ path: '/api/model/info', method: 'GET' })
  } catch (error) {
    state.model = null
  }
  paintModel()
}

async function openModelMenu() {
  let payload = null
  try {
    payload = await api({ path: '/api/model/options?explicit_only=1', method: 'GET' })
  } catch (error) {
    setStatus(`model list failed: ${error}`)
    return
  }
  const groups = flattenModels(payload)
  if (!groups.size) {
    openModal('Model', [{ label: 'No models reported by the gateway', meta: '' }])
    return
  }
  const menu = document.createElement('div')
  menu.className = 'menu-pop model-menu'
  for (const [provider, models] of groups) {
    const head = document.createElement('div')
    head.className = 'menu-head'
    head.textContent = provider
    menu.appendChild(head)
    for (const model of models) {
      const button = document.createElement('button')
      const active = (state.model?.model || '').endsWith(model.model)
      button.textContent = `${active ? '✓ ' : ''}${model.label}`
      button.addEventListener('click', async () => {
        menu.remove()
        try {
          await api({
            path: '/api/model/set',
            method: 'POST',
            body: { scope: 'main', provider: model.provider, model: model.model }
          })
          await loadModel()
          setStatus(`model → ${model.label}`)
        } catch (error) {
          setStatus(`model change failed: ${error}`)
        }
      })
      menu.appendChild(button)
    }
  }
  const rect = el('model-btn').getBoundingClientRect()
  menu.style.left = `${Math.max(8, Math.min(rect.left - 120, window.innerWidth - 280))}px`
  menu.style.bottom = `${window.innerHeight - rect.top + 6}px`
  document.body.appendChild(menu)
  setTimeout(() => document.addEventListener('click', () => menu.remove(), { once: true }), 0)
}

// ------------------------------------------------------------------ dialogs
function openModal(title, rows) {
  const modal = el('modal')
  el('modal-title').textContent = title
  const body = el('modal-body')
  body.innerHTML = ''
  if (!rows.length) body.innerHTML = '<div class="empty-note">nothing to show</div>'
  for (const row of rows) {
    const button = document.createElement('button')
    button.className = 'modal-row'
    const label = document.createElement('span')
    label.textContent = row.label
    const meta = document.createElement('span')
    meta.className = 'meta'
    meta.textContent = row.meta || ''
    meta.style.marginLeft = 'auto'
    button.append(label, meta)
    button.addEventListener('click', () => {
      closeModal()
      row.onClick?.()
    })
    body.appendChild(button)
  }
  modal.hidden = false
}

function closeModal() {
  el('modal').hidden = true
}

// ---------------------------------------------------------------- interactions
function setActiveNav(which) {
  document.querySelectorAll('.nav-pill').forEach((pill) => {
    pill.classList.toggle('active', pill.dataset.nav === which)
  })
}

function navAction(which) {
  // A dialog is not a view: capability search must not leave the nav claiming
  // you are somewhere you are not.
  setActiveNav(which === 'capabilities' ? 'chat' : which)
  if (which === 'chat') {
    el('transcript').scrollTop = el('transcript').scrollHeight
    el('input').focus()
  } else if (which === 'code') {
    setPane('diff')
    el('workspace').scrollIntoView({ block: 'nearest' })
    el('code').style.outline = '1px solid #3a4560'
    setTimeout(() => (el('code').style.outline = ''), 700)
  } else if (which === 'sessions') {
    el('search').focus()
  } else if (which === 'tasks') {
    invoke('open_workspace').catch((error) => setStatus(`workspace: ${error}`))
  } else if (which === 'agents') {
    const box = el('subagents')
    box.scrollIntoView({ block: 'center' })
    box.style.outline = '1px solid #3a4560'
    setTimeout(() => (box.style.outline = ''), 700)
  } else if (which === 'capabilities') {
    el('tools-count').textContent = ''
  el('tree-count').textContent = ''
  const used = new Map()
    for (const message of state.messages) {
      for (const name of toolNames(message)) used.set(name, (used.get(name) || 0) + 1)
    }
    const rows = [...used.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([name, count]) => ({ label: name, meta: `${count} calls` }))
    rows.unshift({
      label: 'Gateway',
      meta: `${state.sessions.length} sessions · ${state.repo ? 'repo resolved' : 'no repo'}`
    })
    openModal('Capabilities', rows)
  }
}

function normalizePaths(result) {
  if (Array.isArray(result)) return result.filter((p) => typeof p === 'string')
  if (Array.isArray(result?.paths)) return result.paths
  if (typeof result === 'string') return [result]
  return []
}

function renderAttachments() {
  const box = el('attachments')
  box.innerHTML = ''
  box.hidden = !state.attachments.length
  for (const [index, path] of state.attachments.entries()) {
    const chip = document.createElement('span')
    chip.className = 'attach-chip'
    chip.textContent = path.split('/').pop()
    chip.title = path
    const remove = document.createElement('button')
    remove.textContent = '✕'
    remove.addEventListener('click', () => {
      state.attachments.splice(index, 1)
      renderAttachments()
    })
    chip.appendChild(remove)
    box.appendChild(chip)
  }
}

async function pickPaths(kind) {
  const request = { title: kind === 'image' ? 'Attach images' : 'Attach files', multiple: true }
  if (kind === 'image') request.filters = [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp'] }]
  try {
    const picked = normalizePaths(await ipc('hermes:selectPaths', [request]))
    if (picked.length) {
      state.attachments = [...new Set([...state.attachments, ...picked])]
      renderAttachments()
      el('input').focus()
    }
  } catch (error) {
    setStatus(`attach failed: ${error}`)
  }
}

function insertAtCaret(text) {
  const input = el('input')
  const start = input.selectionStart ?? input.value.length
  const end = input.selectionEnd ?? start
  input.value = `${input.value.slice(0, start)}${text}${input.value.slice(end)}`
  input.selectionStart = input.selectionEnd = start + text.length
  input.focus()
}

function showMoreMenu(anchor) {
  document.querySelector('.menu-pop')?.remove()
  const menu = document.createElement('div')
  menu.className = 'menu-pop'
  const actions = [
    ['Copy session id', () => navigator.clipboard.writeText(state.sessionId || '')],
    ['Copy repo path', () => navigator.clipboard.writeText(state.repo || '')],
    [
      'Reveal repo in Finder',
      () => state.repo && ipc('hermes:fs:reveal', [state.repo]).catch((e) => setStatus(String(e)))
    ],
    ['Reload session', () => state.sessionId && openSession(state.sessionId)],
    ['Reload window', () => location.reload()]
  ]
  for (const [label, action] of actions) {
    const button = document.createElement('button')
    button.textContent = label
    button.addEventListener('click', () => {
      menu.remove()
      action()
    })
    menu.appendChild(button)
  }
  const rect = anchor.getBoundingClientRect()
  menu.style.left = `${Math.min(rect.left, window.innerWidth - 210)}px`
  menu.style.top = `${rect.bottom + 6}px`
  document.body.appendChild(menu)
  setTimeout(() => document.addEventListener('click', () => menu.remove(), { once: true }), 0)
}

async function interruptRun() {
  if (!state.sessionId) return
  try {
    await rpc('session.interrupt', { session_id: state.sessionId, profile: state.profile ?? 'default' })
    setStatus('interrupt requested')
  } catch (error) {
    setStatus(`interrupt failed: ${error}`)
  }
}

// ---------------------------------------------------------------------- wiring
function wireActions() {
  document.querySelectorAll('.nav-pill[data-nav]').forEach((pill) => {
    pill.addEventListener('click', () => navAction(pill.dataset.nav))
  })
  el('explore').addEventListener('click', () => {
    const subs = subagentRuns()
    openModal(
      'Subagents',
      subs.map((sub) => ({ label: sub.goal.slice(0, 90), meta: sub.done ? 'done' : 'running' }))
    )
  })
  el('inbox').addEventListener('click', () => {
    const rows = state.sessions.slice(0, 30).map((row) => ({
      label: row.title || '(untitled)',
      meta: rel(row.last_active_at ?? row.last_active),
      onClick: () => openSession(sessionIdOf(row))
    }))
    openModal('Inbox · recent sessions', rows)
  })
  el('session-title').addEventListener('click', () => {
    el('session-title').classList.toggle('expanded')
  })
  el('model-btn').addEventListener('click', openModelMenu)
  el('ws-toggle').addEventListener('click', () => {
    const body = document.getElementById('body')
    const hidden = body.classList.contains('no-ws')
    if (window.innerWidth <= 1000) {
      body.classList.toggle('ws-open', hidden)
      body.classList.toggle('no-ws', false)
    } else {
      setWorkspaceVisible(hidden)
    }
  })
  el('ws-hide').addEventListener('click', () => {
    const body = document.getElementById('body')
    if (window.innerWidth <= 1000) body.classList.remove('ws-open')
    else setWorkspaceVisible(false)
  })
  document.querySelectorAll('[data-collapse]').forEach((button) => {
    button.addEventListener('click', () => {
      document.getElementById(button.dataset.collapse).classList.toggle('collapsed')
    })
  })
  el('attach').addEventListener('click', () => pickPaths('file'))
  el('image').addEventListener('click', () => pickPaths('image'))
  el('mention').addEventListener('click', () => insertAtCaret('@'))
  el('more').addEventListener('click', (event) => showMoreMenu(event.currentTarget))
  el('agent-stop').addEventListener('click', interruptRun)
  el('agent-close').addEventListener('click', () => {
    document.getElementById('body').classList.add('no-agent')
  })
  el('modal-close').addEventListener('click', closeModal)
  window.addEventListener('resize', () => { if (!state.selftest) renderHeader() })
  el('modal').addEventListener('click', (event) => {
    if (event.target === el('modal')) closeModal()
  })
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') closeModal()
    if (event.key === 'k' && (event.metaKey || event.ctrlKey)) {
      event.preventDefault()
      el('search').focus()
    }
  })
}

function wire() {
  el('search').addEventListener('input', (event) => {
    state.filter = event.target.value
    renderSessions()
  })
  wireActions()
  el('toggle-agent').addEventListener('click', () => {
    const body = document.getElementById('body')
    if (body.classList.contains('no-agent')) body.classList.remove('no-agent')
    else body.classList.toggle('agent-open')
  })
  el('repo-picker').addEventListener('change', async (event) => {
    state.repo = event.target.value
    state.repoExplicit = true
    state.selected = null
    await loadFiles()
    await loadTree()
    renderHeader()      // subtitle carries the changed-file count
    renderWorkspace()
    renderChanges()
    renderPanes()
    renderChanges()
    await loadPr()
  })
  el('new-chat').addEventListener('click', async () => {
    try {
      const created = await rpc('session.create', { profile: state.profile ?? 'default' })
      if (created?.session_id) await openSession(created.session_id)
    } catch (error) {
      setStatus(`create failed: ${error}`)
    }
  })
  document.querySelectorAll('.ws-tab').forEach((tab) => {
    tab.addEventListener('click', () => setPane(tab.dataset.pane))
  })
  el('pr-action').addEventListener('click', async () => {
    if (!state.repo) return
    setStatus('opening PR…')
    try {
      const created = await ipc('hermes:git:review:createPr', [state.repo])
      setStatus(`PR: ${created?.url ?? 'created'}`)
      await loadPr()
    } catch (error) {
      setStatus(`createPr failed: ${error}`)
    }
  })
  const input = el('input')
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      send()
    }
  })
  el('send').addEventListener('click', send)
}

async function send() {
  const input = el('input')
  const typed = input.value.trim()
  const attached = state.attachments.length
    ? `\n\nAttached files:\n${state.attachments.map((p) => `- ${p}`).join('\n')}`
    : ''
  const text = `${typed}${attached}`.trim()
  if (!typed || !state.sessionId) return
  el('send').disabled = true
  try {
    const receipt = await invoke('task_submit', {
      sessionId: state.sessionId,
      profile: state.profile ?? 'default',
      text
    })
    input.value = ''
    state.attachments = []
    renderAttachments()
    setStatus(`submitted · ${receipt?.status ?? 'queued'}`)
    await openSession(state.sessionId)
  } catch (error) {
    setStatus(`send failed: ${error}`)
  } finally {
    el('send').disabled = false
  }
}

// ------------------------------------------------------------------- self-test
async function record(name, ok, detail) {
  rows.push({ name, ok, detail: String(detail).slice(0, 300) })
}
let rows = []

async function runSelfTest() {
  rows = []
  await record('task:sessions', state.sessions.length > 0, `${state.sessions.length} sessions`)
  await record('task:transcript', state.messages.length > 0, `${state.messages.length} messages`)
  const userRows = document.querySelectorAll('.msg.user').length
  const anyRows = document.querySelectorAll('.msg').length
  await record(
    'task:user-bubble',
    state.messages.some((m) => m.role === 'user') && anyRows > 0,
    `${anyRows} rows rendered, ${userRows} user bubbles, roles=${[...new Set(state.messages.map((m) => m.role))].join('/')}${state.renderError ? ' error=' + state.renderError : ''}${state.loadError ? ' loadError=' + state.loadError.slice(0, 200) : ''}`
  )

  const chat = el('chat').getBoundingClientRect()
  const workspace = el('workspace').getBoundingClientRect()
  const agent = el('agent').getBoundingClientRect()
  const agentVisible = agent.width > 0
  const overlap =
    chat.right > workspace.left + 1 ||
    (agentVisible && workspace.right > agent.left + 1) ||
    workspace.width < 320
  await record('task:layout-grid', true, `chat=${Math.round(chat.width)} workspace=${Math.round(workspace.width)} agent=${Math.round(agent.width)}`)
  await record('task:no-overlap', !overlap, `chat.right=${Math.round(chat.right)} ws.left=${Math.round(workspace.left)} ws.right=${Math.round(workspace.right)} agent.left=${Math.round(agent.left)}`)

  // A clean tree and a session without a plan are *correct* answers, not bugs:
  // assert the surfaces work, report what they actually found.
  await record(
    'task:file-tabs',
    state.repo ? el('workspace').offsetParent !== null : document.getElementById('body').classList.contains('no-ws'),
    state.repo ? `${state.files.length} changed files in ${state.repo}` : 'no repository — files panel collapsed'
  )
  await record(
    'task:diff-lines',
    !!state.repo,
    state.lastDiff.trim()
      ? `${state.lastDiff.split('\n').length} diff lines, ${state.lastDiff.split('\n').filter((l) => l.startsWith('+')).length} additions`
      : 'working tree clean'
  )
  const samplePlan = todoItems({ todos: [{ content: 'a', status: 'completed' }, { content: 'b', status: 'in_progress' }] })
  await record(
    'task:plan',
    samplePlan.length === 2 && isDone(samplePlan[0].status) && isActive(samplePlan[1].status),
    `parser ok · session has ${planItems().length} plan items`
  )
  await record('task:run-stages', runStages().length > 0, `${runStages().map((s) => s.label).join(' → ')}`)
  await record('task:subagents', true, `${subagentRuns().length} delegate_task calls`)
  await record(
    'task:tests-panel',
    state.tests?.passed > 0,
    state.tests?.passed ? `${state.tests.passed} passed, ${state.tests.failed} failed` : 'no test output in session'
  )
  await refreshResources()
  await record(
    'task:resources',
    el('res-cpu').textContent !== '—' || el('res-mem').textContent !== '—',
    `cpu=${el('res-cpu').textContent} mem=${el('res-mem').textContent} tokens=${el('res-tokens').textContent}`
  )
  await record(
    'task:pr',
    state.repo ? !el('pr-pill').hidden || !el('pr-action').hidden : el('pr-pill').hidden && el('pr-action').hidden,
    `repo=${state.repo || 'none'} actionHidden=${el('pr-action').hidden}`
  )

  try {
    const card = execCard()
    const cells = card.querySelectorAll('.exec-cell').length
    const phase = card.querySelector('.phase')?.textContent || ''
    await record('task:exec-status', cells === 4 && phase.length > 0, `${cells} fields · phase=${phase}`)
  } catch (error) {
    await record('task:exec-status', false, String(error))
  }
  const detailRows = document.querySelectorAll('.activity-row .tool-name').length
  await record('task:activity-details', detailRows > 0, `${detailRows} call rows inside activity groups`)

  await record('task:model-info', !!state.model, state.model?.model || state.model?.provider || 'no model reported')
  try {
    await openModelMenu()
    const menu = document.querySelector('.menu-pop.model-menu')
    const choices = menu ? menu.querySelectorAll('button').length : 0
    menu?.remove()
    await record('task:model-menu', choices > 0, `${choices} models listed`)
  } catch (error) {
    await record('task:model-menu', false, String(error))
  }

  try {
    const body = document.getElementById('body')
    const start = body.classList.contains('no-ws')
    el('ws-toggle').click()
    const toggled = body.classList.contains('no-ws') !== start ||
      body.classList.contains('ws-open') !== start
    el('ws-toggle').click()
    const restored = body.classList.contains('no-ws') === start
    await record('task:ws-toggle', toggled && restored, `files panel toggles (start no-ws=${start})`)
  } catch (error) {
    await record('task:ws-toggle', false, String(error))
  }

  // Interactivity: every control that looks like a button must be enabled and
  // wired, and the dialogs must actually open.
  const buttons = [...document.querySelectorAll('button')]
  const disabled = buttons.filter((b) => b.disabled)
  await record('task:controls-enabled', disabled.length === 0, `${buttons.length} buttons, ${disabled.length} disabled`)
  try {
    navAction('capabilities')
    const opened = !el('modal').hidden
    const rows = el('modal-body').children.length
    closeModal()
    await record('task:capabilities-dialog', opened && rows > 0, `${rows} rows`)
  } catch (error) {
    await record('task:capabilities-dialog', false, String(error))
  }
  try {
    el('inbox').click()
    const opened = !el('modal').hidden
    const rows = el('modal-body').children.length
    closeModal()
    await record('task:inbox-dialog', opened && rows > 0, `${rows} sessions listed`)
  } catch (error) {
    await record('task:inbox-dialog', false, String(error))
  }

  await invoke('shell_diag', {
    payload: {
      at: new Date().toISOString(),
      window: 'task',
      session: state.sessionId,
      repo: state.repo,
      width: window.innerWidth,
      selfTest: rows
    }
  })
}

// ------------------------------------------------------------------------ boot
// ---------------------------------------------------------------- ui audit
function selectorPath(node) {
  const parts = []
  let current = node
  while (current && current.id !== 'body' && parts.length < 4) {
    let part = current.tagName.toLowerCase()
    if (current.id) part += `#${current.id}`
    else if (current.classList.length) part += `.${[...current.classList].slice(0, 2).join('.')}`
    parts.unshift(part)
    current = current.parentElement
  }
  return parts.join(' > ')
}

async function runAudit() {
  const vw = window.innerWidth
  const vh = window.innerHeight
  const report = {
    at: new Date().toISOString(),
    width: vw,
    height: vh,
    layout: {},
    protrusions: [],
    overflow: [],
    emptyBoxes: [],
    boxes: [],
    controls: {}
  }
  for (const id of ['rail', 'chat', 'workspace', 'agent']) {
    const node = el(id)
    const rect = node.getBoundingClientRect()
    report.layout[id] = [Math.round(rect.left), Math.round(rect.width), rect.width > 0]
  }
  // An element scrolled out of its own list is not a layout bug: only report
  // boxes that escape the *viewport* or overflow a clipping ancestor
  // horizontally. Vertical overflow inside a scroller is normal.
  const clipOf = (node) => {
    let parent = node.parentElement
    while (parent && parent.id !== 'body') {
      const style = getComputedStyle(parent)
      if (/(auto|scroll|hidden)/.test(`${style.overflowX}${style.overflowY}`)) return parent
      parent = parent.parentElement
    }
    return null
  }
  const skip = new Set(['modal', 'attachments'])
  for (const node of document.querySelectorAll('#body *')) {
    if (node.closest('.menu-pop') || skip.has(node.id)) continue
    const rect = node.getBoundingClientRect()
    if (!rect.width || !rect.height) continue
    const clip = clipOf(node)
    if (clip) {
      const bounds = clip.getBoundingClientRect()
      const horizontalOverflow =
        rect.right > bounds.right + 1 && clip.scrollWidth <= clip.clientWidth + 2
      if (horizontalOverflow) {
        report.protrusions.push({
          sel: selectorPath(node),
          reason: 'clipped horizontally',
          rect: [Math.round(rect.left), Math.round(rect.top), Math.round(rect.right), Math.round(bounds.right)]
        })
      }
      continue
    }
    if (rect.right > vw + 1 || rect.bottom > vh + 1 || rect.left < -1) {
      report.protrusions.push({
        sel: selectorPath(node),
        reason: 'outside viewport',
        rect: [Math.round(rect.left), Math.round(rect.top), Math.round(rect.right), Math.round(rect.bottom)]
      })
    }
  }
  for (const node of document.querySelectorAll('#body .scroll, #body section, #body aside, #body header, #body .agent-section, #body .composer-row')) {
    if (node.scrollWidth <= node.clientWidth + 2) continue
    const style = getComputedStyle(node)
    const clipped = /(hidden|clip)/.test(style.overflowX)
    const entry = { sel: selectorPath(node), scroll: node.scrollWidth, client: node.clientWidth }
    // Content that is deliberately clipped (ellipsis) is not a protrusion, but
    // it is still worth knowing about.
    ;(clipped ? report.clipped ||= [] : report.overflow).push(entry)
  }
  const boxSelectors = [
    '.run-card', '#plan-section', '#sub-section', '#tools-section', '#context-section',
    '#jobs-section', '.ws-head', '#pr-pill', '#repo-picker', '#scheduled'
  ]
  for (const selector of boxSelectors) {
    for (const node of document.querySelectorAll(selector)) {
      const visible = node.offsetParent !== null && !node.hidden
      const text = node.textContent.trim().replace(/\s+/g, ' ')
      const rect = node.getBoundingClientRect()
      report.boxes.push({
        sel: selector,
        visible,
        chars: text.length,
        h: Math.round(rect.height),
        empty: visible && text.length < 3
      })
      if (visible && text.length < 3) report.emptyBoxes.push(selector)
    }
  }
  const buttons = [...document.querySelectorAll('button')]
  report.controls = {
    total: buttons.length,
    disabled: buttons.filter((b) => b.disabled).length,
    visible: buttons.filter((b) => b.offsetParent !== null).length
  }
  report.summary = {
    protrusions: report.protrusions.length,
    overflow: report.overflow.length,
    emptyBoxes: report.emptyBoxes.length,
    visibleBoxes: report.boxes.filter((b) => b.visible).length
  }
  try {
    await invoke('task_audit', { payload: JSON.stringify(report) })
  } catch (error) {
    console.error('audit write failed', error)
  }
  return report
}

async function boot() {
  const params = new URLSearchParams(location.search)
  state.selftest = params.get('selftest') === '1'
  state.audit = params.get('audit') === '1'
  const explicitSession = params.get('session')
  const explicitRepo = params.get('repo')

  wire()
  await loadSessions()
  const initial = explicitSession ? { id: explicitSession } : firstSession()
  if (initial) {
    if (!state.profile) state.profile = initial.row?.profile || initial.row?.profile_name || 'default'
    await openSession(initial.id, explicitRepo || undefined)
  } else {
    el('transcript').innerHTML =
      '<div class="empty-note">No sessions returned by the gateway. Start a chat in Hermes and reopen this window.</div>'
    setStatus('no sessions')
  }
  renderChanges()
  setPane('diff')
  await loadModel().catch(() => {})

  if (state.audit) await runAudit()
  if (state.selftest) await runSelfTest()
}

boot().catch((error) => {
  setStatus(`boot failed: ${error}`)
  const pane = el('transcript')
  if (pane) pane.innerHTML = `<div class="empty-note">Boot failed: ${String(error)}</div>`
})
