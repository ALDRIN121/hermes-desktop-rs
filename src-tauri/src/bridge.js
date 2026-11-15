// Hermes desktop bridge, Rust side of `window.hermesDesktop`.
//
// Electron's preload exposes this object from a privileged context. Tauri has
// no preload, so the shell injects this script at document start (Tauri's
// `initialization_script`, the direct analogue of a preload) and forwards every
// call to one Rust command, `hermes_ipc`.
//
// Two rules shape this file:
//  1. Members the Rust shell does NOT implement are left `undefined`, not
//     stubbed with a rejecting function. The renderer already treats an absent
//     member as "older shell" (optional members are all `?.` guarded in
//     global.d.ts), so absence degrades features instead of breaking boot.
//     Event subscriptions (`on*`) are the exception: several are called
//     unguarded at module init, so they return an inert unsubscribe.
//  2. Loopback sockets are dialled in Rust. WebKit's App Transport Security
//     refuses `ws://127.0.0.1` from the `tauri://localhost` origin (verified:
//     WS closes 1006, `fetch` to loopback says "Load failed", while the same
//     handshake succeeds from Node), so `WebSocket` is replaced with a
//     lookalike that rides Tauri IPC.
(() => {
  const boot = __HERMES_BOOTSTRAP__;

  // Electron ran the renderer on Chromium; this shell runs it on WebKit, which
  // has no requestIdleCallback below Safari 16.4. One unguarded caller (the
  // Capabilities catalog warmer) otherwise blanks that whole view with
  // "Can't find variable: requestIdleCallback". Provide Chromium's shape: a
  // timer-based idle callback with the same handle/cancel contract.
  if (typeof window.requestIdleCallback !== 'function') {
    const handles = new Map();
    let next = 1;
    window.requestIdleCallback = callback => {
      const id = next++;
      const start = Date.now();
      handles.set(id, setTimeout(() => {
        if (!handles.delete(id)) {
          return;
        }
        callback({ didTimeout: false, timeRemaining: () => Math.max(0, 50 - (Date.now() - start)) });
      }, 1));
      return id;
    };
    window.cancelIdleCallback = id => {
      if (!handles.has(id)) {
        return;
      }
      const timer = handles.get(id);
      handles.delete(id);
      if (timer !== undefined) {
        clearTimeout(timer);
      }
    };
  }

  const invoke = (cmd, args) => {
    const core = window.__TAURI__ && (window.__TAURI__.core || window.__TAURI__)
    if (!core || typeof core.invoke !== 'function') {
      return Promise.reject(new Error('tauri core unavailable'))
    }
    return core.invoke(cmd, args)
  }

  const call = (channel, args) => invoke('hermes_ipc', { channel, args: args || [] })

  // Fire-and-forget sends (preload `ipcRenderer.send`).
  const send = (channel, args) => {
    call(channel, args).catch(() => undefined)
  }

  const unsubscribe = () => () => undefined

  // Electron channel names → Tauri events (the terminal's data/exit streams).
  const listen = (channel, callback) => {
    const events = window.__TAURI__ && window.__TAURI__.event
    if (!events || typeof events.listen !== 'function') return unsubscribe
    let removed = false
    let unlisten = null
    events
      .listen(channel, (event) => callback(event.payload))
      .then((fn) => {
        if (removed) fn()
        else unlisten = fn
      })
      .catch(() => undefined)
    return () => {
      removed = true
      if (unlisten) unlisten()
    }
  }

  const desktop = {
    // --- synchronous launch facts (Electron: preload sendSync) ---------------
    glassSupported: boot.glassSupported === true,
    translucencySupported: boot.translucencySupported === true,
    localModelsEnabled: boot.localModelsEnabled === true,
    assumeFreshInstall: boot.assumeFreshInstall === true,
    localSkin: boot.localSkin || null,

    uiPreferences: {
      snapshot: boot.uiPreferences || null,
      read: () => call('hermes:ui-preferences:read'),
      write: (payload) => call('hermes:ui-preferences:write', [payload]),
      // No cross-window preference bus in the Rust shell yet: this window's
      // own writes are applied locally, so the subscription is inert rather
      // than wrong.
      onChanged: unsubscribe
    },

    // --- backend resolution --------------------------------------------------
    getConnection: (profile) => call('hermes:connection', [profile === undefined ? null : profile]),
    getConnectionFor: (payload) => call('hermes:connection:for', [payload]),
    revalidateConnection: () => call('hermes:connection:revalidate'),
    getGatewayWsUrl: (profile) => call('hermes:gateway:ws-url', [profile === undefined ? null : profile]),
    getGatewayWsUrlFor: (payload) => call('hermes:gateway:ws-url-for', [payload]),
    getEmbedHostOrigin: () => call('hermes:embed-host:origin'),

    // --- REST transport ------------------------------------------------------
    api: (request) => call('hermes:api', [request]),

    // Boot surfaces. The Rust shell attaches to a running gateway, so boot is
    // always complete and no install is in flight.
    getBootProgress: () => call('hermes:boot-progress:get'),
    getBootstrapState: () => call('hermes:bootstrap:get'),
    probeLocalBackend: () => call('hermes:local-backend:probe'),

    // --- window controls -----------------------------------------------------
    windowControls: {
      custom: false,
      minimize: () => send('hermes:window-control', ['minimize']),
      toggleMaximize: () => send('hermes:window-control', ['toggle-maximize']),
      close: () => send('hermes:window-control', ['close'])
    },

    getVersion: (scope) => call('hermes:version', [scope]),

    // --- desktop surface -----------------------------------------------------
    // Links, machine facts, SSH and the gateway file bridge. Each maps to the
    // same channel the Electron main process registered, so the renderer's
    // existing call sites work unchanged.
    openExternal: (url) => call('hermes:openExternal', [url]),
    getMachineProfile: () => call('hermes:machine:profile'),
    sshResolveHost: (host) => call('hermes:ssh-config:resolve', [host]),
    settings: {
      getDefaultProjectDir: () => call('hermes:setting:defaultProjectDir:get'),
      setDefaultProjectDir: (dir) => call('hermes:setting:defaultProjectDir:set', [dir]),
      pickDefaultProjectDir: () => call('hermes:setting:defaultProjectDir:pick')
    },
    saveGatewayFile: (payload) => call('hermes:saveGatewayFile', [payload]),
    saveImageFromUrl: (url) => call('hermes:saveImageFromUrl', [url]),
    normalizePreviewTarget: (target, baseDir) => call('hermes:normalizePreviewTarget', [target, baseDir]),
    watchPreviewFile: (url) => call('hermes:watchPreviewFile', [url]),
    repairBootstrap: () => call('hermes:bootstrap:repair'),
    preparedSubmissions: { read: () => call('hermes:prepared-submissions:read') },
    themes: {
      fetchMarketplace: (id) => call('hermes:vscode-theme:fetch', [id]),
      searchMarketplace: (query) => call('hermes:vscode-theme:search', [query])
    },

    // Staging a dropped file is a bytes operation the renderer can do itself
    // (it already holds the File), but the Electron shape returns the staged
    // path. Announce absence rather than a path that was never written.
    getPathForFile: () => '',

    // --- filesystem, dialogs, clipboard --------------------------------------
    readFileText: (filePath) => call('hermes:readFileText', [filePath]),
    readFileDataUrl: (filePath) => call('hermes:readFileDataUrl', [filePath]),
    readFileDataUrlForAttach: (filePath) => call('hermes:readFileDataUrlForAttach', [filePath]),
    readPluginSource: (filePath) => call('hermes:readPluginSource', [filePath]),
    selectPaths: (options) => call('hermes:selectPaths', [options || {}]),
    selectSavePath: (options) => call('hermes:selectSavePath', [options || {}]),
    saveTextFileAs: (request) => call('hermes:saveTextFileAs', [request]),
    readClipboard: () => call('hermes:readClipboard'),
    writeClipboard: (text) => call('hermes:writeClipboard', [text]),
    saveImageBuffer: (data, ext, name) => call('hermes:saveImageBuffer', [{ data, ext, name }]),
    savePastedText: (text) => call('hermes:savePastedText', [{ text }]),
    dataUrlReadMax: {
      get: () => call('hermes:data-url-read-max:get'),
      set: (maxMb) => call('hermes:data-url-read-max:set', [maxMb])
    },
    readDir: (dirPath) => call('hermes:fs:readDir', [dirPath]),
    gitRoot: (startPath) => call('hermes:fs:gitRoot', [startPath]),
    revealPath: (targetPath) => call('hermes:fs:reveal', [targetPath]),
    openDir: (dirPath) => call('hermes:fs:openDir', [dirPath]),
    desktopPluginsRoot: () => call('hermes:fs:desktopPluginsRoot'),
    reconcileDesktopPlugins: () => call('hermes:fs:reconcileDesktopPlugins').then((r) => r.published),
    reconcileDesktopPluginsReport: () => call('hermes:fs:reconcileDesktopPlugins'),
    logsRoot: (profile) => call('hermes:fs:logsRoot', [profile]),
    renamePath: (targetPath, newName) => call('hermes:fs:rename', [targetPath, newName]),
    writeTextFile: (filePath, content) => call('hermes:fs:writeText', [filePath, content]),
    trashPath: (targetPath) => call('hermes:fs:trash', [targetPath]),

    // --- git -----------------------------------------------------------------
    git: {
      worktreeList: (repoPath) => call('hermes:git:worktreeList', [repoPath]),
      worktreeAdd: (repoPath, options) => call('hermes:git:worktreeAdd', [repoPath, options]),
      worktreeRemove: (repoPath, worktreePath, options) =>
        call('hermes:git:worktreeRemove', [repoPath, worktreePath, options]),
      branchSwitch: (repoPath, branch) => call('hermes:git:branchSwitch', [repoPath, branch]),
      branchList: (repoPath) => call('hermes:git:branchList', [repoPath]),
      baseBranchList: (repoPath) => call('hermes:git:baseBranchList', [repoPath]),
      repoStatus: (repoPath) => call('hermes:git:repoStatus', [repoPath]),
      fileDiff: (repoPath, filePath) => call('hermes:git:fileDiff', [repoPath, filePath]),
      scanRepos: (roots, options) => call('hermes:git:scanRepos', [roots, options]),
      review: {
        list: (repoPath, scope, baseRef) => call('hermes:git:review:list', [repoPath, scope, baseRef]),
        diff: (repoPath, filePath, scope, baseRef, staged) =>
          call('hermes:git:review:diff', [repoPath, filePath, scope, baseRef, staged]),
        stage: (repoPath, filePath) => call('hermes:git:review:stage', [repoPath, filePath]),
        unstage: (repoPath, filePath) => call('hermes:git:review:unstage', [repoPath, filePath]),
        revert: (repoPath, filePath) => call('hermes:git:review:revert', [repoPath, filePath]),
        revParse: (repoPath, ref) => call('hermes:git:review:revParse', [repoPath, ref]),
        commit: (repoPath, message, push) => call('hermes:git:review:commit', [repoPath, message, push]),
        commitContext: (repoPath) => call('hermes:git:review:commitContext', [repoPath]),
        push: (repoPath) => call('hermes:git:review:push', [repoPath]),
        shipInfo: (repoPath) => call('hermes:git:review:shipInfo', [repoPath]),
        prList: (repoPath) => call('hermes:git:review:prList', [repoPath]),
        createPr: (repoPath) => call('hermes:git:review:createPr', [repoPath])
      }
    },

    // --- logs, zoom, keep-awake, notifications --------------------------------
    revealLogs: () => call('hermes:logs:reveal'),
    getRecentLogs: () => call('hermes:logs:recent'),
    reportRendererError: (report) => send('hermes:logs:renderer-error', [report]),
    logLine: (line) => send('hermes:logs:renderer-line', [line]),
    notify: (payload) => call('hermes:notify', [payload]),
    setKeepAwake: (mode) => send('hermes:keep-awake', [mode]),
    keepAwakeOffer: unsubscribe,
    zoom: {
      get: () => call('hermes:zoom:get'),
      setPercent: (percent) => send('hermes:zoom:set-percent', [percent]),
      factor: () => zoomFactor,
      onChanged: (callback) => listen('hermes:zoom:changed', (payload) => {
        zoomFactor = payload.percent / 100
        callback(payload)
      })
    },

    // --- embedded terminal ---------------------------------------------------
    terminal: {
      start: (options) => call('hermes:terminal:start', [options || {}]),
      attach: (id) => call('hermes:terminal:attach', [id]),
      write: (id, data) => call('hermes:terminal:write', [id, data]),
      resize: (id, size) => call('hermes:terminal:resize', [id, size || {}]),
      cwd: (id) => call('hermes:terminal:cwd', [id]),
      dispose: (id) => call('hermes:terminal:dispose', [id]),
      onData: (id, callback) => listen('hermes:terminal:' + id + ':data', callback),
      onExit: (id, callback) => listen('hermes:terminal:' + id + ':exit', callback)
    },

    // Shell-level introspection (Rust addition; the renderer ignores it).
    rustShell: { info: () => invoke('shell_info', {}) }
  }

  let zoomFactor = 1

  // Anything not implemented here reads as absent, never as a broken stub —
  // except event subscriptions (`on*`), which Electron always provided and the
  // renderer calls unguarded at module init.
  window.hermesDesktop = new Proxy(desktop, {
    get(target, key) {
      if (key in target) return target[key]
      if (typeof key === 'string' && key.startsWith('on')) return unsubscribe
      if (key === 'then' || key === Symbol.toStringTag) return undefined
      return undefined
    },
    has(target, key) {
      return key in target
    }
  })

  // --- startup diagnostics -------------------------------------------------
  // The shell has no shipped devtools channel yet, so it reports what the
  // renderer actually did (did the bundle mount? did anything throw?) back to
  // Rust. This is how the port is verified without a screenshot.
  const errors = []
  window.__hermesErrors = errors
  window.addEventListener('error', (e) => {
    if (e && e.target && e.target !== window) {
      const target = e.target
      errors.push('resource: ' + (target.src || target.href || target.tagName))
    } else {
      errors.push(String((e && e.message) || e))
    }
  }, true)
  window.addEventListener('unhandledrejection', (e) => errors.push('rejection: ' + String(e.reason)))
  const origError = console.error
  console.error = function (...parts) {
    try {
      errors.push('console: ' + parts.map((p) => (p && p.message) || String(p)).join(' ').slice(0, 400))
    } catch {}
    return origError.apply(console, parts)
  }

  // ---------------------------------------------------------------------------
  // WebSocket over IPC (loopback only).
  // ---------------------------------------------------------------------------
  const socketLog = []
  window.__hermesSocketLog = socketLog
  const NativeWS = window.WebSocket
  const redact = (value) => String(value || '').replace(/ticket=[^&]*/g, 'ticket=<redacted>')
  const isLoopback = (url) => /^wss?:\/\/(127\.0\.0\.1|localhost|\[::1\])(:|\/|$)/i.test(String(url))

  class IpcSocket extends EventTarget {
    constructor(url, protocols) {
      super()
      this.url = String(url)
      this.readyState = 0 // CONNECTING
      this.protocol = ''
      this.bufferedAmount = 0
      this.extensions = ''
      this.onopen = null
      this.onmessage = null
      this.onclose = null
      this.onerror = null
      this._id = null
      this.binaryType = 'blob'
      this._closed = false
      this._unlisten = []
      this._pendingFrames = []
      this._outgoing = Promise.resolve()
      this._entry = { url: redact(url), protocols: protocols ? [].concat(protocols).map(redact) : null, events: [] }
      socketLog.push(this._entry)
      this._connect(protocols ? [].concat(protocols) : [])
    }

    async _connect(protocols) {
      try {
        await this._subscribe()
        if (this._closed) return
        const id = await invoke('hermes_ws_open', { url: this.url, protocols })
        this._id = id
        if (this._closed) {
          await invoke('hermes_ws_close', { id }).catch(() => undefined)
          return
        }
        this.readyState = 1
        this._entry.events.push('open')
        this._fire('open', new Event('open'))
        const pending = this._pendingFrames
        this._pendingFrames = []
        for (const [kind, payload] of pending) this._receive(kind, payload)
      } catch (error) {
        if (this._closed) return
        this._entry.events.push('error')
        this._fire('error', new Event('error'))
        this._finishClose(1006, 'Gateway connection failed')
      }
    }

    async _subscribe() {
      const events = window.__TAURI__ && window.__TAURI__.event
      if (!events) throw new Error('tauri events unavailable')
      for (const [channel, kind] of [['hermes-ws-message', 'message'], ['hermes-ws-closed', 'close']]) {
        const unlisten = await events.listen(channel, event => {
          if (this._closed) return
          const payload = event.payload || {}
          if (this._id === null) {
            // A native socket may produce a greeting before its open IPC resolves.
            // Keep this bounded across concurrent sockets; overflow fails visibly.
            if (this._pendingFrames.length >= 256) {
              this._fire('error', new Event('error'))
              this._finishClose(1006, 'Gateway startup buffer exceeded')
            } else this._pendingFrames.push([kind, payload])
          } else this._receive(kind, payload)
        })
        if (this._closed) unlisten()
        else this._unlisten.push(unlisten)
      }
    }

    _receive(kind, payload) {
      if (this._closed || payload.id !== this._id) return
      if (kind === 'close') {
        this._finishClose(payload.code || 1006, payload.reason || '')
        return
      }
      let data = payload.data
      if (payload.binary) {
        const bytes = new Uint8Array(data)
        data = this.binaryType === 'arraybuffer' ? bytes.buffer : new Blob([bytes])
      }
      this._entry.events.push('message')
      this._fire('message', new MessageEvent('message', { data }))
    }

    _finishClose(code, reason) {
      if (this._closed) return
      this._closed = true
      this.readyState = 3
      for (const unlisten of this._unlisten) unlisten()
      this._unlisten = []
      this._pendingFrames = []
      this._fire('close', new CloseEvent('close', { code, reason, wasClean: code === 1000 }))
    }

    _fire(type, event) {
      const handler = this['on' + type]
      if (typeof handler === 'function') {
        try {
          handler.call(this, event)
        } catch (error) {
          errors.push('handler ' + type + ': ' + error)
        }
      }
      this.dispatchEvent(event)
    }

    send(data) {
      if (this.readyState === 0) throw new Error('WebSocket is still connecting')
      if (this.readyState !== 1 || this._id === null) return
      const id = this._id
      // WebSocket.send takes ownership of the bytes now; callers may reuse
      // their PCM buffer before the asynchronous IPC queue drains.
      if (ArrayBuffer.isView(data)) {
        data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice().buffer
      } else if (data instanceof ArrayBuffer) data = data.slice(0)
      else if (!(data instanceof Blob)) data = String(data)
      // Blob conversion is asynchronous: queue every frame to retain send order.
      this._outgoing = this._outgoing.then(async () => {
        if (this._closed) return
        let binary = false
        if (data instanceof Blob) data = await data.arrayBuffer()
        if (data instanceof ArrayBuffer) {
          data = Array.from(new Uint8Array(data)); binary = true
        } else if (ArrayBuffer.isView(data)) {
          data = Array.from(new Uint8Array(data.buffer, data.byteOffset, data.byteLength)); binary = true
        } else data = String(data)
        await invoke('hermes_ws_send', { id, data, binary })
      }).catch(() => {
        if (!this._closed) this._fire('error', new Event('error'))
      })
    }

    close() {
      if (this._closed || this.readyState === 2) return
      if (this._id === null) {
        this._finishClose(1000, '')
        return
      }
      this.readyState = 2
      // A local close follows all accepted sends, including Blob conversion.
      // A peer close still retires immediately through _finishClose.
      this._outgoing = this._outgoing.then(async () => {
        if (this._closed) return
        await invoke('hermes_ws_close', { id: this._id }).catch(() => undefined)
        this._finishClose(1000, '')
      })
    }
  }

  function HermesWebSocket(url, protocols) {
    if (isLoopback(url)) return new IpcSocket(url, protocols)
    return new NativeWS(url, protocols)
  }
  HermesWebSocket.CONNECTING = 0
  HermesWebSocket.OPEN = 1
  HermesWebSocket.CLOSING = 2
  HermesWebSocket.CLOSED = 3
  HermesWebSocket.prototype = NativeWS.prototype
  window.WebSocket = HermesWebSocket

  const report = () => {
    const root = document.getElementById('root')
    const payload = {
      at: new Date().toISOString(),
      title: document.title,
      url: location.href,
      readyState: document.readyState,
      rootChildren: root ? root.childElementCount : -1,
      text: (document.body.innerText || '').slice(0, 1500),
      errors: errors.slice(0, 12),
      sockets: socketLog.slice(-8)
    }
    pending.then((selfTest) => {
      if (selfTest) payload.selfTest = selfTest
      invoke('shell_diag', { payload }).catch(() => undefined)
    })
  }
  // --- optional self-test --------------------------------------------------
  // HERMES_SHELL_SELFTEST=1 exercises the filesystem, clipboard and terminal
  // channels through this same bridge and reports the results to Rust, so the
  // port can be verified end-to-end without driving the UI by hand.
  const selfTest = async () => {
    const results = []
    const record = (name, ok, detail) =>
      results.push({ name, ok, detail: String(detail === undefined ? '' : detail).slice(0, 240) })

    try {
      const file = '~/.hermes/desktop-shell-selftest.txt'
      const written = await desktop.writeTextFile(file, 'selftest-payload')
      record('fs:writeText', !!(written && written.path), JSON.stringify(written))
      const read = await desktop.readFileText(file)
      record('fs:readFileText', !!(read && read.text === 'selftest-payload'), JSON.stringify(read).slice(0, 160))
      const dir = await desktop.readDir('~/.hermes')
      record('fs:readDir', Array.isArray(dir.entries) && dir.entries.length > 0, (dir.entries || []).length + ' entries')
      const root = await desktop.gitRoot('~/.hermes/hermes-agent')
      record('fs:gitRoot', typeof root === 'string' && root.length > 0, String(root))
      const removed = await desktop.trashPath(file)
      record('fs:trash', removed === true, String(removed))
      const clip = await desktop.readClipboard()
      record('clipboard:read', typeof clip === 'string', clip.slice(0, 40))
      const cap = await desktop.dataUrlReadMax.get()
      record('dataUrlReadMax:get', !!(cap && typeof cap.maxMb === 'number'), JSON.stringify(cap))
      const dataUrl = await desktop.readFileDataUrl('~/.hermes/hermes-agent/README.md')
      record('readFileDataUrl', typeof dataUrl === 'string' && dataUrl.startsWith('data:'), String(dataUrl).slice(0, 40))

      const marker = 'shell-selftest-' + Date.now()
      await desktop.logLine(marker)
      const logs = await desktop.getRecentLogs()
      record('logs:line+recent', Array.isArray(logs) && logs.some((line) => String(line).includes(marker)), (logs || []).length + ' lines')

      const zoom = await desktop.zoom.get()
      record('zoom:get', !!(zoom && typeof zoom.percent === 'number'), JSON.stringify(zoom))

      const repo = '~/.hermes/hermes-agent'
      const status = await desktop.git.repoStatus(repo)
      // The installed Hermes tree is a pinned, detached HEAD, so a null branch
      // with detached:true is the correct answer there.
      record(
        'git:repoStatus',
        !!(status && (typeof status.branch === 'string' || status.detached === true)),
        JSON.stringify({ branch: status && status.branch, detached: status && status.detached, changed: status && status.changed })
      )
      const branches = await desktop.git.branchList(repo)
      record('git:branchList', Array.isArray(branches) && branches.length > 0, (branches || []).length + ' branches')
      const head = await desktop.git.review.revParse(repo, 'HEAD')
      record('git:revParse', typeof head === 'string' && head.length === 40, String(head).slice(0, 12))
      const worktrees = await desktop.git.worktreeList(repo)
      record('git:worktreeList', Array.isArray(worktrees), (worktrees || []).length + ' worktrees')
    } catch (error) {
      record('fs:exception', false, error)
    }

    try {
      const started = await desktop.terminal.start({ cols: 80, rows: 24 })
      let output = ''
      const stop = desktop.terminal.onData(started.id, (chunk) => {
        output += chunk
      })
      await new Promise((resolve) => setTimeout(resolve, 300))
      const resized = await desktop.terminal.resize(started.id, { cols: 100, rows: 30 })
      const where = await desktop.terminal.cwd(started.id)
      // `$((6*7))` never appears as "42" in the echoed input line, so seeing 42
      // proves the shell EXECUTED the command, not just that the PTY echoed.
      desktop.terminal.write(started.id, 'echo $((6*7)); exit\r')
      for (let i = 0; i < 40 && !output.includes('42'); i++) {
        await new Promise((resolve) => setTimeout(resolve, 150))
      }
      stop()
      desktop.terminal.dispose(started.id)
      record('terminal:resize', resized === true, String(resized))
      record('terminal:cwd', typeof where === 'string' && where.length > 0, String(where))
      record(
        'terminal:start+exec',
        output.includes('42'),
        JSON.stringify({ id: started.id, shell: started.shell, seen: output.slice(-160) })
      )
    } catch (error) {
      record('terminal:exception', false, error)
    }

    return results
  }
  const pending = boot.selfTest === true ? selfTest() : Promise.resolve(null)

  if (boot.selfTest === true) {
    setTimeout(report, 5000)
    setTimeout(report, 15000)
  }

  window.__hermesRustShell = true
})()
