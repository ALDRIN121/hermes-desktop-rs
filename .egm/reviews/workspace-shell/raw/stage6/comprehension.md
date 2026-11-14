# Comprehension review — Workspace shell

Stage 6 (cold read). Target: `docs/designs/WORKSPACE_SHELL.md` (100 lines), companion
`docs/plans/WORKSPACE_SHELL_PLAN.md` (100 lines). Grounding: `ui/task.html|css|js`,
`src-tauri/src/main.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/git_ops.rs`,
`ui/codex.*`, and the reference image
`dashboard_20261114_191700_53a88d0f_image_2abc6a.png` (1672×941 px as measured on the file).

## What this proposes

Replace `ui/task.*` (window label `task`) with a four-column IDE-like shell: a 50 px top bar
(traffic lights, Hermes mark, six nav pills, a repo pill and right-hand controls), a 264 px
left rail (new chat, search, explore/inbox, pinned, sessions, scheduled jobs, capabilities,
profile), a 452 px chat column (title header, messages, plan card, execution card, completion
summary, file-change bar, composer), a flexible workspace column (file tabs, breadcrumb, code
with line numbers, a Diff/Tests/Terminal/Logs tab bar, diff card, tests card), and a 320 px
agent rail (Hermes Agent status, Goal, plan mirror, tools, repo context tree, active jobs).
All values are supposed to come from live gateway sources — `/api/profiles/sessions/sidebar`,
`session.resume`, `hermes:git:review:*`, `hermes:fs:*`, `task_stats` — with four states and
three breakpoints, and no fabricated numbers.

## How the existing system works, per this document

The current window is a "three-column approximation": a fixed 288 px sidebar, a header of
pills, and a transcript with an absolutely positioned Outputs card that overlaps it. The
document says the window has no top navigation, no file tabs, no editor, no diff/tests
surface and no agent rail, and that the overlay makes it read as broken and non-adaptive. It
does not describe what each existing element reads from, how the window is opened, or what
happens to the Outputs card's contents; the data-source table is the only account of where
numbers come from, and it lists sources without saying which region consumes each one.

---

## A. Regions of the reference the spec does not mention or under-specifies

### 1. The bottom tab bar is not actually a tab bar — **Blocker** — §Reference (workspace), lines 51-55; Plan items 18-22

In the reference, `Diff (2)` is the active tab **and** both the diff card and the tests card
are visible at once, stacked in one scroll column, with nothing indicating they are alternate
views. The design calls them a "bottom tab bar" (implying exclusive switching); the plan says
"Diff default when changes exist" (also implying exclusivity) but then specifies four panels
as if all existed. An implementer cannot know whether to build four exclusive panels, a
scrolling stack with the tabs as anchors, or a per-file selector.

**Recommend:** state explicitly which it is; if the tabs switch content, say what the Diff
card and Tests card are doing on screen together in the reference.

### 2. Execution card is under-specified and has no defined end — **Major** — §Reference (chat), lines 46-49; Plan item 12

The reference card shows a header icon + `Executing...`, subtext `Running tests (4/5)`, `42s`
right-aligned, a `Tool activity` label, and a row `Running pytest -q` ending in a green
**check** (not a dot, as the design says). The spec names four fragments and no lifecycle:
what `(4/5)` counts (plan steps? tool calls?), where the elapsed value comes from, and what
replaces the card when the run ends — the reference shows it alongside a completed summary.
Plan item 12 says "shown while `running`", which would delete the tool-activity row the
moment the run finishes, losing the only record of it in the chat.

**Recommend:** enumerate the card's fields with their sources and add the completed/failed
form of the card (or state that it is removed and where its content moves).

### 3. Plan card's visual contract is reduced to three glyphs — **Major** — §Reference (chat), line 46; Plan item 11

The reference card has a bordered container, a header row with an icon, `Plan` and `5 steps`
right-aligned, three completed checks, an **active row tinted with a filled ring** (not the
`▸` the design specifies), and a pending hollow circle. The design's `✓ / ▸ / ○` legend
describes states by glyph, and the current code emits exactly `✓ / ▸ / ○`
(`ui/task.js:532-540`), so an implementer will reproduce the triangle and miss the tint and
ring. The design also says `Plan · N steps` while the current code renders `done/total`
(`ui/task.js:524`) — two different strings for the same slot.

**Recommend:** describe the three markers by state and treatment (colour, tint, ring), and
fix the counter format to one string.

### 4. Diff card: three visible affordances are missing — **Major** — §Reference (workspace), lines 52-53; Plan item 19

The reference's diff file header carries a **copy-path icon** at the far right; unchanged
context lines are rendered in a neutral style with their own gutter numbers; the hunk header
includes the function context after `@@`; and only the first hunk is shown, with no indication
whether more hunks follow. The design lists "file header, +28 −4, hunk header, green/red line
backgrounds, gutter" and stops. It also never says what the diff shows for a deleted file or
a binary file, although Plan item 16 introduces a `D` badge.

**Recommend:** list the card's parts (including the copy affordance and context-line style)
and state the deleted/binary/no-hunk behaviour.

### 5. Tests card: provenance and parts are unstated — **Major** — §Reference (workspace), lines 54-55; Plan item 20

The card has a flask icon, a `Tests` heading, a green `8 passed` pill, `0 failed · 0 skipped`
and `6.4s` on the right, then per-file rows with a check, the path, `(4 passed)` and a
duration. The design names the totals and "per-file rows with durations" but not the pass
counts, the icons, or the pill. More importantly it never says whether the card reflects the
**last** test run, an aggregate over the session, or all runs; the plan says "parse
pytest/jest summaries from tool output" without saying which formats are supported or what
happens when both appear.

**Recommend:** state the source run (last matching tool result), the supported output
formats, and the row anatomy.

### 6. Left rail: half the rows in the reference have no definition — **Major** — §Reference (left rail), lines 42-44; Plan item 7

Visible but absent or one-word in the spec: section headers (`Pinned`, `Sessions`,
`Scheduled jobs`, `Capabilities`), four pinned items each with its own icon and label, the
`Inbox` badge value (`3`), the `+ Add capability` row, and the profile footer's **name plus
ellipsis** (the spec says only "profile footer"). `Search` and `New chat` both display `⌘K`
in the reference — the same shortcut twice — which the spec does not flag, so it will be
copied. The plan resolves this by disabling Explore/Inbox and filling Pinned from "config",
which contradicts the design's own "all live; no fabricated values" heading.

**Recommend:** enumerate the rail's rows with their trailing accessories and the shortcut per
row, and either name a source for pins/inbox or remove them.

### 7. Agent rail: three blocks are named but not specified — **Major** — §Reference (agent rail), lines 56-58; Plan items 24-29

`Goal` has an edit (pencil) icon that the design does not mention and the plan does not make
inert or live. `Tools & Capabilities` has a disclosure chevron in the reference, implying
collapsibility, and six rows with mixed on/off marks. `Context` shows a root row
`TokenHub (repo)` plus three entries and one folder — the design says "Context tree for the
repo" without saying where the root label comes from, what the depth is, or that the term
collides with the existing "Context" (token-window usage) in today's Outputs card. `Active
jobs` shows a `2` count badge the design omits.

**Recommend:** specify each block's header, affordances and rows, and rename the repo tree
(e.g. "Repository") to avoid the existing meaning of "Context".

### 8. Composer: the two selectors are collapsed into one — **Minor** — §Reference (chat), line 50; Plan item 14

The reference shows a left control cluster (`+`, `@`, image, globe), an `Agent` mode pill and
a separate model pill `GPT-6.1 (High)` with a dropdown, then a solid send button. The design
says "composer with attachment row and model selector"; the plan says "textarea + agent
selector (static)". Two distinct controls in the mock, one in each document, with different
names and different liveness.

**Recommend:** list the composer's controls, and mark each `live` or `static`.

### 9. The status line disappears without a replacement — **Minor** — §States (absent); `ui/task.html:132`, `ui/task.css:254`

Today the only non-modal feedback for transport problems is `#status`
(`loading sessions…`, `resume failed`, `submit failed: …`, `submitted · queued`). The new
layout has no status line and the design never says where that information goes, so a failed
resume becomes an empty pane with no explanation.

**Recommend:** name the surface that reports data-source failures, or keep the status line
and give it a home in the grid.

### 10. Panel chrome and the icon strip are not described — **Minor** — §Reference (whole frame), line 37

The reference's four panels are separated by hairline borders at x≈246/306, 753, 1337 and
1665, with a ~9 px gutter between the top bar and the panels (the bar's border sits at
y≈46-47, panels start at y≈56). The ASCII diagram shows flush columns with no gutter, no
rounded corners and no border tokens, and the document says "structure only — no reference
assets or CSS", which does not say whether panel chrome is in or out of scope. Separately,
the reference's left column is not one panel: a full-height divider at x≈246 splits it.

**Recommend:** one line on panel chrome (flush grid vs inset cards, radius, border token) or
an explicit "visual chrome out of scope", and confirm whether the left column has one panel
or two.

### 11. File tab strip: ordering and badge colours are missing — **Minor** — §Reference (workspace), line 51; Plan item 16

The reference shows `tokenhub/cli.py` (`M`, yellow/orange) **before** `tokenhub/exporter.py`
(`A`, green) — not alphabetical — with no close affordance and a `+` button. The design says
"tabs list files, first file selected" without stating the order, badge colours, or that tabs
have no close button.

**Recommend:** state the ordering rule (git status order, not path order), the badge colours,
and the default selection.

### 12. Message rows change shape without being specified — **Minor** — §Reference (chat), line 45; Plan item 10

The reference renders the user turn left-aligned as plain prose with an avatar and a
`You 6:21 PM` label, and the agent turn with the Hermes mark and `Hermes (Agent) 6:21 PM`.
Today the user turn is a right-aligned blue bubble with a bare timestamp (`ui/task.css:126-140`,
`ui/task.js:182-186`). The design says only "user/agent messages with avatars and timestamps",
so the bubble-to-row change is invisible to a reader and untested.

**Recommend:** specify left-aligned rows, no bubbles, and the role labels.

---

## B. Claims I cannot verify or trust

### 13. "1680×946 in the reference" and the four column widths — **Minor** — §Reference, lines 16 and 24; §Responsiveness, line 89

The file is **1672×941**. Measured panel borders are at x≈246/306, 753, 1337 and 1665, i.e.
chat ≈447 px (matches 452 within 5), but left rail ≈306 px (spec: 264), workspace ≈584 px
(spec: 644) and agent rail ≈328 px (spec: 320). The four quoted numbers sum to 1680, which is
8 px wider than the artifact — they were derived from the target width, not measured.

**Recommend:** present 264/452/644/320 as the **design targets for a 1680 px window** and stop
calling them measurements of the reference.

### 14. "50 px top bar" — **Minor** — §Reference, lines 19, 39; §Responsiveness, line 89

The artifact's top-bar bottom border is at y≈46-47 and the content panels begin at y≈56, so
the bar reads as ~46 px plus a ~9 px gutter, not 50 px with flush content. Either the height
or the gutter is unstated.

**Recommend:** state bar height and inter-panel gutter as separate numbers.

### 15. "all live; no fabricated values" cannot hold for four regions — **Major** — §Data sources, line 60; Plan item 7

The data table covers sessions, scheduled jobs, title/branch/repo, messages, plan, changed
files, diff, file content, file tree, tests, tools, active jobs, PR and resources. It does
**not** cover: pinned chats, the Inbox badge count, "Explore agents", or any capability
enabled/disabled state — all of which the reference shows with concrete values. Plan item 7
then fills Pinned from "config" and disables Explore/Inbox, which is fabrication with a
different name.

**Recommend:** add a source per visible value or delete the value from the region list; if a
row is decorative, say "not live, inert".

### 16. The reference's own numbers do not reconcile — **Major** — §Reference, lines 28-31, 49-55

`2 files changed +186 −12`, of which `tokenhub/cli.py` is `+28 −4`; `exporter.py` is marked
`A` (new file) so it must carry `+158 −8`, which is plausible only for a large new module.
The code view shows ~5 added lines of the 28. `Diff (2)` implies two files but the diff card
shows one file's single hunk. The tests numbers *are* internally consistent (`4+3+1 = 8`,
`3.1+2.4+0.9 = 6.4s`). The spec presents the whole frame as an accepted target without saying
which values a build is expected to reproduce.

**Recommend:** mark the illustrative numbers and list which the acceptance checks assert
(`review:list` counts, test summary parsing), and which are only illustrative.

### 17. "elapsed ticking" has no source — **Major** — §States, line 85; §Data sources, line 65

`session.resume → info` exposes `started_at` and `last_active_at`, and the current code renders
`last_active_at − started_at` as a **fixed** duration (`ui/task.js:268-271`, `499-501`). A
ticking clock needs a run-start timestamp and a client timer; neither exists in the listed
sources. "Polling every 4 s; idle 15 s" is already what the code does (`ui/task.js:710`), so
that part is verifiable; the tick is not.

**Recommend:** define the timestamp the tick counts from (and where it comes from), or drop
"ticking" and show a duration.

### 18. Tests / Tools / Terminal / Logs all rest on a source the repo does not read — **Major** — §Data sources, lines 72-73; Plan items 20-22

Three of the four bottom panels and both rails' tool lists are "parsed from transcript tool
output". The current renderer explicitly discards non-user/non-assistant messages
(`ui/task.js:173`) and nothing in `ui/` or `src-tauri/` parses tool results, so no code in the
repo has ever demonstrated that `session.resume` returns tool output in a parseable form. The
design treats this as an established source.

**Recommend:** prove the payload (a sample `session.resume` message with a pytest summary) and
quote its shape, or downgrade these panels to "unavailable until the payload is confirmed".

### 19. "Current window … does not adapt" — **Minor** — §Problem, lines 7-12

The grid is `64px 288px minmax(0,1fr)` (`ui/task.css:27`), so the chat column *does* flex; the
non-adaptation is the fixed 300 px absolute card (`ui/task.css:223-234`), which never reflows
or collapses. "Does not adapt" overstates it and hides the actual defect an implementer has to
remove.

**Recommend:** say the overlay is fixed-width and always covers the transcript's right side,
rather than that the window does not adapt.

### 20. "a 40-line lexer covers strings/comments/keywords" — **Minor** — Plan, Risks, line 98

A line count is not a scope. It says nothing about languages, multi-line constructs,
heredocs, or what happens at 200k lines, and the design never asks for highlighting at all.

**Recommend:** replace the estimate with a scope statement (languages, max file size, plain
fallback) or move highlighting to Non-goals.

---

## C. Ambiguities a fresh implementer would have to guess about

### 21. The breakpoints cannot satisfy the plan's own acceptance widths — **Blocker** — §Responsiveness, lines 89-92; Plan items 1, 5, 33

Minimum column sum is `264 + 320 + 420 + 320 = 1324`. At 1440 (a plan screenshot width) the
media query has not fired (`< 1440` is false), so the minimum is 1456 → 16 px overflow. At
1280 the agent rail must already be a 56 px strip (264+452+420+56 = 1192). At 1024 the
arithmetic fails outright: with the rail at 56 px and the chat at its collapsed 360 px,
`264 + 360 + 420 + 56 = 1100 > 1024`; the left rail only collapses below 1000. So Plan item 5
("at 1024×700 the four columns are present, no overlap, no horizontal scrollbar") is
impossible as written, and the 1000–1100 px band has no valid layout at all.

**Recommend:** publish a per-width table (which column collapses, to what width, at which
breakpoint) whose sums are ≤ the width at 1680/1440/1280/1024/960, and make at least one
breakpoint drop a column instead of shrinking one.

### 22. The state model is missing — the reference shows states that cannot co-exist — **Blocker** — §States, lines 80-85; Plan item 12

The four listed states (no session / no changes / changes / running) do not cover what the
reference displays simultaneously: a running execution card (`Executing… Running tests (4/5)
42s`), a finished summary (`Tests passed`, `8 unit tests (all passing)`), a plan with 3 of 5
steps done and step 4 active, a tests card with 8 passed, and `Diff (2)`. Nor is there any
state for completed, failed, resumed-mid-run, gateway-unreachable, send-failed, or gh-missing
— all of which the current code handles (`ui/task.js:366`, `402`, `427`, `643`).

**Recommend:** replace the list with an explicit state matrix (idle/running/completed/failed ×
has-session/has-changes/has-tests) saying what each pane shows, and state that the reference
is a composite mockup no single session can produce.

### 23. Long lines: three different rules — **Major** — §Responsiveness, line 95; Plan item 19; reference

The design says "long strings truncate with ellipsis + `title`, never widen a column". The
plan says "horizontal scroll for long lines (no wrapping)". The reference shows code clipped
at the panel edge with **no ellipsis and no scrollbar**. Truncating source code with an
ellipsis destroys the text you are trying to read and copy, so the design's global rule is
wrong for the code and diff panes.

**Recommend:** split the rule by surface — code/diff scroll horizontally, labels/rows ellipsis
+ title — and say which side the line-number gutter sits on (the reference is left, right-aligned).

### 24. "Terminal" means two different things — **Major** — §Reference (workspace), line 52; Plan item 21

The design lists a `Terminal` tab in an IDE shell and puts interactive terminals out of scope
nowhere; the plan ships a read-only "last command + output tail … labelled as a log". A fresh
implementer reading the design will build (or promise) a terminal.

**Recommend:** rename the tab in the design (`Output`/`Terminal log`) and state the read-only
constraint there.

### 25. Goal: paraphrase or truncation? — **Major** — §Reference (agent rail), line 56; Plan item 25

Plan item 25 says Goal is the first user message, truncated. The reference's Goal is a
rewritten sentence ("Add usage export command to TokenHub with filters and tests.") for a much
longer prompt. A truncation would show the prompt's opening words instead.

**Recommend:** state that Goal is a paraphrase and where it comes from (no gateway field
provides one), or show the truncated first message in the mock.

### 26. "Mirrors the chat plan" is contradicted by the reference — **Major** — §Reference, line 57; Plan item 30

The rail's steps are shortened forms of the chat's steps ("Review current CLI structure" vs
"Review current CLI structure and data model"), yet Plan item 30's acceptance criterion says
"rail mirrors chat plan **exactly**".

**Recommend:** say whether the rail wraps, truncates, or re-words, then drop "exactly".

### 27. Chat column scroll model and small-height behaviour — **Major** — §Responsiveness, lines 93-94; Plan item 2

The design says every pane scrolls independently, but the chat column contains a fixed header,
a plan card, an execution card, a summary, a file-change bar and a composer, and nothing says
which of them scroll (messages only?) or whether the composer is pinned. Plan item 2 permits
`position: absolute` for the composer without the design saying so. At the 600 px minimum
height, header + plan card + exec card + summary + composer can exceed the column with no
stated behaviour.

**Recommend:** state the column's scroll containment (header fixed, message list scrolls,
cards part of the scroll, composer pinned) and the degraded layout at minimum height.

### 28. Non-repo, no-gh and empty states are not carried over — **Major** — §States, lines 80-84

Every data source can fail, and the current window has explicit fallbacks the design silently
drops: repo picker for a cwd that is not a repository (`ui/task.js:562-587`), "working tree
clean", "not a repository", "no gh → no chip" (`ui/task.js:643`), `—` for stats. The new
design offers only "select a file" and "no test output".

**Recommend:** one line per source for its failure/empty rendering, reusing the strings that
already exist.

### 29. No accessibility or keyboard contract — **Major** — all regions

Nothing on focus order between four panes, focus rings, `tablist`/`tab` roles for the two tab
strips, keyboard access to sessions, `aria-live` for running/elapsed updates, or Escape. The
reference also binds `⌘K` to both New chat and Search.

**Recommend:** add a short interaction contract (tab order per pane, roles for both tab
strips, single `⌘K` owner, focus-ring token, `aria-live` on the status text).

### 30. Transcript history cap and scroll behaviour — **Minor** — §Reference (chat), line 45

The current renderer caps at 250 messages with "N earlier messages not shown" and scrolls to
the bottom (`ui/task.js:159-168`, `220`). The design says "messages" and nothing else.

**Recommend:** carry the cap and auto-scroll rule into the design, or state that the list
virtualises.

### 31. Deleted/binary file selected — **Minor** — §Reference (workspace), line 51; Plan item 16

A `D` badge exists in the plan, but the code view and diff have no defined rendering for a
deleted file, an untracked binary, or a file outside the session cwd (the data table's parenthetical
"when the session cwd holds it" is the only hint and it is not a rule).

**Recommend:** state what each tab shows for those three cases.

### 32. "No horizontal scrollbar" scope — **Minor** — Plan item 5 vs item 19

Acceptance forbids a horizontal scrollbar at 1024; the diff pane is required to scroll
horizontally for long lines. Two different scrollbars.

**Recommend:** scope the rule to the page/shell, not the panes.

---

## D. Contradictions between the spec, the plan, and `ui/task.*`

### 33. Two live data rows and two self-tests lose their DOM — **Blocker** — §Data sources, lines 75-76; Plan items 2, 32

`Resources | task_stats (gateway pid, cpu, rss) + usage` and `PR | hermes:git:review:shipInfo`
remain in the data table, but no region in the design renders either: the top bar, header,
rails, workspace and composer sections never mention a PR chip, a resource readout, or token
usage. Plan item 2 deletes `#outputs` (where both live today, `ui/task.html:80-96`,
`ui/task.js:657-668`, `626-654`), and Plan item 32 keeps "the existing … resources/PR checks",
which assert exactly that deleted DOM (`task:resources` reads `#res-cpu/#res-mem/#res-tokens`,
`task:pr` reads `#pr-pill/#pr-action`, `ui/task.js:776-785`). As written, Phase 0 makes Phase 5
unpassable.

**Recommend:** either name the region that shows PR and resources (the reference shows
neither), or delete both rows from the data table and both checks from the plan.

### 34. `hermes:fs:readFileText` does not exist — **Blocker** — §Data sources, line 70

The registered channel is `hermes:readFileText` (`src-tauri/src/commands.rs:491`,
`src-tauri/src/bridge.js:105`, `crates/hermes-contract/src/channels.rs:204`), returning
`{text, path, …}`. There is no `hermes:fs:readFileText` anywhere in the tree; a literal
implementation of the table throws an unknown-channel error, and the code view silently
renders nothing.

**Recommend:** correct the channel name and cite the call signature (`bridge.js:105`).

### 35. The window inventory is unresolved — and a second workspace window already exists — **Blocker** — §Status line 3; §Problem; Plan Phase 0 and 4

`main.rs` opens `main` (`index.html`, 1360×900) on every launch and opens `task` only from the
tray or `--task` (`main.rs:157-175`, `235-237`); a separate window with label **`workspace`**
already exists (`main.rs:55-83`, `ui/codex.html`), implementing changed files, a unified diff
(`ui/codex.js:120`, `201` via `hermes:git:review:diff`), stage/unstage/revert and a repo picker.
The design adds a "Workspace" **pane** that duplicates that window and never says whether
codex.html is retired, merged, or kept, nor whether `task` becomes the launch surface. The
word "workspace" now names two different things, one of which is a window label.

**Recommend:** state the window inventory after this change (which windows exist, which opens
at launch, what happens to codex.html) and rename the pane.

### 36. The default window size is never set, and today's default hides the agent rail — **Major** — Plan item 4; `main.rs:170-171`

Plan item 4 changes only the minimum (to 960×600). The current task window is
`inner_size(1240, 820)`, which is below the design's own `< 1440` breakpoint, so the reference
four-column layout would never be what a user sees on open; the plan's screenshots omit
1240×820 entirely.

**Recommend:** set the default inner size in the plan and add the default size to the
screenshot set.

### 37. Existing functionality is dropped with no record — **Major** — §Problem, §Non-goals, lines 97-99

Today's window renders: subagent dots and goals (`delegate_task`), a Sources list of URLs from
the transcript, a context meter with used/max and cache-hit %, message and tool-call counts, a
progress bar, a Run log tab, a repo picker, a PR chip with "Open PR", and the status line
(`ui/task.html:67-132`, `ui/task.js:223-346`). The design keeps none of these except a tools
list; the Non-goals mention voice, payments, canvas, browser preview and extra editor tabs —
not any of the above.

**Recommend:** add a "removed or relocated from today's window" table, each item marked
removed or moved, so the deletions are decisions rather than accidents.

### 38. The capability-state rule cannot produce the reference — **Major** — §Data sources, line 73 vs §Agent rail, lines 56-57

"Tools enabled: tool names present in the transcript; unknown tools are not listed" makes
every listed tool enabled by construction — a disabled row is unrepresentable. The reference
shows two disabled capabilities, and the two rails disagree about the list itself (`Browser
(Playwright)` on the left vs `Web / Playwright` on the right; `HTTP / APIs` only on the left),
so they cannot be the same source. The design also says the **left** rail shows capabilities
"with enabled/disabled", while the reference's left rail shows no state marks at all.

**Recommend:** name the capability catalog and the policy that makes a row disabled (the
gateway has capability/plugin metadata), or drop the disabled state and the second list.

### 39. Five of six nav pills are inert, and the design does not say so — **Major** — §Reference (top bar), lines 39-41; Plan item 6

The design presents Chat · Code · Sessions · Tasks · Agents · Capabilities as navigation;
the plan makes all but Chat `inert` with `title="not ported"`. A reader of the design alone
would build routing; a reader of the plan alone would ship five dead buttons with a tooltip.

**Recommend:** mark the inert items in the design itself (and say what "not ported" means to
a user).

### 40. Timestamps: design silent, plan relative, reference and code absolute — **Major** — §Reference (chat), line 45; Plan item 10; `ui/task.js:49-50`, `215`

The reference shows `6:21 PM`; the current code formats `HH:MM` via `clock()`; the plan says
"relative timestamp" and the code's `relative()` is used for session rows only. Three
positions, one slot.

**Recommend:** one format per surface (absolute for messages, relative for session rows) stated
in the design.

### 41. The session list's composition and limits are not the ones in the code — **Minor** — §Data sources, line 64; `ui/task.js:355-362`, `110`, `136`

The design names one endpoint; the code merges `payload.recents.sessions` **and**
`payload.messaging.sessions` into one list, renders at most 60 session rows and 6 cron rows,
and passes `limit=40` and `recents_profile=<hardcoded 'default'>`. The design's example URL
omits `limit` and hardcodes nothing, but also never says whether messaging sessions belong in
the list or how many rows render.

**Recommend:** state the list's composition (recents + messaging?), its cap, and that the
profile comes from the window's ticket rather than a literal.

### 42. `#rail` already means something else, and the 64 px icon strip vanishes — **Minor** — §Reference (left rail); `ui/task.css:27`, `32-58`; Plan item 7

The current `#rail` is a 64 px icon strip (active/plus/refresh/avatar) that the new design
deletes without saying so, while the plan calls the 264 px column the "left rail". A port of
existing code will attach the wrong styles to the wrong element.

**Recommend:** rename the new column (e.g. "session pane") and record the icon strip's removal.

### 43. `review:list` already carries the numstat, and the scope is unspecified — **Minor** — §Data sources, line 68; `git_ops.rs:393`

`hermes:git:review:list` returns `{files:[{path, added, removed, status, staged}], base}`, so
"(+ `git diff --numstat`)" implies a second call that is unnecessary, and the design never
says which scope/base the workspace renders (`review_diff` takes
`(repo, file, scope, base, staged)`), while the code calls it with `'uncommitted'`.

**Recommend:** drop the numstat parenthetical and state the scope (`uncommitted`) the Diff tab
uses.

---

## Severity summary

| Severity | Count | Items |
| --- | --- | --- |
| Blocker | 6 | 1, 21, 22, 33, 34, 35 |
| Major | 22 | 2, 3, 4, 5, 6, 7, 15, 16, 17, 18, 23, 24, 25, 26, 27, 28, 29, 36, 37, 38, 39, 40 |
| Minor | 15 | 8, 9, 10, 11, 12, 13, 14, 19, 20, 30, 31, 32, 41, 42, 43 |

Total: 43 findings.

## Verdict

Could a competent developer start from these two files without inventing a decision? **No.**
The layout's own arithmetic fails two of its acceptance widths, the bottom tab bar and the
run-state model are undefined, two data rows and two self-tests have no home after Phase 0,
and one data-source channel does not exist. The region inventory is also incomplete enough
that a build would silently drop subagent visibility, context telemetry, PR state and every
error path that exists today.
