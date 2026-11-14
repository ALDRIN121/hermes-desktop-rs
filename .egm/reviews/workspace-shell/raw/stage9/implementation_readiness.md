# Implementation readiness — `docs/plans/WORKSPACE_SHELL_PLAN.md`

Reviewed against the code the plan must change, not against the plan's own claims.
Every finding below cites the file and line that decides it. Read-only review.

**Verdict: not implementation-ready.** Three findings block Phase 0/3 outright
(F1, F2, F3), five more make a named phase acceptance criterion unsatisfiable on
this machine (F5, F6, F9, F10, F15).

---

## 1. Channels, argument order, JSON shapes — what the plan assumes vs. what exists

| Plan says | Reality | Verdict |
| --- | --- | --- |
| `git:review:list` (Phase 3.16) | `hermes:git:review:list`, `commands.rs:699-712`, args `[repo, scope, base]` | name needs the `hermes:` prefix |
| `readDir` (Phase 4.28) | `hermes:fs:readDir`, `commands.rs:514`, one arg, `{entries:[{name,path,isDirectory}]}` | exists; shape differs from plan's "tree" |
| `review:list` `added`/`removed` | **exist** — `git_ops.rs:451-457` returns `{path, added, removed, status, staged}` inside `{files, base}` | correct |
| `git:review:diff` | `commands.rs:714-731` — five **positional** args `[repo, file, scope, base, staged]`, returns a bare JSON **string** | not stated in the plan; wrong-order/omitted-arg calls fail silently |
| `session.resume` → `todo_state`, `info` | exists (`ui/task.js:381-390`); `info` carries `"tools": {}, "skills": {}` | plan's "Tools & Capabilities with enabled state" has no source |
| cron sessions with `schedule` | **no such field** — sidebar rows are plain session rows with `min_message_count=1` | Phase 1.7 and 4.29 unbuildable as written |
| `session.create` | real RPC, but **zero callers in this repo**; returns `stored_session_id` | untested path, and the new session is invisible in the rail |
| `hermes:window-control` | only `minimize` / `toggle-maximize` / `close` (`commands.rs:462-476`) | no way to set a window size |

### F1 — Blocker: Phase 0's grid minimums contradict its own acceptance test

Phase 0.1 fixes the grid at `264px | minmax(320px,452px) | minmax(420px,1fr) | 320px`.
Minimum sum = **264 + 320 + 420 + 320 = 1324 px**. Phase 0.5 then requires
"at 1680×946, 1280×800 and 1024×700 the four columns are present, no overlap, no
horizontal scrollbar". At 1280 and 1024 that is arithmetically impossible, and
Phase 0.4's `minWidth 960` (`main.rs:171` currently `min_inner_size(900.0, 560.0)`)
guarantees a horizontal scrollbar at minimum size.

The design doc is self-consistent (collapsed rails: 56 + 360 + 420 + 56 = 892 at
1024); the **plan** drops the collapse rules and states the opposite acceptance.

Recommendation — in `ui/task.css`, replace the single `#app` grid with:

```css
#app { display: grid; grid-template-columns: var(--rail-w) minmax(320px,452px) minmax(420px,1fr) var(--agent-w); grid-template-rows: 50px 1fr; }
#app { --rail-w: 264px; --agent-w: 320px; }
@media (max-width: 1440px) { #app { --agent-w: 56px; } }
@media (max-width: 1180px) { #app { --rail-w: 216px; grid-template-columns: var(--rail-w) 360px minmax(360px,1fr) var(--agent-w); } }
@media (max-width: 1000px) { #app { --rail-w: 56px; } }
```

and restate Phase 0.5 as "four regions present (two of them collapsed) with
`scrollWidth === clientWidth` on `#app`". Cite the collapsed widths, not "four columns".

### F2 — Blocker: the plan never resolves a repository, and no session on this machine has one

Phase 3 builds file tabs, diff, and the `N files changed` bar from
`git:review:list`, and Phase 4.28 builds the tree from `readDir` — both need an
absolute **repo** path. The plan never says where it comes from.

The data says it is not free:

- `sessions.git_repo_root` is NULL for **all 173** rows in `~/.hermes/state.db`.
- Sampled `sessions.cwd` over the 60 most recent sessions: 55 × `/Users/aldrinjoseph`,
  2 × `/Users/aldrinjoseph/Developer`, 2 × `.../Resume-agent`, 1 × `~/.local/bin` —
  **none is inside a git work tree**. `hermes:fs:gitRoot` (`fs_ops.rs:340`) returns
  `Value::Null` for all of them.
- `review_list` fails soft: `git_ops.rs:395-397` returns `{"files": [], "base": null}`
  for a non-repo path — no error, so the UI renders an empty workspace and looks broken
  rather than reporting a cause.

The current window already solves this and the plan drops it: `loadChanges()`
(`ui/task.js:552-623`) does `fs:gitRoot(info.cwd)` → `git:scanRepos([~/Developer,~/Projects,~/code], {maxDepth:2})`
→ a `#repo-picker` `<select>`; `ui/codex.js:29-45` does the same.

Recommendation:
1. Keep the resolution step and the picker in `ui/task.js`; expose `repoRoot` in `state`
   and pass it to every git call.
2. Add a determinism hook in `main.rs:157-175` (`open_task_window`): read
   `HERMES_TASK_REPO` / `HERMES_TASK_SESSION` and append `repo=…&session=…`, the way
   `open_workspace_window` already does for `HERMES_WORKSPACE_REPO` (`main.rs:66-70`).
3. State the empty-state text per case ("session cwd is not a repository — pick one").

### F3 — Blocker: Phase 0 deletes the DOM ids that `task.js` dereferences unconditionally

Phase 0.2 deletes `#outputs`. `renderOutputs()` (`ui/task.js:223-346`) writes into
`#stat-messages`, `#stat-tools`, `#stat-elapsed`, `#context-fill`, `#context-text`,
`#cache-text`, `#subagents`, `#subagent-list`, `#tools`, `#sources`, `#outputs-empty`
(`ui/task.html:67-119`). `el(...).textContent = …` on a missing node throws, and
`openSession()` calls `renderOutputs()` inside its `try` (`ui/task.js:395`), so the
resume path reports failure and the transcript never paints.

Recommendation: Phase 0 must either move those ids into the new agent rail in the
same commit, or open `renderOutputs()` with `if (!el('stat-messages')) return`.
Do not ship a layout-only Phase 0.

### F4 — High: `review:list` status badges are wrong in branch scope, and the file list is capped

- `git_ops.rs:426` hardcodes `"status": "M"` for every file when `scope == "branch" | "lastTurn"`.
  Phase 3.16's `M`/`A`/`D` badges and Phase 3.23's "counts equal numstat" break there.
  A/D are only correct in the default `uncommitted` path (`git_ops.rs:455`, `status_letter`).
- `git_ops.rs:461` truncates to 200 rows; `repo_status` (`git_ops.rs:210-223`) also
  reports `changed` after `truncate(200)`. So "N files changed" saturates at 200.
- Untracked directories arrive as a single row (`--untracked-files=normal`, `git_ops.rs:75`)
  and `review_diff` then runs `git diff --no-index -- /dev/null <dir>` (`git_ops.rs:488`),
  which produces no output — a clickable tab that always renders empty.

Recommendation: in `git_ops.rs:394`, derive `status` from `parse_status` for **all**
scopes (map `A`/`D`/`R` there), return `isDir` for untracked directory rows, and drop
the 200 cap or return `{ files, total }` so the header can print the true count.

### F5 — High: `review:diff` argument order and return type are unstated and fail soft

`commands.rs:714-731` reads `args[0]=repo, args[1]=file, args[2]=scope, args[3]=base, args[4]=staged`
(the same order as the Electron preload, `ui/electron-preload.js:511`) and returns
`Value::String(...)`. Two silent failures follow from a naive port:

- Omitting `scope`/`base` falls through to `git diff HEAD -- file` (`git_ops.rs:498`),
  so a branch-scope UI silently shows uncommitted changes only.
- Treating the result as an object (`diff.lines`, `Object.keys(diff)`) breaks; it is a
  plain string.

Recommendation: one helper in `ui/task.js`:

```js
const diffFor = (repo, file, scope, base) =>
  call('hermes:git:review:diff', [repo, file, scope || 'uncommitted', base || '', false])
    .then((d) => (typeof d === 'string' ? d : ''));
```

and port `renderDiff()` from `ui/codex.js:124-160` rather than writing a new one; it
already handles `@@`, `+++`, `+`, `-` line classes.

### F6 — High: scheduled jobs have no `schedule` and no next run anywhere in the sidebar payload

`ui/task.js:144` reads `job.schedule || relative(job.last_active)`. The sidebar route
(`hermes_cli/web_routers/profiles.py:523`) builds `cron.sessions` via
`list_sessions_rich(source="cron", min_message_count=1, compact_rows=True)`, which is a
projection of the `sessions` table (`hermes_state_sessions.py:1084`,
`_SESSION_COMPACT_EXCLUDED`). That table has no `schedule`, `next_run`, or `cron_job`
column — verified in `~/.hermes/state.db` and by grepping the router. The fallback is
therefore *always* taken; nothing in the UI can tell you it happened.

Also: this profile has exactly **one** cron session, and zero pinned sessions.

Recommendation: add a `hermes:api` GET on `/api/cron/jobs`
(`hermes_cli/web_routers/cron.py`, route list confirmed) and render jobs, not run
sessions. Keep `cron.sessions` only for "last run". Phase 1.7's "Pinned (static labels
from config)" should be deleted — with 0 pinned rows it is invented data, which the
design forbids.

### F7 — High: "Tools & Capabilities with enabled state" has no data source

`session.resume`'s `info` is built as
`{**_lazy_info_route(...), "tools": {}, "skills": {}, cwd, branch, project, lazy: True}`
(`tui_gateway/methods_session.py:365`). `info.tools` is an empty object; there is no
shell channel that returns tool enablement, and `hermes:feature-flags`
(`commands.rs:369`) returns only `localModels`/`assumeFreshInstall`.

Recommendation: rename the region to **"Tools used"** and derive it from transcript
tool names (already implemented, `ui/task.js:223-324`), or add a REST route first. Phase
1.7 and 4.27 must not promise an enabled/disabled affordance.

### F8 — Medium: `readDir` is one level, unsorted for the plan's purpose, and returns `.git`/`target`

`fs_ops.rs:194-233`: `{entries: [{name, path, isDirectory}]}`, directories first, only
`.DS_Store` filtered. No depth parameter, no `mtime`, no `truncated`. Phase 4.28's
"depth 2, folders expandable" is 1 + N calls; on a Rust repo the first level includes
`.git`, `target`, and `node_modules`, which will hang the pane if expanded eagerly.

Recommendation: lazy-expand only (call on click), client-side denylist
`['.git','node_modules','target','dist','.venv']`, and cache `Map<path, entries>`.
If "refresh on file mtimes" (Phase 5.31) is kept, `fs_ops::read_dir` must gain
`"mtimeMs": metadata.modified()` — no channel exposes mtimes today.

### F9 — Medium: the four-width screenshot matrix is not reachable on this display, and there is no resize channel

- Logical desktop here is **1440×900** (`osascript … bounds of window of desktop` →
  `0, 0, 1440, 900`). A 1680×946 window cannot be shown, let alone screenshotted, and
  900 px of height does not fit under the menu bar.
- `open_task_window` builds 1240×820 (`main.rs:170`) and Phase 0.4 only changes the
  *minimum*; nothing in the plan raises the default.
- `hermes:window-control` cannot set a size (`commands.rs:462-476`), so an in-app
  self-test cannot drive breakpoints either.

Recommendation: change the matrix to **1440×900, 1280×800, 1024×700, 960×600**; add a
`"set-size"` arm to `hermes:window-control` using
`window.set_size(tauri::LogicalSize::new(w, h))`, gated behind the selftest flag, and
have `task:layout-grid` iterate the widths itself.

### F10 — Medium: which of Phase 5's self-tests are actually assertable

| Self-test | Assertable? | Fixture |
| --- | --- | --- |
| `task:layout-grid` | yes — `getComputedStyle(el('app')).gridTemplateColumns` splits into N tracks | DOM ids must exist first |
| `task:no-overlap` | yes — `getBoundingClientRect()` pairwise intersection of `#chat`, `#workspace`, `#agent-rail` | needs F9's resize to test more than one width |
| `task:file-tabs` | only with a repo | **no fixture exists** — no session cwd is a repo (F2) |
| `task:diff-lines` | only with a dirty repo | **no fixture exists** — needs a temp repo |
| `task:plan-mirror` | yes | fixture **exists but must be pinned by id**: 8 sessions carry paired `todo` tool results (`20260902_200912_27e61f` first), and `session.resume` derives `todo_state` from them (`tui_gateway/tool_progress.py::_todo_state_from_history`). `PORT_REPORT.md:196`'s "no session carries `todo_state`" is stale. |
| `task:tests-panel` | yes | fixture **exists**: 97 sessions contain pytest-shaped tool output (`… N passed in 0.01s`) |
| existing `task:pr` | yes | needs `gh` authed (it is: `ALDRIN121`) + a repo from `scanRepos` |

The blocker for the two "exists" rows is selection: `boot()` reads `?session=`
(`ui/task.js:798`) but `open_task_window` only ever appends `selftest=1` (`main.rs:163-167`),
and the default is `sessions[0]` — today a subagent session with no plan and no tests.
Without F2's `session=`/`repo=` plumbing, `task:plan-mirror` and `task:tests-panel` are
guaranteed red on a clean checkout.

### F11 — Medium: the traffic-light spacer needs a window-style change the plan omits

Phase 0.2 blesses a "traffic-light spacer" inside the webview, but
`open_task_window` (`main.rs:168-172`) uses default decorations, so the macOS buttons
live in the OS title bar above the page. The 50 px top bar will sit *below* a title bar
and the spacer will be dead space.

Recommendation: on `open_task_window`, macOS only:
`.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true)`, plus ~78 px of left
padding in the top bar. Otherwise delete the spacer from the plan.

Minor, same item: Phase 0.2's rule ("no `position: absolute` except composer and spacer")
also outlaws `#status` (`ui/task.css:265`, `position:absolute`) and `#composer-wrap` is
*not* absolute today — restate the rule as "no absolutely positioned card over another pane".

---

## 2. Capability and window-registration work

### F12 — Medium: none is strictly needed — but the plan must say so, and one constraint is load-bearing

- Label stays `task` (design §Target), already in `src-tauri/capabilities/default.json:5`
  (`"windows": ["main", "workspace", "task"]`), so **no capability change**.
- `src-tauri/tauri.conf.json` has `"app": { "windows": [] }` — every window is built in
  `main.rs`; there is no config entry to add.
- `tauri::generate_handler!` (`main.rs:197-209`) needs no change **unless** a new
  `#[tauri::command]` is introduced (e.g. a resize or cron-jobs helper). If one is,
  it must be added there *and* nothing else — app commands are not ACL-gated in v2.
- If the plan ever renames the label (e.g. `workspace-shell`), the window silently loses
  `core:default` and every `invoke` fails. Add the label to
  `capabilities/default.json` `windows` in the same commit.
- Constraint the plan should record: the task window is built **without**
  `initialization_script` (`main.rs:168` vs. the main window's `main.rs:219`), so
  `window.hermesDesktop` (`src-tauri/src/bridge.js`) is **undefined** there. The rebuilt
  UI must use `window.__TAURI__.core.invoke` only — `withGlobalTauri: true` is what makes
  the current window work.

---

## 3. What is missing for each phase to be independently shippable

| Phase | Ships alone? | Missing |
| --- | --- | --- |
| 0 — grid | **No** | F3 (deleting `#outputs` breaks `renderOutputs`), F1 (acceptance math), F11 (title bar), and no `#workspace`/`#agent-rail` DOM exists yet, so `task:no-overlap` has nothing to measure |
| 1 — top bar/rails | **No** | F6 (`schedule`), F7 (tool enabled state), F17 (New chat), and "Pinned from config" is fabricated data |
| 2 — chat column | **No** | the completion bar needs `review:list` → F2; "send round-trips in a throwaway session" needs a session-creation path that has never been exercised (F17) |
| 3 — workspace | **No** | F2 is a hard blocker; F4/F5/F15 change what "correct" means; `#repo-picker` must be carried forward |
| 4 — agent rail | **No** | F2 (tree root), F6 (jobs), F8 (readDir cost) |
| 5 — live behaviour | **No** | F9 (no resize, no 1680 display), F10 (`session=`/`repo=` plumbing), no mtime source for the refresh trigger |

### F13 — Medium: phase independence is not achievable in the stated order, and one reordering fixes most of it

Phases 0–2 depend on data Phase 3 supplies (repo root), and Phase 3 depends on F2 which no
phase owns. Recommendation — insert a **Phase 0.5 "data plumbing"** that is small and
independently verifiable:

1. `open_task_window` accepts `repo`/`session` (env + query), `main.rs:157`.
2. `ui/task.js` gains `state.repoRoot` with `fs:gitRoot → scanRepos` resolution and the
   picker, ported from `ui/task.js:552-623`.
3. `ui/task.js` gains `diffFor()` (F5) and `listFor()` wrappers.
4. `shell_diag` payload gains `repoRoot`/`sessionId` so failures are diagnosable.

After that, Phases 0, 1, 2, 4 each have the data they need.

### F14 — Low: channel-name drift will produce `unimplemented:` errors

The plan writes `git:review:list` (3.16) and `readDir` (4.28); the dispatcher matches on
the full string and falls through to `Err(format!("unimplemented:{other}"))`
(`commands.rs:886`). Copying the plan literally yields a channel that does not exist.
Recommendation: normalise every channel name in the plan to the `hermes:`-prefixed form
used in `commands.rs`, as the design doc already does.

### F15 — Low: Phase 3.23's acceptance criterion is unsatisfiable as written

"counts equal `git diff --numstat`" cannot hold: `review_list` sums **staged + unstaged**
per path (`git_ops.rs:436-446`) and counts untracked files by reading them
(`git_ops.rs:448-450`). `git diff --numstat` excludes both staged content and untracked
files, and for a partially staged file the sum genuinely differs from `git diff HEAD`.
Replace with: "counts equal `git diff HEAD --numstat` plus the line count of each
untracked file", or assert against `review:list` itself.

### F16 — Low: the plan leaves a second git-review window in place

`ui/codex.html|js` is the `workspace` window (`main.rs:55-83`), opened from the tray
(`main.rs:104`) and from `--workspace`. The plan rewrites `task.*` into a superset and
never mentions it, leaving two diff surfaces and two tray entries. Decide in Phase 3:
retire `codex.html` (and the tray item), or scope it to "PR review only". Whichever,
say it in the plan so the next reader does not build the diff view twice.

### F17 — Low: "New chat via `session.create`" is an untested path with two traps

`session.create` exists as a gateway RPC (used by the shipped renderer:
`ui/assets/main-Dix6zEDd.js`, returns `stored_session_id`, not `session_id`), but there
are **no callers** in `src-tauri/src/task_ops.rs` or `ui/task.js` — this would be its
first use in the shell. Two traps:

1. The rail is built from a sidebar that filters `min_message_count=1`
   (`hermes_cli/web_routers/profiles.py:563`), so a freshly created empty session **never
   appears** in the Sessions list. "New chat" would look like a no-op.
2. `request_id` is a client-side envelope field that the renderer strips before sending
   (`ui/assets/actions-D6n3bbKu.js`, `function Oa`), so it must not be passed as an RPC
   param.

Recommendation: optimistic insert of the created row into `state.sessions` + immediate
`openSession(stored_session_id)`, and a `task:new-chat` self-test that asserts the row
exists and is active.

---

## 4. Facts verified on this machine (for the next reviewer)

- Gateway alive: `~/.hermes/gateway.pid` → pid 90832, up 1 d 23 h. `task_stats` will return real numbers.
- `gh` installed at `/opt/homebrew/bin/gh`, authenticated as `ALDRIN121` → `review:shipInfo` reaches `ghReady: true`.
- 173 sessions, 22 947 messages; 1 cron session; 0 pinned; 0 with `git_repo_root`.
- 8 sessions have paired `todo` tool results (plan fixture); 97 sessions have pytest-shaped output (tests fixture).
- Display 1440×900 logical; 1680-wide acceptance is physically unreachable.
- `git_ops` already has the fixture pattern to copy for F10:
  `git_ops::tests::repo(name)` (`git_ops.rs:715-724`) creates a temp repo with one commit.
