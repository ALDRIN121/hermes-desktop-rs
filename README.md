# hermes-desktop-rs

A Rust (Tauri) shell for the Hermes desktop app.

The source-built Hermes UI runs inside the native Tauri application. Normal
startup opens the unified Hermes window; it does not launch Chrome. The privileged half
that Electron's main process used to own is Rust: window creation, the
`window.hermesDesktop` bridge, local backend discovery, native grant minting,
the REST proxy, the gateway WebSocket and the `hermes-media` protocol.

The Hermes gateway remains Python and presentation uses React/TypeScript inside
the application's embedded WebView. This is not an all-Rust frontend rewrite.
See [implementation progress](docs/implementation/PROGRESS.md),
[native UI verification](docs/implementation/NATIVE_UI_REVIEW.md) and
[PORT_REPORT.md](PORT_REPORT.md) for the current limits. Full parity is in progress.

## Run

```bash
# Node >=26 and the pinned pnpm version are required for renderer builds.
# Install renderer dependencies as described in renderer/README.md first.
node scripts/build_renderer.mjs
cargo run -p hermes-desktop-rust
```

Requires a running Hermes gateway (`hermes gateway ensure`) — the shell attaches
to it and never installs one.

## Menu bar and the `hermes-rust` command

```bash
./scripts/bundle.sh debug          # → ~/Applications/Hermes Rust.app
install -m 755 scripts/hermes-rust ~/.local/bin/hermes-rust

hermes-rust                        # open / focus the chat window
hermes-rust workspace              # open the repository review window
hermes-rust task                   # open the task view (Codex-style layout)
```

The app puts a template **H** icon in the macOS menu bar with
**Open Hermes / Workspace review / Legacy task preview / Quit**. Closing the chat window hides it
instead of quitting, so the menu-bar icon is the way back in — left-click the
icon, or run `hermes-rust` again.

`hermes-rust` launches through `open -n` so a second invocation reaches the
running process (the single-instance plugin forwards argv and the new process
exits); there is never a second copy of the app.

## Test

```bash
cargo test --workspace                  # contract + shell, including local socket/PTY fixtures
node --test tests/ui/bridge-socket.test.mjs
HERMES_SHELL_SELFTEST=1 cargo run -p hermes-desktop-rust
cat "$TMPDIR/hermes-rust-diag.json"     # per-channel self-test results
```

## Legacy task preview (`hermes-rust task`)

This earlier shell-owned prototype is an explicit comparison view. The main
application is the source-built full Hermes renderer. The approved redesign and
remaining acceptance work are in [the full plan](docs/superpowers/plans/2026-11-15-full-hermes-plan.md).

Rebuilt as a four-column workspace: top bar (nav pills, repo, branch, status),
left rail (new chat, search, pinned, sessions, scheduled jobs, capabilities,
profile), chat column (messages, plan card, execution card, change bar,
composer), workspace (file tabs with M/A badges, breadcrumb, code with line
numbers, Diff/Tests/Terminal/Logs panes) and an agent rail (goal, plan, tools
used, context tree, active jobs, run stats, changes, PR).

Flags: `--session <id>` `--repo <path>` `--width 1024 --height 700` (breakpoint
reproduction) `--selftest`. Layout contract: 1440+ 264/452/320, ≤1439
216/380/288, ≤1279 icon rail, ≤1099 agent rail hidden behind the ▤ toggle.

A shell-owned workspace window over the live gateway — the Codex-style layout:

- **Header** — project, branch, model, live running dot, elapsed time, progress
  bar, and a PR row (`#12 · open`, or **Open PR** when the branch has none).
- **Plan** — `todo_state`, or the last todo-tool call when the session has one.
- **Chat / Run log** — prose view and a dense execution view of the same turn.
- **Outputs card** — run stats, resources (gateway CPU/RSS from `gateway.pid`,
  tokens from `usage`), context meter with cache %, subagents parsed from
  `delegate_task` arguments, tools, changed files for the repo, and sources.
- **Composer** — submits through `task_submit`, which attaches and submits on one
  socket (`prompt.submit` is attach-required) and then polls while `running`.

## Layout

- `crates/hermes-contract` — channel inventory (281 channels, generated from the
  Electron source), `HermesConnection`/`HermesApiRequest` wire types, `gateway
  attach` discovery, control-socket ticket minting, the HTTP retry policy, and
  the port ledger.
- `src-tauri/src/commands.rs` — one `hermes_ipc` command dispatching on the same
  channel names Electron registered. Unported channels fail with
  `unimplemented:<channel>`.
- `src-tauri/src/gateway_ws.rs` — the gateway WebSocket, dialled from Rust
  because WebKit refuses loopback `ws://` from the `tauri://` origin.
- `src-tauri/src/fs_ops.rs` — `hermes:fs:*`, preview reads, data-URL cap,
  composer attachments, dialogs (`rfd`) and clipboard (`arboard`).
- `src-tauri/src/terminal.rs` — `hermes:terminal:*` on a real PTY
  (`portable-pty`), events relayed as `hermes:terminal:<id>:data|exit`.
- `src-tauri/src/bridge.js` — injected at document start; builds
  `window.hermesDesktop`, the IPC-backed `WebSocket` for loopback URLs, and the
  optional self-test harness.
