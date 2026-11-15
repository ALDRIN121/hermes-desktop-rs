# Phases 6–11: work report

Written 15 November 2026. Branch `codex/full-hermes`, ten local commits, nothing
pushed. This records what was actually built and verified in this stretch, and
what remains open. It is not a completion claim for Phases 6–11.

## The failure mode this stretch was about

The renderer treats an absent `window.hermesDesktop` member as "this is an older
shell" — every optional member is `?.`-guarded in `global.d.ts`, and the
deliberate design in `bridge.js` is that unimplemented members stay `undefined`
rather than throwing. That is the right call for boot resilience, but it means
an unimplemented member does not fail. It **deletes a feature and leaves the UI
looking plausible**.

The concrete consequence: an audit found 24 members the renderer calls that the
Rust shell never defined. Links did not open. The locale probe never resolved.
SSH aliases could not resolve. Media had no save path. Previews could not be
classified. None of it produced an error anywhere — the affordances simply
were not drawn, or drew and did nothing.

Most of this stretch was finding those silences and giving them real answers.

## What was built

### All settings catalogue — `2a6ba90`

`Settings → Advanced & diagnostics → All settings`. Lists **every leaf of the
active profile's config record**, not just the keys a curated page happens to
surface: dotted paths in yaml order, grouped by top-level section, each row
labelled with its path and type. Search matches key paths *and* rendered values
(`MAX_TOKEN`, `terminal`), with a live "N of M settings" counter.

Editing is typed and in place: booleans reuse the same `Switch` row the curated
pages use, numbers parse and reject empty/NaN, strings commit on blur or Enter,
arrays and objects edit as JSON that refuses to save when it will not parse.

A save writes `diffConfig` of exactly the changed path through
`saveHermesConfigRecord`, so the backend's deep merge only ever sees one key,
then invalidates the shared config cache — every other settings page repaints.
The page honours the shared "Applies to" scope, so it edits whichever profile
the rest of Settings is pointed at.

Wiring was three lines: view id, `SUBPAGE_VIEWS` entry, nav item. The `advanced`
nav group already swept unknown page ids into itself.

**Evidence:** 3 unit tests (flatten including empty sections/arrays/nulls,
section grouping, search over keys and values). `src/app/settings` suite: 81
files, 625 tests passed. `tsc -p . --noEmit` clean. ESLint 0 errors. Rendered in
the bundled app: nav entry present, "Editing profile: default", "18 of 18
settings", grouped rows with number, boolean and string editors.

### The desktop surface — `2c43df3`

New `src-tauri/src/desktop_ops.rs`. Implemented, each mirroring the Electron
main-process behaviour it replaces:

| Member | Behaviour |
|---|---|
| `openExternal` | Web schemes only (`http`, `https`, `mailto`, `tel`). `javascript:` and `file:` refused; whitespace/control characters refused so a URL cannot smuggle a second argv. |
| `getMachineProfile` | `ageDays` from home birthtime, `arch`, `locale` from `LANG` with `_`→`-`, `platform`, `release` from `uname -r`, `username`. |
| `sshResolveHost` | `ssh -G <host>`. This is the only correct answer: it applies `Include`s, `Match` blocks and the system config in one pass. Hand-parsing the file answers a different question than ssh itself. Option-lookalike hosts (`-oProxyCommand=…`) refused. |
| `settings.get/set/pickDefaultProjectDir` | Round-trips through the same `desktop-ui.json` preferences file the renderer's own UI preferences use — one file describes one user's shell. |
| `saveGatewayFile` | Asks for a destination, streams into a sibling `.part`, renames only on success. A failed download never truncates a file already at the destination. |
| `saveImageFromUrl` | http(s) only; extension follows the URL's own path so the dialog suggests the right filter. |
| `normalizePreviewTarget` | Classifies `url` / `file` and `missing`/`directory`/`image`/`pdf`/`html`/`text`, with size and render mode. Answers "what is at this path" and lets the renderer decide how to show it. |
| `watchPreviewFile` | Stable id plus modification stamp. |
| `repairBootstrap` | Honest no-op: this shell attaches to a gateway it did not install, so there is nothing to repair. |

**A test caught me inventing things.** I declared three channels ported
(`hermes:gateway-file:stream-url`, `hermes:readGatewayFile`,
`hermes:themes:*`) that do not exist in the shipped inventory. The port-ledger
test `every_ported_channel_exists_in_the_inventory` failed and named them. I
removed the fabricated channels and the two functions with no shipped channel
backing them, rather than widening the ledger to match my guesses.

**Evidence:** 6 unit tests. Shell tests 30 → 36 at that commit. Bridge tests
gained a test asserting every newly-present member routes to its channel and
that `openExternal` forwards the URL verbatim.

### Media byte ranges and a measured startup baseline — `b2ec7c6`

**The defect.** `hermes-media://stream/` read the whole file and answered 200
with no `Range` support. WebKit's `<audio>`/`<video>` seek only when the server
answers 206 with `accept-ranges`; a player that cannot seek cannot play a voice
clip past its first chunk. And every media play loaded the entire file into
memory.

Now: `bytes=start-end`, `bytes=start-` and `bytes=-suffix` all answer 206 with
`content-range` and the exact span; an end past the file is clamped, not
widened; an unsatisfiable, reversed or malformed range answers **416 with
`content-range: bytes */total`** rather than silently returning everything (which
would mis-seat the player's cursor); only the requested span is read from disk.

**Phase 11 had no measurements at all.** `scripts/measure-startup.sh` plus an
`on_page_load` → `Finished` stamp inside the shell produces real numbers, and
RSS is summed across the process tree because WebKit's renderer is a separate
pid and a shell-only number understates the footprint.

| Run | First paint | Settled | RSS |
|-----|-------------|---------|-----|
| 1 | 2,124 ms | 7,340 ms | 91.1 MB |
| 2 | 979 ms | 5,196 ms | 99.8 MB |
| 3 | 1,441 ms | 5,731 ms | 116.5 MB |
| **Median** | **1,441 ms** | 5,731 ms | 99.8 MB |

Budgets set from those numbers and written down in `PERFORMANCE.md`: first paint
2,500 ms, settled 9,000 ms, RSS 200 MB.

**Evidence:** 10 new tests covering whole-file, bounded, open-ended, suffix,
clamped, overrun, reversed, malformed, missing-file and bad-host cases.

### Remote media — implemented, not yet verified live

`hermes-media://remote/` previously returned 501, so remote-gateway audio and
video had **no playback path at all** — an SSH or cloud profile could not play a
clip. Now Rust resolves the owning connection, mints a grant, forwards the
request including `Range`, and returns the bytes with `content-range` intact.
The credential never reaches the page.

Tested for the honest failure shape (unreachable gateway → 502, never a 200 with
empty bytes that a player would read as a zero-length clip) and for a missing
path → 400. **Not verified against a live remote gateway** — none exists in this
environment.

### Windows — written, not wired

`src-tauri/src/windows.rs` implements peer windows (`openWindow`) and
single-session windows (`openSessionWindow`), each with its own route baked into
its own bootstrap script so two windows never fight over one global route.
Session ids are refused if empty, over 200 chars, or containing separators or
control characters — a session id addresses a row, not a path.

**This does not compile into the app yet.** It needs `mod windows;`,
`bootstrap_script_for`, and dispatch arms. The file is untracked. As it stands
`canOpenNewWindow()` still returns false, so "New Window" and "open in another
window" remain absent from the UI.

## Verification totals

- Rust workspace: **21 contract + 46 shell tests**, 0 failed (`cargo test --workspace --offline`).
- Bridge: **8/8** (`node --test tests/ui/bridge-socket.test.mjs`).
- Renderer settings suite: **81 files / 625 tests**.
- Earlier scoped sweeps this session: 835 + 143 + 37 + 3,883.
- Native renders confirmed in the bundled fixture app: all seven destinations,
  Voice & audio, All settings.

## Later continuation

The window-wiring and several native gaps below were subsequently addressed.
See [native integration acceptance](NATIVE_INTEGRATION_PASS.md) for current
implementation and evidence. This report preserves the earlier checkpoint.

## What was still open at this checkpoint

Phases 6–11 are **not complete**. Open items, stated plainly:

1. **`windows.rs` unwired** — one commit's work, not a design question.
2. **16 bridge members still absent**: `applyConnectionConfig`,
   `saveConnectionConfig`, `oauthLogin/LogoutConnectionConfig`,
   `getGatewayFileStreamUrl`, `getPluginWsUrl`, `stageFileForAttach`,
   `capturePreview`, `onPreviewFileChanged`, `windowRelay`,
   `testConnectionConfig`, `onExternalOpenFailed`, `mcpOauth`,
   `openSessionWindow`, `openWindow`, `profile`. Down from 24, not zero.
3. **Voice against a real microphone and provider** — needs hardware. The
   Voice & audio page renders and the voice sockets fall through to
   `getGatewayWsUrl`, which exists; nothing has been exercised on a real mic.
4. **Connector OAuth end to end** — Telegram QR, MCP OAuth, plugin grants.
5. **Media capture**, browser/preview pop-out, vault, SSH/cloud families.
6. **Performance beyond cold start** — long transcripts, streaming events,
   repeated route switches, memory after reconnect cycles. Release profile
   never measured, so no claim is made that it is faster.
7. **Whole-tree renderer test run never completes** — killed twice past 20
   minutes. All renderer evidence is scoped sweeps.
8. **Plan checkboxes stale** — 50 unchecked in
   `2026-11-15-full-hermes-plan.md`; phases 1–5 are largely implemented but the
   document reads mostly unticked.

## One measurement worth recording

The first startup run reported **67 s**. That was not startup cost — it was the
boot-failure overlay waiting out its timeout because the fixture gateway was
down. With the gateway running, the same build measures ~6 s. Taken at face
value the first number would have become a fabricated performance claim in a
document, which is why the screen was checked rather than the number trusted.
