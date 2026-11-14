# Design critique — workspace shell (stage 7)

Scope: `docs/designs/WORKSPACE_SHELL.md` (design) and `docs/plans/WORKSPACE_SHELL_PLAN.md`
(plan), judged against the shipped channels in `src-tauri/src/{commands.rs,git_ops.rs,fs_ops.rs,task_ops.rs}`,
`src-tauri/src/bridge.js`, `ui/task.{html,css,js}`, and
`~/.hermes/skills/software-development/electron-to-rust-port/references/hermes-gateway-protocol.md`.

Verdict: the layout thesis is sound and the data-source table is a real improvement over prose, but
the contract is **not executable as written**. Two of its own acceptance criteria are arithmetically
impossible or unfalsifiable (B1, M9), the most prominent numbers on screen (changed-file counts) have
no defined scope (B2), and the state matrix omits the two states a real session spends most of its life
in: not-a-git-repo and gateway-unreachable (B3, M1).

Counts: **4 blockers, 9 major, 7 minor.**

---

## Blockers

### B1 [BLOCKER] §Responsiveness — the four-column budget cannot fit 1024 px, and the overflow is clipped rather than scrolled, so the acceptance test passes while the rail is cut off

Section: design §Responsiveness; plan Phase 0 items 4–5; plan Phase 5 item 33.

The grid is `264 | minmax(320,452) | minmax(420,1fr) | 320`. Minimum non-overflowing width:

- four columns, rail open: `264 + 320 + 420 + 320 = 1324`
- rail collapsed to 56: `264 + 320 + 420 + 56 = 1060`
- rail **and** chat collapsed (`<1180` sets chat to a fixed 360, which *raises* the chat floor from
  320 to 360 and therefore removes 40 px of slack at exactly the widths where slack is needed):
  `264 + 360 + 420 + 56 = 1100`

| window | left | chat | workspace | rail | required | result |
| --- | --- | --- | --- | --- | --- | --- |
| 1680 | 264 | 452 | 644 | 320 | 1680 | ok (reference) |
| 1456 | 264 | 452 | 420 | 320 | 1456 | ok — exact minimum for 4 columns |
| 1440 | 264 | 452 | 420 | 320 | 1456 | **16 px over** (collapse fires strictly `< 1440`) |
| 1324 | 264 | 320 | 420 | 320 | 1324 | ok — chat at its floor |
| 1180 | 264 | 440 | 420 | 56 | 1180 | ok |
| 1100 | 264 | 360 | 420 | 56 | 1100 | ok — exact minimum |
| **1024** | 264 | 360 | 420 | 56 | **1100** | **76 px over** (left rail only collapses below 1000) |
| 960 | 56 | 360 | 420 | 56 | 892 | ok |

So plan item 5 ("at 1024×700 the four columns are present, no overlap, no horizontal scrollbar") is
not achievable under the design's own breakpoints. Worse, `ui/task.css:22` sets
`html, body { overflow: hidden }`, and the plan adds "the whole page never scrolls" (item 3). Grid
overflow with a hidden root scroll container is **clipped, not scrollable** — the rightmost column (the
agent rail) simply disappears off-screen. That also defeats the guard the plan names:
`task:no-overlap` compares bounding rects, and clipped columns do not overlap each other. The test
asserts the one property that is not at risk and ignores the one that is.

Two secondary arithmetic defects in the same contract:
- Dead band `1440–1456`: the collapse threshold is below the width the un-collapsed grid needs, so the
  plan's own 1440×900 screenshot is taken inside the 16 px overflow band.
- The `<1180` rule is a monotonicity violation: narrower window ⇒ larger chat floor.

Recommendation: replace pixel breakpoints with an explicit width budget the CSS can honour —
`min_inner_size(1100, 600)` and drop 1024 from the matrix, or define a genuine two-column mode below
1100 (workspace + rail, chat as an overlay drawer) instead of pretending four columns survive. Then
make the test measure the failure, not its absence:
`doc.scrollWidth <= innerWidth` **and** for each of the four columns
`rect.right <= innerWidth + 0.5` **and** `rect.width >= declaredMin`.

### B2 [BLOCKER] §Data sources, "Changed files + counts" — the channel takes a `scope` argument the design never names, and every count on screen is derived from it

Section: design §Data sources rows "Changed files + counts", "Diff text"; plan items 13, 16, 19, 23.

`hermes:git:review:list` is `review_list(repo, scope, base_ref)` (`git_ops.rs:394`) with three
materially different behaviours, and the design names only the channel:

- **`uncommitted` (the default)** — `parse_status` + `git diff --cached` + `git diff`. A session that
  did the normal agent thing (implement → commit) has an empty list here. All of these then read zero:
  `N files changed` subtitle, `+A −D` file-change bar, `Diff (N)`, the file tab strip, the diff card.
  The window claims "no changes" about work that is committed and real.
- **`branch`** — diffs `defaultBranch...HEAD` and hardcodes `"status": "M"` for every row
  (`git_ops.rs:426`). The design's `M`/`A`/`D` tab badges are therefore wrong for every added or
  deleted file in branch scope — the exact badge vocabulary the reference calls out.
- **`lastTurn`** — requires a `base_ref`; the design never says where a session-start SHA comes from.

The design also annotates the row "(`+ git diff --numstat`)" as if the renderer could run git. It
cannot: there is no numstat channel (`repoStatus` and `review:list` are the only aggregation paths,
both already numstat-derived). The annotation describes an implementation detail of the channel while
omitting the one parameter that changes the answer.

Recommendation: specify scope **per surface**, in the table, with the base ref's provenance:
file-change bar and diff card = `lastTurn` with `base_ref = review:revParse(HEAD)` captured at
`session.resume` time (a new snapshot field or a stored-at-open value); tab strip badges derived from
`git status --name-status`, not from numstat (which is a line-count channel and cannot express A/D/R);
`Diff (N)` = length of that same list. Add "scope + base SHA" to the self-test detail string so the
number on screen is auditable.

### B3 [BLOCKER] §States — not-a-git-repo is missing, renders as "clean" rather than "unknown", `select a file` contradicts the tab source, and the plan regresses a state the current code already handles

Section: design §States, §Data sources "File content"; plan items 5, 16, 17, 20.

The state matrix has three states: no session / no changes / with changes. The interesting fourth is
missing. For `cwd = ~` or `/tmp`:

| region | actual result | what the user sees under the design |
| --- | --- | --- |
| `review:list` | `{files: [], base: null}` (`repo_root` → `None`) | identical to a clean tree → "no changes" (a lie, not an empty state) |
| `review:diff` | `""` | empty diff card, no reason |
| `repoStatus` | `null` | branch/repo empty in header and top bar |
| file tabs | empty | `Diff (0)` disabled, nothing to select |
| tests / plan | still work | fine |
| `readFileText` | **works** (any absolute path) | unreachable — there is no path to click |

That last row is the structural point: the file channel is cwd-independent, but the only navigator
the design gives the workspace is `git:review:list`. So a non-git cwd is not "unsupported", it is
"silently degraded to an empty middle column", and the design's own empty state ("workspace shows
*select a file*") is unfulfillable because the tab strip is derived from *changed* files — with zero
changed files there is no file to select. The same contradiction fires for the far more common case of
a clean tree inside a git repo.

This is a regression, not an omission: `ui/task.js:552-588` already handles it — it calls
`hermes:fs:gitRoot`, and when that fails it offers `git:scanRepos` results in a picker with the
comment "the user chooses, nothing is assumed", plus the explicit note `not a repository`. The redesign
drops that path and the plan has no item for it, so Phase 3 would ship strictly less honest than the
window it replaces.

Recommendation: add a fourth state to §States — **`cwd not a repository`** — with defined copy per
pane, keep the existing scanRepos picker (or move it to the workspace header), and let
`readDir`+`readFileText` serve the file tab strip when git is absent so the workspace is still a
file viewer. Add a self-test that asserts the *string*, not just emptiness: `task:repo-state` ∈
{`repo-clean`, `repo-dirty`, `not-a-repo`, `picker`}. Also state the rule for "no changes but a repo":
tabs empty ⇒ the workspace should show repo-level empty copy, not "select a file".

### B4 [BLOCKER] §Data sources "Diff text" + plan item 31 — unbounded payloads on a 4-second full-snapshot poll; the design has no cap, no virtualization, and no "too large" state

Section: design §States "Running", §Data sources "Diff text"; plan items 19, 31.

Three independent unbounded paths:

1. **Diff.** `review_diff` returns the entire `String` (`git_ops.rs:479`) — no size cap, no line cap —
   and plan item 19 renders every row as DOM with per-row backgrounds and gutter cells. One regenerated
   lockfile or vendored bundle = tens of thousands of rows; `package-lock.json` alone will lock the
   webview. Note the asymmetry: `read_file_text` caps at `TEXT_PREVIEW_MAX_BYTES = 512 KB` with a
   `truncated` flag (`fs_ops.rs:12`), and `review_list` caps at 200 files (`git_ops.rs:461`), but the
   *diff* path — the heaviest one — has neither.
2. **Transcript.** `task_ops::call` dials a fresh WebSocket per read and `session.resume` returns the
   whole snapshot including every message and every raw tool result (protocol doc: "the cheapest way
   to read a transcript"). Plan item 31 polls that every 4 s while running. Cost is O(transcript) per
   tick, per window, plus a ticket mint and handshake each time; a 10k-line session is re-serialised,
   re-parsed and (on message-count change) fully re-rendered, losing scroll position
   (`ui/task.js:697-701` calls `renderTranscript()` wholesale — full rebuild).
3. **Binary.** `git diff` on a binary emits one `Binary files … differ` line, which is fine — but
   numstat emits `-` for both counts, `parse_numstat_map` parses `-` to `None` → `(0,0)`, so a binary
   file that changed shows `+0 −0`. For an *untracked* binary, `count_lines` counts `0x0A` bytes in
   the file and reports them as added lines (`git_ops.rs:114-129`, used at 449) — a PNG shows
   "+1,842 lines". That is a fabricated number in a document whose premise is "no fabricated values".

Recommendation: define a payload contract. Diff: `max_bytes`/`max_lines` per file with an explicit
`truncated` marker and a "diff too large — open in editor" state (ml/cap in the 1–2 MB range, cap rows
around 2–5k, virtualise or hard-stop beyond). Transcript: poll on `revision` change, request/receive
incrementally if the gateway supports it, otherwise render only the last N messages with "show earlier"
and never rebuild the DOM when the message count is unchanged. Binary: use `--numstat -z` + the
`binary` flag, render `binary` instead of counts (never `+0 −0`, never a byte-derived line count), and
route untracked binaries to the file viewer's `binary` state rather than through `count_lines`.
Add `task:diff-cap` and `task:binary-diff` to the self-test list.

---

## Major

### M1 [MAJOR] §States — gateway unreachable/restarting has no state, and the poll swallows the error, so the shell shows stale truth indefinitely

Section: design §States, §Data sources (all rows); plan items 31–32.

Every pane's data comes from either `state.attach()` (REST/`hermes:api`) or a fresh WS dial
(`task_ops::call`). A gateway restart during a session produces: `hermes:api` → `Err` from `attach()`;
`session.resume` → `gateway ws: …`; `task_stats` → `{pid:null,cpu:null,rss:null}`. The current poll
swallows this by design — `catch { /* a transient gateway blip: the next tick retries */ }`
(`ui/task.js:705-707`) — so `running` stays true, elapsed keeps ticking, the status dot stays green,
and the transcript silently freezes. The design's only liveness concept is a status dot and an elapsed
timer, neither of which is fed by a health signal. Under the redesign this is worse, because five more
regions now claim to be live.

Recommendation: a single connection state machine (`attached | reconnecting | detached`) owned by the
window, fed by every channel's failure, surfaced as: status dot colour + `Reconnecting…` label + a
pane-level banner; poll failures must increment a counter and trip the state at N consecutive failures
instead of being discarded; `task_stats` null pid is a valid detached signal (it already is). Add
`task:connection-state` to the self-test and a manual case in Phase 5 (kill the gateway mid-turn).

### M2 [MAJOR] §Data sources "Tests" — the tests card is a regex over an arbitrary `tool` blob, and the promised per-file rows are not derivable in general

Section: design §Data sources "Tests"; plan item 20.

`session.resume`'s `tool` rows carry `tool_name` plus the raw result (protocol doc: "a viewer that
prints them looks like a JSON dump"). The design proposes to reconstruct `8 passed / 0 failed /
per-file rows with durations` from that text. Per-file rows and durations exist only for
`pytest -v`/`--durations` and `jest --verbose` or `--json`; plain `pytest -q` gives one summary line,
`npm test` gives whatever the script printed, and a `cargo test` run gives a different grammar again.
The plan says "parse pytest/jest summaries" and stops there — no corpus, no fixtures, no statement of
what happens when a transcript contains the *source* of a test file (a `readFileText` tool result with
the string "8 passed" in an assertion), or output from two runs (which run wins?).

Recommendation: demote the card honestly. Tier 1 (always truthful): last test-shaped command, its exit
status if the tool row carries one, and the raw tail, labelled with the tool call's timestamp.
Tier 2 (optional, clearly labelled "parsed"): the summary line only, with the matched line quoted
underneath so a wrong parse is visible. Drop "per-file rows with durations" from the design unless the
gateway exposes structured test results; if it does not, say so in §Non-goals rather than in a promise.
Add fixtures: three recorded transcript snippets (pytest -q, jest --verbose, cargo test) as test data.

### M3 [MAJOR] §Data sources "Tools enabled" — "used" is not "enabled", and `info.tools` is the authoritative source the design does not use

Section: design §Data sources "Tools enabled", §Elements "Capabilities (with enabled/disabled)" and
"Tools & Capabilities with enabled state"; plan items 7, 27.

`session.resume` → `info{tools, skills, profile_name}` (protocol doc line 87) is the enabled set. The
design instead derives the list from tool names appearing in the transcript, so:
a session that has used two tools reports **two** capabilities, and a tool that is enabled but unused is
reported as absent — i.e. "disabled". The left rail's "Capabilities (with enabled/disabled)" is
therefore inverted for every tool but the used ones, and a fresh session shows an empty capabilities
list, which reads as "no capabilities" rather than "not yet used". Two of the four regions that show
this data (left rail, agent rail) are specified to show *enabled* state while the only named source
can express *used*.

Recommendation: `info.tools` for the enabled list (with enabled/disabled toggles if the shell can
actually mutate them — it cannot today, so render them read-only and say so), `info.skills` for the
skills row, and a separate, differently-labelled "used in this session" count from the transcript.
Two datasets, two labels, no conflation.

### M4 [MAJOR] §Data sources "File tree" — `readDir` has no depth, no ignore rules and no cap; the rail's "Context (tree)" will be dominated by `.git` and `target`

Section: design §Data sources "File tree" ("`hermes:fs:readDir` on the repo, depth-capped");
plan item 28 ("Context: repo tree from `readDir`, depth 2, folders expandable").

`read_dir` is a single-level `std::fs::read_dir` that filters exactly one name (`.DS_Store`) and returns
`{entries:[{name,path,isDirectory}]}` (`fs_ops.rs:194-233`). It has no depth parameter, no ignore list,
and no result cap. Consequences:
- "depth-capped" is not a property of the channel; it is a call-count policy the renderer must invent,
  and the design does not state it (2 levels? 2 IPC calls? 1 + N?).
- At depth 2 on this very repo, the tree shows `.git/` (index, objects, refs, hooks…), `target/`
  (thousands of entries from a Rust build), `node_modules/`, and `.egm/`. The reference's "Context
  (tree)" is a source tree; this renders a build directory.
- Expansion is one IPC round-trip per folder, each returning a full listing with no cap — expanding
  `target/debug` is a several-thousand-entry payload into a 320 px rail.

Recommendation: define the tree policy in the design, not the implementation: roots from `git ls-files`
(already available through the git channel) with `.git`, `node_modules`, `target`, `dist`, `build`,
dot-directories excluded; fall back to `readDir` for non-repo cwds; hard caps (e.g. 200 entries/folder,
500 nodes total) with a "+N more" row; lazy expansion only. If the tree is meant to be the *session's*
context rather than the repo's files, say so and source it from the transcript's file reads — that
would actually match the label "Context".

### M5 [MAJOR] §Data sources "File content" — the named channel does not exist, the parenthetical is unenforceable, and the two states the channel returns are unhandled

Section: design §Data sources "File content"; plan item 17.

1. There is no `hermes:fs:readFileText`. The fs cluster is `hermes:fs:readDir|gitRoot|reveal|openDir|
   rename|writeText|…`; text reads are `hermes:readFileText` (`commands.rs:491`, `bridge.js:105`).
   A design that names the wrong channel is a design the implementer must silently correct — fine, but
   note the table is presented as the authoritative port contract and this is the only source named for
   the editor surface.
2. "(when the session cwd holds it)" describes a containment check that does not exist: `expand()` +
   `fs::read` on any absolute path (`fs_ops.rs:84-107`). If containment is a requirement, it is a new
   channel parameter, not a parenthetical.
3. The channel returns `{binary, byteSize, language, mimeType, path, text, truncated}` and
   `language`/`mimeType` derived from the extension only. Plan item 17 ("code view with right-aligned
   line numbers and minimal token colouring") has no state for `binary: true` (it would render
   `String::from_utf8_lossy` mojibake) or `truncated: true` (512 KB of a 4 MB file rendered as if
   complete, with line numbers that suggest completeness — again a "plausible but wrong" surface).
   `error_result(…)` returns `{error, code, path}` rather than throwing, so a permission failure and a
   successful read have the same promise shape.

Recommendation: name the real channel; add `binary` and `truncated` to §States explicitly ("binary file
— no preview", "showing first 512 KB of N MB"); use the returned `language` to drive the lexer instead
of re-deriving it; consider `hermes:fs:gitRoot` to resolve containment if the cwd rule matters.

### M6 [MAJOR] §Data sources header row — three of six fields are not in `info`, `branch` is a stale snapshot, and `elapsed` has no clock

Section: design §Data sources "Title, branch, repo, model, running, elapsed | `session.resume` → `info`".

Ground truth: `info` carries `cwd`, `branch`, `project`, `model`, `started_at`, `last_active_at`,
`tool_count`, `usage`. Therefore:
- `running` is top-level in the resume result, not in `info` (`ui/task.js:390` reads
  `snapshot.running`). Wrong pointer.
- `title` is not in `info`; it comes from the sidebar row (`ui/task.js:376`). With polls only
  refreshing `session.resume`, a renamed session keeps the old title.
- `repo` — `project` exists, `cwd` exists; the header's `repo · branch` contract is unstated (repo =
  project name? basename of cwd? git toplevel? They differ for worktrees, which this shell supports).
- `branch` is captured when the session attaches. If the user (or an agent tool) checks out another
  branch mid-session, the header and top bar keep the old branch while the diff card shows the new
  base — the two disagree and neither is marked stale. `repoStatus` returns a live branch; the design
  does not use it.
- `elapsed` has no field. Deriving it from `started_at` makes a resumed session display hours of
  "elapsed", which the reference clearly means as *execution* time (its card says "Executing…" next to
  a stopwatch). `last_active_at` on an idle session has the same problem in reverse.

Recommendation: split the row: `running`, `revision`, `todo_state`, `pending[]` from the snapshot root;
`cwd`, `model`, `project`, `usage` from `info`; `branch`/`ahead`/`behind` from `hermes:git:repoStatus`
on the same 4 s tick (it already returns all three plus `detached`); `elapsed` defined as
`now − turn_start` where turn start is the first message timestamp of the current `execution_generation`,
with a written rule for what happens after a resume. If the value cannot be derived, the design's own
standard says leave it out.

### M7 [MAJOR] §Data sources "Sessions, scheduled" — `recents_profile=default` is hardcoded while the rest of the window scopes by the session's own profile

Section: design §Data sources row 1; plan item 7.

The route's selector is `recents_profile` and the gateway rejects a value that does not resolve to the
ticket's home (protocol doc "Aggregate REST routes scope by a route-specific parameter"). The design
pins it to `default`. `session.resume` takes `profile` per session, and session rows carry their own
`profile` (`ui/task.js:799`). On an install whose active profile is not `default`, the rail lists
`default`'s sessions and the window resumes rows belonging to other profiles — or fails to resume them.
The design also flattens the response: the sidebar returns three groups with different semantics
(`recents.sessions`, `messaging.sessions`, `cron.sessions`; `ui/task.js:358-362`), and the design's
single row "Sessions, scheduled" hides which group feeds which rail section.

Recommendation: take the profile from the shell's active profile (`hermes profile list` /
`shell_info.hermesHome`), state that `recents_profile` ≠ `profile` for the other routes, and split the
row: Sessions ← `recents` + `messaging`; Scheduled + Active jobs ← `cron`. Note the `limit=40` in the
existing call and say whether the rail paginates.

### M8 [MAJOR] §Data sources "PR" — three distinct failures collapse into one boolean, and the top bar has no fallback vocabulary

Section: design §Data sources "PR", §Elements "GitHub ▾" / "GitHub pill"; plan item 6.

`ship_info` returns `{ghReady:false, pr:null}` for: `gh` not installed, `gh` installed but not
authenticated, and no repo/remote at all (`git_ops.rs:662-676` — `repo_root` → `null` gives the same
shape). `ghReady:true, pr:null` means "authenticated, no PR for this branch". Four states, one bit in
the data source, and the design's element list has one pill. Plan item 6 says "GitHub repo pill (`gh`
state)" — singular state, so the implementation will guess. `gh(&["auth","status"], …)` also runs a
subprocess per poll if it is wired into the 4 s tick, which it will be, because the pill shows PR state.

Recommendation: extend the shape (`{available, authenticated, hasRemote, pr}`), or at minimum have the
window derive three labels from two booleans and specify copy: "gh not installed", "sign in to gh",
"no pull request", `#123 · open`. Cache the auth probe (60 s), never per tick.

### M9 [MAJOR] Plan items 5/15/23/31/32 — several acceptance criteria are unverifiable as written; the self-test channel cannot fail a phase

Section: plan "Each phase is independently verifiable", items 5, 15, 23, 31, 32, 33.

- **The test result goes nowhere.** `record(...)` pushes into an array that `shell_diag` writes to
  `$TMPDIR/hermes-rust-diag-<window>.json` (`commands.rs:325-337`); the only documented consumer is a
  human running `cat` (README:47). No runner, no exit code, no expected-value list. A phase cannot be
  "verified" by a file nobody reads.
- **`task:no-overlap` cannot see the failure it is named for** (see B1): clipped columns do not overlap.
- **The inherited checks contradict the empty states the design promises.** `task:transcript` requires
  `messages.length > 0`; `task:outputs-tools` requires tool rows. The new design deliberately permits
  sessions with no messages/tools ("no session", "no test output"), so "plus the existing
  session/transcript/resources/PR checks" will report false for legitimate states, and `record(false)`
  is indistinguishable from broken.
- **Item 23** "counts equal `git diff --numstat`" is not checkable in-app (no numstat channel) and will
  *disagree* for binaries (`-` → 0), renames and paths with spaces (numstat quoting vs `parse_status`
  key), and >200 files (silent truncation, `git_ops.rs:461`).
- **Item 15** "send round-trips (verified in a throwaway session)" mutates the user's session store and
  depends on a live model; not a repeatable assertion. It is also the one path with a known trap
  (`prompt.submit` is attach-required and receipts are admissions, not turns — protocol doc lines
  94-103), so "send works" needs a stated observable: receipt `status`, then `pending[]`, then
  `messages[]`.
- **Item 31** "refresh on change of HEAD or file mtimes (cheap `git status` poll)" names no HEAD
  channel (`review:revParse` exists and is unmentioned), no mtime channel exists at all, and "cheap" is
  unquantified: `review:list` spawns `status` + 2 × `numstat` per call, and a 4 s cadence across a
  200-file tree plus a `readDir` tree walk is not obviously cheap.

Recommendation: give each phase 2–4 named assertions with expected values and a runnable harness
(`HERMES_SHELL_SELFTEST=1` → exit non-zero if any `ok !== true`, with a per-phase allowlist so
"legitimately empty" is expressible as `{expected:"empty"}`), replace item 23 with a fixture repo test
in `git_ops.rs` (the file already has `repo()` fixtures — reuse them for binary, rename, space-path and
>200-file cases), and rewrite item 31 as "poll `repoStatus` + `revParse HEAD` every 4 s while running;
refetch diff when the SHA changes; refetch tree on SHA change only".

---

## Minor

### m1 [MINOR] §Data sources — `review:list` truncates at 200 files silently while the header claims `N files changed`
`git_ops.rs:461`. A dirty monorepo (or a session that generated files outside `.gitignore`) shows
"200 files changed" as if that were the count. Either return `{files, total, truncated}` or say in the
design that 200 is the contract and render "+N more".

### m2 [MINOR] §Data sources — renamed files and paths containing spaces report `+0 −0`
`parse_numstat_map` keys on the raw third field; git quotes and brace-compresses those paths, so the
lookup by `entry.path` misses and both counts fall back to 0. `git status -z` (already used) does not
have this problem — use the same `-z` protocol for numstat.

### m3 [MINOR] §Elements "file tabs (`M` / `A` badges)" — the channel's full vocabulary is M/A/D/R/U
`status_letter` returns `R` for renames; conflicts (`UU/AA/DD`) fall through to `"M"`. The plan only
specifies M/A/D, so a rename renders as M and a conflict renders as a plain modification. Decide the
badge set and the conflict treatment (conflicts deserve their own colour, not a silent M).

### m4 [MINOR] §Responsiveness — the top bar has no overflow rule at 960 px
Six nav pills + GitHub pill + branch + dot + monitor + gear + share + ellipsis + avatar inside 50 px at
960 wide, with no truncation, collapse or priority order. The "long strings truncate, never widen a
column" rule is written for grid columns and does not apply to a flex top bar. Specify: which items drop
first (a `…` menu), and whether the branch label truncates at N chars.

### m5 [MINOR] §States — the workspace's default tab is undefined for the empty case
Plan item 18: "Diff default when changes exist." When there are no changes and no tests, all four tabs
are empty and no default is stated. Define it (Logs, since any session with a tool call has one) and say
what an empty Diff looks like when `review:diff` returns `""` on failure rather than on emptiness —
today those are the same string.

### m6 [MINOR] §Data sources vs §Elements — the table covers 13 regions for ~40 enumerated elements
No named source for: avatar/initials (plan says profile footer — from what field?), Inbox badge (design
shows "Inbox 3"; plan item 7 says the row is *disabled* — the two documents disagree, and the badge is
exactly the "fabricated count" the plan forbids), model selector (plan says static — then it is a
control that lies), ahead/behind, GitHub pill state, "Pinned" (plan: static from config — which config
key?), participant/timestamps. The design's claim "(all live; no fabricated values)" is only true for the
rows that exist.

### m7 [MINOR] Plan items 1–2 / design §Responsiveness — `position: absolute` is banned by name but the composer exception is unbounded
"No element may use `position: absolute` except the composer inside the chat column and the traffic-light
spacer" — the composer is the element most likely to need `position: sticky` + a scroll container, and
the exception re-creates the failure mode the design just removed (an overlay whose height is not part of
the column's layout). Prefer a grid row for the composer and drop the exception; keep only the traffic
light spacer, which is inside the 50 px top bar and could equally be a padding block.

---

## Answers to the five questions

**(a) Can the named APIs deliver each region?** Six of thirteen rows are sound
(sessions/scheduled via `hermes:api` + `recents_profile` — subject to M7; messages; plan — `todo_state`
with the todo-tool fallback matches the protocol doc; active jobs from `cron.sessions`; resources from
`task_stats` + `usage`; PR from `shipInfo` — subject to M8). Seven are not deliverable as written:
changed-file counts (B2: scope undefined, `status` hardcoded in branch scope), diff text (B4: unbounded),
file content (M5: channel name wrong, no containment, `binary`/`truncated` unhandled), file tree (M4:
`readDir` has no depth/ignore/cap), tests (M2: scraping, per-file rows undeliverable in general),
tools-enabled (M3: transcript ≠ enabled, `info.tools` is authoritative), header row (M6: `running`,
`title`, `elapsed` are not in `info`; `branch` is stale).

**(b) Which regions render empty/wrong for non-git cwd, no todos, no tests?**
- *Not a git repo*: file tabs, diff card, `Diff (N)`, `N files changed`, `+A −D` all read as "clean"
  rather than "unknown" (B3) — wrong, not empty; header repo/branch blank; `readFileText` works but is
  unreachable. Severity: wrong data, and a regression against `ui/task.js:552-588`.
- *No todos*: chat plan card is specified hidden (plan item 11) ✓; the agent rail's plan mirror has no
  hidden rule (item 26) → an empty "Plan" block in the rail while the chat column correctly hides it —
  the same data rendered two ways.
- *No tests*: tests card has an explicit empty state ✓ ("no test output in this session"); the tab
  itself stays present and enabled, which is fine but needs the default-tab rule (m5).
- *Clean tree in a repo*: the design's "select a file" is unfulfillable because tabs come from changed
  files (B3). The plan's empty states are therefore **not sufficient**: they cover absence of *data*
  but not absence of *capability*, and they have no vocabulary for "we cannot know".

**(c) Does the responsiveness contract survive 1024 px with four columns?** No. Arithmetic in B1:
the four-column minimum is 1324 px open-rail / 1100 px with the chat floor at 360, and 1024 is 76 px
short; `overflow:hidden` clips rather than scrolls, so the agent rail is cut off while "no horizontal
scrollbar" and `task:no-overlap` both pass. Additional defects: 16 px overflow at exactly 1440 (the
plan's own screenshot width), and the `<1180` rule raising the chat floor from 320 to 360 as the window
shrinks.

**(d) Missing failure modes.** Gateway down/restart (M1 — no state, errors swallowed, stale UI);
`gh` missing/unauthenticated/no-remote collapsed to one boolean (M8); huge diffs (B4 — no cap on
`review_diff`, every row rendered); binary files (B4 — `+0 −0` from `-` numstat, and fabricated line
counts for untracked binaries via `count_lines`); 10k-line transcripts (B4 — full snapshot over a fresh
WS every 4 s, full DOM rebuild); plus, unaddressed: file deleted between list and diff click
(`review_diff` → `""`, indistinguishable from an empty diff), 200-file truncation presented as a total
(m1), `readFileText` error shape vs success shape (M5). The plan addresses exactly one of these — the
empty-state risk in §Risks — and does so only for the tests panel.

**(e) Unverifiable as written.** Plan items 5 (`no-overlap` cannot detect clipping), 15 ("throwaway
session", non-deterministic, mutates the user's store), 23 (no numstat channel; will disagree for
binaries/renames/>200 files), 31 (no HEAD or mtime channel named; "cheap" unquantified), 32 (results land
in a temp JSON no runner reads; inherited assertions assert `>0` and therefore fail on the empty states
the design requires), and the "each phase is independently verifiable" claim — verification is
concentrated in Phase 5, so Phases 1–4 are accepted by prose screenshot only. See M9.

---

## What to change before implementation (ordered)

1. Rewrite §Responsiveness as a width budget with a stated minimum viable width, and make the test
   assert no clipping plus a minimum column width (B1).
2. Add a `scope` + `base_ref` column to every changed-file row in §Data sources; capture the base SHA at
   resume (B2).
3. Add the two missing states — `not a repository` and `connection lost` — to §States, with copy per
   pane, and restore the existing repo picker (B3, M1).
4. Write a payload contract for diff/transcript/binary with caps and an explicit "too large" state (B4).
5. Re-source the four rows that point at the wrong thing: `info.tools` (M3), live `repoStatus` (M6),
   active profile for `recents_profile` (M7), `readDir` → a policy or `git ls-files` (M4).
6. Replace prose acceptance with named assertions + a runner that exits non-zero (M9).
