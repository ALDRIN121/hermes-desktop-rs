# Workspace shell — design

Status: draft for EGM review. Target: `hermes-desktop-rs`, window label `task`.

## Problem

The current `ui/task.*` window is a three-column approximation: a fixed 288 px
sidebar, a transcript that the Outputs card is absolutely positioned over, and a
header of pills. It does not match the reference: no top navigation, no file
tabs, no editor, no diff/tests surface, no right agent rail — and the absolute
Outputs card overlaps the transcript at any width, so the window reads as broken
and does not adapt.

## Reference (accepted target)

A dense IDE-like shell, 1680×946 in the reference:

```
┌────────────────────────────────────────────────────────────── 50 px top bar ─┐
│ ✕✕✕  Hermes   [Chat] Code  Sessions  Tasks  Agents  Capabilities   GitHub ▾  │
│                                        main ↑  ●  ▣  ⚙  ⤴  ⋯  AJ              │
├───────────┬─────────────────┬──────────────────────────┬─────────────────────┤
│ rail      │ chat column     │ workspace                │ agent rail          │
│ 264 px    │ 452 px          │ flexible (644 px)        │ 320 px              │
│ New chat  │ title + repo    │ file tabs  (M / A)        │ Hermes Agent ● run  │
│ Search    │ messages        │ breadcrumb               │ Goal                │
│ Explore   │ plan card       │ code (line numbers)      │ Plan 5 steps        │
│ Inbox 3   │ exec card       │ [Diff 2][Tests][Term][Logs]│ Tools & Capabilities│
│ Pinned    │ summary         │ diff card                │ Context (tree)      │
│ Sessions  │ [+186 −12 view] │ tests card               │ Active jobs 2       │
│ Scheduled │ composer        │                          │                     │
│ Caps      │                 │                          │                     │
│ AJ        │                 │                          │                     │
└───────────┴─────────────────┴──────────────────────────┴─────────────────────┘
```

Elements to reproduce (structure only — no reference assets or CSS):

- **Top bar** (50 px): traffic-light space, Hermes mark + wordmark, nav pills
  Chat · Code · Sessions · Tasks · Agents · Capabilities, right cluster: GitHub
  pill, branch ↑, status dot, monitor, gear, share, ellipsis, avatar.
- **Left rail** (264 px): New chat (⌘K), Search, Explore agents, Inbox (badge),
  Pinned, Sessions (selected row highlighted, green dot), Scheduled jobs (two
  line rows with next run), Capabilities (with enabled/disabled), profile footer.
- **Chat column** (452 px): title + `repo · branch · N files changed` subtitle;
  user/agent messages with avatars and timestamps; **plan card** (`Plan · N
  steps`, ✓ / ▸ / ○ rows, active row tinted); **execution card** ("Executing…",
  `Running tests (4/5)`, elapsed, tool-activity row with monospace command and
  status dot); completion summary; **file-change bar** `2 files changed +186 −12`
  with **View diff**; composer with attachment row and model selector.
- **Workspace**: file tab strip (active tab, `M`/`A` badges), breadcrumb, code
  view with right-aligned line numbers, bottom tab bar **Diff (N) · Tests ·
  Terminal · Logs**, diff card (file header, `+28 −4`, hunk header, green/red
  line backgrounds, gutter), tests card (`8 passed`, `0 failed · 0 skipped`,
  duration, per-file rows with durations).
- **Agent rail** (320 px): `Hermes Agent` + running pill + pause/close; Goal;
  Plan mirroring the chat plan; Tools & Capabilities with enabled state;
  Context tree for the repo; Active jobs with next-run lines.

## Data sources (all live; no fabricated values)

| Region | Source |
| --- | --- |
| Sessions, scheduled | `/api/profiles/sessions/sidebar?recents_profile=default` via `hermes:api` |
| Title, branch, repo, model, running, elapsed | `session.resume` → `info` |
| Messages, tool calls | `session.resume` → `messages` |
| Plan | `todo_state`, else latest todo-tool call arguments |
| Changed files + counts | `hermes:git:review:list` (+ `git diff --numstat`) |
| Diff text | `hermes:git:review:diff` per file |
| File content | `hermes:fs:readFileText` (when the session cwd holds it) |
| File tree | `hermes:fs:readDir` on the repo, depth-capped |
| Tests | parsed from transcript tool output (`N passed`, `M failed`, per-file rows) — shown only when the session actually ran tests; otherwise "no test output in this session" |
| Tools enabled | tool names present in the transcript; unknown tools are not listed |
| Active jobs | cron sessions with `schedule` |
| PR | `hermes:git:review:shipInfo` |
| Resources | `task_stats` (gateway pid, cpu, rss) + `usage` |

## States

- **No session**: rails render, chat shows an empty state, workspace shows
  "select a file", no fake numbers.
- **Session, no changes**: file tabs empty state, Diff (0) disabled, tests
  "no test output".
- **Session with changes**: tabs list files, first file selected, diff rendered.
- **Running**: status dot + elapsed ticking; polling every 4 s; idle 15 s.

## Responsiveness

- Grid: `264px | minmax(320px, 452px) | minmax(420px, 1fr) | 320px`, top bar 50 px.
- Breakpoints: < 1440 px hide the agent rail's body behind a toggle (rail 56 px
  icon strip); < 1180 px collapse the chat column to 360 px; < 1000 px hide the
  left rail to icons and let the workspace take the rest; window min 960×600.
- Every pane scrolls independently (`min-height: 0`, `overflow: auto`); no
  absolutely positioned card may overlap another pane.
- Long strings truncate with ellipsis + `title`, never widen a column.

## Non-goals

Voice, payments, canvas, browser preview, and the Cursor-style multi-file editor
tabs beyond the changed-file set.
