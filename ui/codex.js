// Workspace window logic.
//
// This is shell-owned UI: it talks to the same Rust channels the Hermes
// renderer uses (`hermes:git:*` via `hermes_ipc`), so there is exactly one
// implementation of every git operation in the app.
//
// Nothing here is copied from another app's bundle: the layout is a plain
// two-pane review surface built from our own CSS.

const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args)
const call = (channel, args = []) => invoke('hermes_ipc', { channel, args })

const state = {
  repo: null,
  files: [],
  selected: null,
  status: null
}

const el = (id) => document.getElementById(id)

function setStatus(text) {
  el('status').textContent = text
}

function fmtCounts(added, removed) {
  return `${added > 0 ? '+' + added : ''}${removed > 0 ? ' −' + removed : ''}` || '±0'
}

async function pickRepo() {
  const params = new URLSearchParams(location.search)
  const explicit = params.get('repo')
  if (explicit) return explicit

  const repos = await discoverRepos()
  if (repos.length) return repos[0].root

  const home = await call('hermes:fs:gitRoot', ['~']).catch(() => null)
  return typeof home === 'string' ? home : null
}

async function discoverRepos() {
  const roots = ['~/Developer', '~/Projects', '~/code']
  const repos = await call('hermes:git:scanRepos', [roots, { maxDepth: 2 }]).catch(() => [])
  return Array.isArray(repos) ? repos : []
}

async function fillRepoPicker() {
  const picker = el('repo-picker')
  if (!picker) return
  const repos = await discoverRepos()
  picker.innerHTML = ''
  for (const repo of repos.slice(0, 40)) {
    const option = document.createElement('option')
    option.value = repo.root
    option.textContent = repo.label || repo.root
    option.selected = repo.root === state.repo
    picker.appendChild(option)
  }
  picker.disabled = repos.length === 0
}

async function refresh() {
  setStatus('refreshing…')
  if (!state.repo) state.repo = await pickRepo()
  if (!state.repo) {
    el('repo-label').textContent = 'no repository found'
    el('files').innerHTML = '<div class="empty">No git repository found under ~/Developer.</div>'
    setStatus('idle')
    return
  }

  el('repo-label').textContent = state.repo.replace(/^\/Users\/[^/]+/, '~')
  await fillRepoPicker()

  const [status, list] = await Promise.all([
    call('hermes:git:repoStatus', [state.repo]).catch(() => null),
    call('hermes:git:review:list', [state.repo, 'uncommitted', '']).catch(() => ({ files: [] }))
  ])

  state.status = status
  state.files = (list && list.files) || []

  el('branch-pill').textContent = status ? (status.detached ? 'detached' : status.branch || '—') : '—'
  el('sync-pill').textContent = status ? `↑${status.ahead || 0} ↓${status.behind || 0}` : '—'
  el('counts-pill').textContent = status
    ? `${status.changed} changed · ${fmtCounts(status.added || 0, status.removed || 0)}`
    : '—'

  renderFiles()
  if (!state.selected && state.files.length) selectFile(state.files[0].path)
  setStatus(`updated ${new Date().toLocaleTimeString()}`)
}

function renderFiles() {
  const list = el('files')
  if (!state.files.length) {
    list.innerHTML = '<div class="empty">Working tree clean.</div>'
    el('diff').textContent = ''
    return
  }
  list.innerHTML = ''
  for (const file of state.files) {
    const row = document.createElement('div')
    row.className = 'file' + (file.path === state.selected ? ' active' : '')
    row.innerHTML = `
      <span class="status">${file.status || 'M'}</span>
      <span class="name" title="${file.path}">${file.path}</span>
      <span class="counts"><span class="add">+${file.added || 0}</span><span class="del">−${file.removed || 0}</span></span>
    `
    row.addEventListener('click', () => selectFile(file.path))
    list.appendChild(row)
  }
}

async function selectFile(path) {
  state.selected = path
  renderFiles()
  setStatus(`diffing ${path}…`)
  const diff = await call('hermes:git:review:diff', [state.repo, path, 'uncommitted', '', false]).catch(() => '')
  renderDiff(typeof diff === 'string' ? diff : '')
  setStatus(path)
}

function renderDiff(text) {
  const target = el('diff')
  target.textContent = ''
  if (!text.trim()) {
    target.innerHTML = '<div class="empty">No diff for this file.</div>'
    return
  }
  const frag = document.createDocumentFragment()
  for (const raw of text.split('\n')) {
    const line = document.createElement('span')
    line.className = 'line'
    const body = document.createElement('span')
    if (raw.startsWith('+++') || raw.startsWith('---') || raw.startsWith('diff ') || raw.startsWith('index ')) {
      line.classList.add('meta')
      body.textContent = raw
    } else if (raw.startsWith('@@')) {
      line.classList.add('hunk')
      body.textContent = raw
    } else if (raw.startsWith('+')) {
      line.classList.add('add')
      body.textContent = raw
    } else if (raw.startsWith('-')) {
      line.classList.add('del')
      body.textContent = raw
    } else {
      body.textContent = raw
    }
    line.appendChild(body)
    line.appendChild(document.createTextNode('\n'))
    frag.appendChild(line)
  }
  target.appendChild(frag)
}

async function act(operation) {
  if (!state.repo) return
  const file = state.selected
  if (!file && operation !== 'stage') {
    setStatus('select a file first')
    return
  }
  setStatus(`${operation} ${file}…`)
  try {
    await call(`hermes:git:review:${operation}`, [state.repo, file])
    await refresh()
    if (state.selected) await selectFile(state.selected)
  } catch (error) {
    setStatus(`${operation} failed: ${error}`)
  }
}

el('btn-refresh').addEventListener('click', refresh)
el('btn-stage').addEventListener('click', () => act('stage'))
el('btn-unstage').addEventListener('click', () => act('unstage'))
el('btn-revert').addEventListener('click', () => act('revert'))
el('repo-picker').addEventListener('change', (event) => {
  state.repo = event.target.value
  state.selected = null
  state.files = []
  el('diff').textContent = ''
  refresh()
})

// --- optional self-test (HERMES_WORKSPACE_SELFTEST=1 opens with ?selftest=1) --
async function selfTest() {
  const results = []
  const record = (name, ok, detail) =>
    results.push({ name, ok, detail: String(detail === undefined ? '' : detail).slice(0, 220) })
  try {
    record('workspace:repo', !!state.repo, state.repo)
    const status = await call('hermes:git:repoStatus', [state.repo])
    record('workspace:status', !!(status && typeof status.changed === 'number'), JSON.stringify({ branch: status && status.branch, changed: status && status.changed }))
    const list = await call('hermes:git:review:list', [state.repo, 'uncommitted', ''])
    record('workspace:files', Array.isArray(list.files), (list.files || []).length + ' files')
    record('workspace:rendered', el('files').querySelectorAll('.file').length === (list.files || []).length, el('files').children.length + ' rows')
    if (state.selected) {
      const diff = await call('hermes:git:review:diff', [state.repo, state.selected, 'uncommitted', '', false])
      record('workspace:diff', typeof diff === 'string' && diff.length > 0, String(diff).slice(0, 60))
    } else {
      record('workspace:diff', true, 'clean tree — nothing to diff')
    }
  } catch (error) {
    record('workspace:exception', false, error)
  }
  await invoke('shell_diag', { payload: { at: new Date().toISOString(), window: 'workspace', selfTest: results } })
}

const params = new URLSearchParams(location.search)
refresh().then(() => {
  if (params.get('selftest') === '1') selfTest()
})
