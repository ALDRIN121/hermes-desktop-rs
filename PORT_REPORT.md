# Hermes Desktop → Rust: port report

Generated from REA evidence on the shipped artifact plus the Electron source tree
that produced it. Everything below was measured on this machine, not estimated.

## 1. What the shipped app actually is (REA)

`inspect_electron_fuses` on `Hermes.app` (Evidence `ev_ee8dc8e2…`):

| Fuse | State |
|---|---|
| RunAsNode | enabled |
| EnableNodeCliInspectArguments | enabled |
| EnableEmbeddedAsarIntegrityValidation | disabled |
| OnlyLoadAppFromAsar | disabled |
| GrantFileProtocolExtraPrivileges | enabled |

`analyze_javascript_application` on `Contents/Resources/app.asar`
(Evidence `ev_38ecdbcf…`, sha256 `4b87396e…`):

| Measure | Value |
|---|---|
| Files examined / total | 1,019 / 4,140 |
| JavaScript files parsed | 1,007 (37.4 MB of text, 0 parse failures) |
| Browser windows | 11 |
| Explicit webPreferences | 22 |
| contextBridge APIs | 1 (323 exposed members) |
| IPC operations | 536 |
| Literal channels | 281 |
| Main-process handlers | 199 |
| Renderer transmissions | 243 (197 paired, 45 unpaired literal) |
| Renderer listeners | 94 |

Source-side cross-check (`apps/desktop/electron/**/*.ts`): 281 distinct channels
(identical to REA's literal count), 216 `ipcMain` registrations, and a bridge
declared over 1,903 lines of `global.d.ts`.

Size of the thing being converted:

| Layer | Size |
|---|---|
| Renderer (`apps/desktop/src`) | 466,333 lines of TS/TSX across 3,714 files |
| Electron main (`apps/desktop/electron`) | 84,757 lines across 353 files |
| Native addons | node-pty only (plus small ObjC/C helpers for HUD/screenshots) |

## 2. The verdict

A line-for-line Rust rewrite of the renderer is not a task, it is a year: the UI
is 466k lines of React + Tailwind + xterm.js + CodeMirror + noVNC + streamdown.
Porting that to egui/slint would also throw away the design system and every
integration the web stack gives for free.

What *is* a Rust conversion, and what shipped here, is the privileged half — the
part Electron's main process owned:

- window creation
- the preload bridge (`window.hermesDesktop`, 323 members)
- backend discovery and the native grant channel
- the REST transport policy
- the gateway WebSocket
- the `hermes-media` custom protocol

That is `src-tauri` + `crates/hermes-contract`: **Rust replaces Node/Electron for
everything except the web page itself.**

## 3. What the Rust port does now (verified)

Run: `cd ~/Developer/hermes-desktop-rs && cargo run -p hermes-desktop-rust`

Verified against the live gateway on this machine:

- `hermes gateway attach` discovery → descriptor (`HermesConnection`) the
  renderer accepts
- native grant minting over the control socket (`~/.hermes/gateway.sock`,
  verb `session-ticket`, purpose `native-http`), so REST calls authenticate
- `hermes:api` REST proxy with the ported retry policy (idempotent verbs retry;
  mutating verbs only when provably never sent)
- gateway WebSocket dialled **in Rust** (`tokio-tungstenite`), ticket passed as
  the `hermes-gateway-ticket.*` subprotocol, messages relayed to the renderer
  over Tauri events
- the shipped React bundle boots unchanged and renders the real app: session
  list, bots, capabilities, messaging, artifacts, scheduled jobs, all populated
  from the live backend, zero console errors

Why the WebSocket is dialled from Rust: on macOS the page is `tauri://localhost`
and WebKit's ATS refuses every loopback `ws://`/`http://` dial from that origin
(WS closes 1006, `fetch` reports "Load failed"; the identical handshake succeeds
from Node). Electron never hit this because its pages are `file://` and the dial
happened in the main process. The Rust shell keeps that boundary.

Tests: `cargo test` → 31 passing (11 in `hermes-contract`: attach parsing,
descriptor wire shape, retry policy, control-socket mint, channel invariants;
20 in the shell: fs/dialog/clipboard handlers, a real PTY round trip, and git
against throwaway repositories).

End-to-end verification lives in the shell itself: `HERMES_SHELL_SELFTEST=1`
makes the injected bridge exercise the real channels through the same path the
UI uses, and report the results back to Rust. Last run: **17/17 passed**,
renderer errors 0:

```
fs:writeText · fs:readFileText · fs:readDir (90 entries) · fs:gitRoot · fs:trash
clipboard:read · dataUrlReadMax:get · readFileDataUrl
logs:line+recent (200 lines) · zoom:get
git:repoStatus · git:branchList (2 branches) · git:revParse · git:worktreeList
terminal:resize · terminal:cwd · terminal:start+exec (zsh ran `echo $((6*7))` → 42)
```

## 4. Port ledger

281 channels. They are not one kind of thing: **45 are push-only events** the
renderer subscribes to (`ipcRenderer.on`, never invokes) and **236 are requests**
that the shell either answers or refuses. Events are excluded from the porting
denominator — an event the shell never emits is a missing feature, not a
channel waiting to be implemented.

**77 of 236 request channels ported (32.6%)** — and the boot-and-data path plus
the day-to-day local surfaces are real Rust:

- backend: `hermes:connection*`, `hermes:gateway:ws-url*`, `hermes:api`
- boot: `hermes:bootstrap:get`, `hermes:boot-progress:get`, `hermes:local-backend:probe`
- preferences/skin/flags/window: `hermes:ui-preferences:*`, `hermes:feature-flags`,
  `hermes:skin:local`, `hermes:translucency:support`, `hermes:hud:windowing`,
  `hermes:version`, `hermes:window-control`, `hermes:zoom:get|set-percent`
- filesystem: `hermes:fs:*` (readDir, writeText, rename, trash, reveal, openDir,
  gitRoot, logsRoot, desktopPluginsRoot, reconcile), `hermes:readFileText`,
  `hermes:readFileDataUrl(ForAttach)`, `hermes:readPluginSource`
- dialogs + clipboard: `hermes:selectPaths`, `hermes:selectSavePath`,
  `hermes:saveTextFileAs`, `hermes:readClipboard`, `hermes:writeClipboard`,
  `hermes:saveImageBuffer`, `hermes:savePastedText`, `hermes:data-url-read-max:*`
- terminal: `hermes:terminal:start|attach|write|resize|cwd|dispose`
- git: all 21 channels — status, branches, worktrees, review list/diff,
  stage/unstage/revert, commit, push, rev-parse, commit context, repo scan, and
  the `gh`-backed ship info / PR list / PR create
- system: `hermes:logs:*`, `hermes:notify`, `hermes:keep-awake`

Every other request answers `unimplemented:<channel>` — a typed refusal, never a
silent success.

Remaining request groups (159 channels):

| Group | Channels | Work |
|---|---|---|
| Windows: session windows, browser pop-outs, HUD, pet overlay, quick entry | ~55 | per-window builders; Tauri supports multi-window |
| Updater / install / bootstrap / uninstall | ~28 | Rust updater, or keep the Python installer |
| Menus, tray, power, screenshots, native theme | ~25 | Tauri plugins + small native shims |
| Preview files, plugin file grants, MCP OAuth, plugins | ~30 | needs the permission/grant model |
| SSH + remote connection registry | ~20 | `russh`/`ssh` plus the registry stores |
| Everything else | ~21 | ordinary porting |

## 5. The workspace window (Codex-style surface)

`ui/codex.html` + `ui/codex.js` + `ui/codex.css` — a second Tauri window,
shell-owned UI, that pairs repository status with a diff and per-file git
actions. It calls the *same* Rust channels the renderer does (`hermes:git:*` via
`hermes_ipc`), so there is one implementation of every git operation.

Open it with `HERMES_WORKSPACE_WINDOW=1` (or `HERMES_WORKSPACE_REPO=<path>` to
pin a repository). It shows: repo picker (from `git:scanRepos`), branch /
ahead-behind / changed counts, changed-file list with `+n −m`, a colourised
unified diff, and Stage / Unstage / Revert.

Verified on a dirty repository by screen capture and the accessibility tree:
`fresh.txt A +1 −0`, `notes.md M +2 −1`, status `main ↑0 ↓0 · 2 changed · +3 −1`,
diff pane showing the `--no-index` diff of the untracked file.

Two real defects that only the rendered check could find, both fixed:

- untracked files rendered "No diff for this file" — `git diff --no-index`
  exits **1** when the files differ, and the runner treated any non-zero exit as
  failure;
- `repoStatus.added` was always 0 — the helper ran `git --numstat` instead of
  `git diff --numstat`.

Both now have tests (`untracked_files_diff_and_count_without_an_extra_line`,
plus the exact-count assertion that caught the second one).

## 6. Task view

`ui/task.html|css|js` — the shell-owned workspace window (`hermes-rust task`).
Sessions come from the REST proxy, transcripts and sends from canonical
JSON-RPC through `task_call`/`task_submit`; git, resources and PR state come from
the same Rust channels the renderer uses.

Self-test (`hermes-rust task --selftest`), 7/7 after the block landed: sessions
87, sidebar 60 rows, transcript 913 messages, user bubble present, tools listed,
resources `cpu 15% / mem 0.2 GB / tokens 12.3M`, PR row resolved for the picked
repository (`Open PR` shown because that branch has no PR).

Defects this block exposed, all fixed: the PR channels are `hermes:git:review:*`
(not `hermes:git:*`); `gateway.pid` is JSON, not a bare integer; `usage.calls`
resets per execution so the cumulative count comes from the transcript; a `.app`
launched from Finder has no `/opt/homebrew/bin` on PATH, so `gh` must be resolved
explicitly; and `prompt.submit` needs an attach on the *same* socket.

Still mockup-only: the plan block (no session here carries `todo_state`), cost
in dollars, and the "worked for N" summary line.

## 7. Files

```
hermes-desktop-rs/
├── Cargo.toml
├── crates/hermes-contract/      # 281-channel inventory, 45 events, types, discovery, policy
│   └── src/{channels,discovery,registry,transport,types}.rs
├── src-tauri/
│   ├── src/main.rs              # window, bootstrap script, hermes-media scheme
│   ├── src/commands.rs          # hermes_ipc dispatch, REST proxy, preferences
│   ├── src/gateway_ws.rs        # gateway WebSocket owned by Rust
│   ├── src/fs_ops.rs            # hermes:fs:*, preview reads, dialogs, clipboard
│   ├── src/git_ops.rs           # all 21 hermes:git:* channels
│   ├── src/system_ops.rs        # logs, zoom, keep-awake, notifications
│   ├── src/terminal.rs          # hermes:terminal:* on a real PTY
│   ├── src/bridge.js            # injected window.hermesDesktop + WS shim + self-test
│   └── tauri.conf.json
└── ui/                          # the shipped renderer bundle, unmodified
```
