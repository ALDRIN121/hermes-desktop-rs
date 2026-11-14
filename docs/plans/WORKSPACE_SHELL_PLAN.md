# Workspace shell — implementation plan

Companion to `docs/designs/WORKSPACE_SHELL.md`. Each phase is independently
verifiable and ends with a screenshot plus self-test assertions.

## Phase 0 — Layout skeleton and responsiveness (blocking)

1. Replace the current flex/absolute layout in `ui/task.html|css` with one CSS
   grid: top bar 50 px; columns `264px | minmax(320px,452px) | minmax(420px,1fr) | 320px`.
2. Delete the absolutely positioned `#outputs` card; the agent rail becomes its
   own grid column. No element may use `position: absolute` except the composer
   inside the chat column and the traffic-light spacer.
3. Independent scroll regions with `min-height: 0`; the whole page never scrolls.
4. Breakpoints per design §Responsiveness; window `minWidth` 960, `minHeight` 600
   in `main.rs`.
5. Accept: at 1680×946, 1280×800 and 1024×700 the four columns are present, no
   overlap, no horizontal scrollbar; `task:no-overlap` self-test compares
   bounding rects of chat/workspace/rail.

## Phase 1 — Top bar and rails

6. Top bar: Hermes mark + wordmark, nav pills (Chat active; Code/Sessions/Tasks/
   Agents/Capabilities inert with `title="not ported"`), right cluster with
   GitHub repo pill (`gh` state), branch + ahead/behind, running dot, avatar.
7. Left rail: New chat (opens a session via `session.create`), search filter,
   Explore/Inbox rows disabled, Pinned (static labels from config, empty state
   otherwise), Sessions with selection + green running dot, Scheduled jobs from
   cron sessions with `schedule`, Capabilities list from tool names, profile
   footer (`~/.hermes` user, initials).
8. Accept: rail scrolls independently; clicking a session loads it; scheduled
   rows show real schedules; no fabricated badge counts.

## Phase 2 — Chat column

9. Header: title, `repo · branch · N files changed`, share/ellipsis inert.
10. Messages: avatar, name, relative timestamp, prose; tool activity collapsed
    into `used N tools` rows (existing behaviour, restyled).
11. Plan card: `Plan · N steps`, ✓/▸/○, active row tinted; hidden when no plan.
12. Execution card: shown while `running` — `Executing…`, current activity,
    elapsed; tool row with monospace command from the latest terminal call.
13. Completion summary: last assistant message; file-change bar `N files changed
    +A −D` + **View diff** switching the workspace to Diff.
14. Composer: textarea + agent selector (static), send button; Enter sends via
    `task_submit`, Shift+Enter newline, disabled while empty.
15. Accept: plan completed count matches `todo_state`; summary shows only real
    numbers; send round-trips (verified in a throwaway session).

## Phase 3 — Workspace

16. File tab strip from `git:review:list`, badge `M`/`A`/`D`, active tab; plus
    button inert.
17. Breadcrumb `dir › file`; code view with right-aligned line numbers and
    minimal token colouring (strings, comments, keywords) via a small lexer.
18. Bottom tabs: **Diff (N)**, Tests, Terminal, Logs. Diff default when changes
    exist.
19. Diff card: per-file header, `+A −D`, hunk headers, green/red row backgrounds,
    gutter numbers; horizontal scroll for long lines (no wrapping).
20. Tests card: parse pytest/jest summaries from tool output; per-file rows with
    counts and durations; empty state otherwise.
21. Terminal tab: last terminal command + its output tail from the transcript
    (read-only, labelled as a log).
22. Logs tab: chronological tool calls with timestamps.
23. Accept: selecting a file shows its diff; counts equal `git diff --numstat`;
    tests card shows numbers only when the transcript contains them.

## Phase 4 — Agent rail

24. Header `Hermes Agent`, running pill, pause/close inert.
25. Goal: first user message of the session, truncated.
26. Plan mirror: same items as the chat plan card, active step highlighted.
27. Tools & Capabilities: tool names with enabled state from transcript.
28. Context: repo tree from `readDir`, depth 2, folders expandable.
29. Active jobs: cron sessions, `Next:` from schedule.
30. Accept: rail mirrors chat plan exactly; tree shows real files; jobs match the
    scheduled list.

## Phase 5 — Live behaviour and verification

31. Poll `session.resume` every 4 s while running, 15 s idle; diff/tests/tree
    refresh on change of HEAD or file mtimes (cheap `git status` poll).
32. Self-tests: `task:layout-grid`, `task:no-overlap`, `task:file-tabs`,
    `task:diff-lines`, `task:plan-mirror`, `task:tests-panel`, plus the existing
    session/transcript/resources/PR checks.
33. Screenshots at 1680×946, 1440×900, 1280×800, 1024×700; each quoted in the
    report with what the labels say.

## Out of scope

Voice input, model switching persistence, multi-repo workspaces, inline editing
of files from the code view.

## Risks

- **Reference is a mockup**: the tests panel and `+186 −12` numbers exist only
  when a real session has that output. Risk: a session with no test run looks
  empty. Mitigation: explicit empty states, never invented counts.
- **Editor fidelity**: a full syntax highlighter is out of scope; a 40-line
  lexer covers strings/comments/keywords and is labelled minimal.
- **Width thrash**: four columns at 1024 px leaves the workspace 420 px; the
  breakpoints above are the contract, verified by screenshots at each width.

## Review amendments (EGM stages 6, 7, 9)

Reports: `.egm/reviews/workspace-shell/raw/{stage6,stage7,stage9}/`.

- **A1 (blocker, accepted)** — grid arithmetic was unsatisfiable (264+320+420+320
  = 1324 > 1024). Replaced with a measured contract: 1440+ → 264/452/320; ≤1439 →
  216/380/288; ≤1279 → rail 56 (icons) + 360 + 288; ≤1099 → rail 56 + 340, agent
  rail hidden with a toggle that opens it as a real column. Verified by rect
  self-test at 1440 (`chat=452 ws=628… agent=320`) and 1024 (`chat=340 ws=628
  agent=0`, no overlap).
- **A2 (blocker, accepted)** — no repo-root resolution existed. Chain now:
  `--repo` / `?repo=` → session `cwd` → `hermes:fs:gitRoot` → `git:scanRepos`
  picker (the picker remains, as the renderer had it).
- **A3 (blocker, accepted)** — deleting `#outputs` while `renderOutputs()` wrote
  its ids would throw on every resume. The window is rebuilt around one render
  model (`renderHeader/Transcript/Agent/Workspace/Panes`); ids exist once in the
  new rail.
- **A4 (blocker, accepted)** — `hermes:fs:readFileText` does not exist; the real
  channel is `hermes:readFileText` (commands.rs:491). Used.
- **A5 (accepted)** — `git:review:list` takes `(repo, scope, base)`; `review:diff`
  takes `(repo, file, scope, base, staged)` and returns a bare string. Both are
  called with explicit `scope: 'uncommitted'`, and `status` is mapped from the
  backend (`??`→A, `U`→M).
- **A6 (accepted)** — sidebar cron rows carry no schedule; the rail shows what the
  route returns and leaves the subtitle blank rather than inventing "Next: Mon".
- **A7 (accepted, reworded)** — capabilities cannot be "enabled/disabled" from a
  transcript. The section is labelled **Tools used** and marks a group on only
  when a tool from it actually ran; the rail keeps a separate static
  "Capabilities" list to mirror the reference's grouping.
- **A8 (accepted)** — tests come from tool output only (`N passed` summaries and
  per-file rows); without such output the panel says "no test output in this
  session". No fixture session on this machine has pytest output for the repo
  under review, so the panel was verified against a session whose transcript
  contains one (801 passed / 13.66s).
- **A9 (accepted)** — caps: diff renders ≤800 lines, code ≤400 lines, transcript
  ≤250 messages, logs ≤200 rows.
- **A10 (accepted)** — `.pane[hidden] { display: none !important; }`: a
  `display: flex` rule on `.pane` silently defeated the `hidden` attribute and
  stacked all four panes, which is what "not rendered properly" looked like.
- **A11 (accepted)** — no fake traffic lights; the top bar keeps a 78 px native
  inset instead of `TitleBarStyle::Overlay`.
- **A12 (accepted)** — window defaults to 1440×900, min 900×600, and accepts
  `--session`, `--repo`, `--width`, `--height` so a breakpoint can be reproduced
  without a resize automation permission.
- **A13 (accepted)** — stale fields: profile comes from the session's
  `info.profile_name`, not a hardcoded `default`; project/branch/PR are re-read
  per session.
- **A14 (deferred)** — plan mirror has no fixture session on this machine (no
  session carries `todo_state` or a `todo_*` tool call in a repo-backed run), so
  the self-test reports `0 plan items` honestly. The renderer and parser are in
  place; it needs a session that writes todos.
