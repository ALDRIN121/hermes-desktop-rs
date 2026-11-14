# Hermes UI reliability and workspace design

> Historical limited-scope proposal. Superseded for architecture and feature scope by the [full Hermes design](../specs/2026-11-15-full-hermes-design.md) and [full Hermes plan](../plans/2026-11-15-full-hermes-plan.md). Do not execute the exclusions or vanilla-only structure below as the full-product destination.


**Date:** 2026-11-14. **Status:** proposal for user review — **not approved for implementation**. **Baseline:** `0c272d7`.

This specification describes the recommended first implementation scope. It is the assistant's proposal, not a record of user-approved design choices. The user requested the audit, image generation/refinement and planning, and explicitly reserved implementation approval.

## Intent

Make Hermes a dependable place to read a conversation, supervise an agent and review its local changes. The user should be able to answer: Which session/profile/repository is this? What is happening now? How fresh is this information? What changed? What will this button do?

Use the current Rust/Tauri shell and editable custom HTML/CSS/JavaScript. Do not rewrite the shipped Classic bundle. Reference: [review and evidence](../../reviews/2026-11-14-ui-audit/REVIEW.md), [UI ledger](../../reviews/2026-11-14-ui-audit/FEATURE_LEDGER.md), [channel ledger](../../reviews/2026-11-14-ui-audit/CHANNEL_COVERAGE.md), [generated images](../../reviews/2026-11-14-ui-audit/VISUAL_PROPOSALS.md).

## Goals and boundaries

- Preserve unique searchable sessions, reliable text submission, interruption and explicit main-agent model scope.
- Make errors, loading, disconnected, stale, stopping, cancelled, failed and unknown states visible.
- Keep session content, draft, repository and response identities coherent across asynchronous work.
- Provide full retrievable transcript/file/diff content; distinguish a display cap from complete data.
- Present run evidence with its provenance; do not infer successful completion from idle alone.
- Make local review mutations specific, reviewable and accurately labelled.
- Make native reopen and routing dependable and preserve existing file-read protection expectations.
- Keep Classic accessible as a clearly labelled legacy surface with explicit known limitations.

Initial scope excludes full Classic parity, cloud/multi-connection management, creating/managing agents, capability/plugin installation/OAuth, scheduled-job mutation, true interactive terminal in Task, remote media/stream seeking, voice, pets/HUD/quick entry and binary/multimodal upload. These are explicit deferred items, not functioning features represented by mock controls. Classic receives a shared bridge-level unsupported-operation diagnostic and only already-consumed capability flags; per-control hiding/disablement across its compiled screens is deferred, because generated Classic assets are not edited.

## Global constraints

- Implementation requires explicit user approval of this packet.
- Retain Rust/Tauri and the custom vanilla JavaScript/HTML/CSS surfaces; do not edit generated Classic assets.
- No dependency installation or toolchain migration is required by this proposal.
- Never retry a mutation automatically after an ambiguous delivery or timeout.
- Never display success, a clean tree, or completed tests when the result is unknown or retrieval failed.
- All mutation targets must include the captured session/profile/repository identity.
- No real user repository or live account is used for destructive acceptance tests.
- All hidden panels remain reachable by labelled controls and keyboard navigation.
- Generated images are visual concepts; this written specification governs behavior.

## Product structure and navigation

Task becomes the documented primary window. Workspace remains a separate repository-review window sharing review behavior; **Classic Hermes…** is an explicit fallback. Normal launch, tray “Open Hermes” and normal tray click should resolve to primary Task. A separate Classic tray item opens Classic. Focus/show/unminimize an existing Task without navigating on a plain reopen.

The primary toolbar shows repository (or **No repository**), branch (or **Unavailable**, not a successful empty state), profile identity, connection state and a labelled **Run details** toggle. **Chat / Split / Changes** controls describe real layout modes. Split is offered only where width supports it.

The session rail has New chat, Search chats, unique sessions grouped by recency, Scheduled jobs, Agents used, Tools used and Classic Hermes. No Inbox duplicate, fake capabilities catalogue or Tasks label that actually opens a Git window. Scheduled jobs is read-only inspection; Agents used and Tools used are explicitly current-session evidence. Full management remains deferred.

Deduplicate session rows by `(profile, session_id)`; retain the newest available activity record and merge useful fields. Sort before grouping. Session labels and scope must remain available to assistive technology even when the rail collapses. Cmd/Ctrl+K opens/focuses search; advertise that beside Search. Cmd/Ctrl+N creates a chat with an in-flight guard.

## Layout and tokens

| Task viewport, logical CSS pixels | Default structure | Panel behavior |
| --- | --- | --- |
| ≥1600 | 224px rail, conversation min 480px, changes min 480px; optional 288px docked inspector only if both main minimums still fit | Chat/Split/Changes modes; inspector toggle genuinely opens/closes |
| 1280–1599 | 224px rail; Chat or Changes full content, Split only when both main minimums fit | Inspector is an accessible side drawer, never a squeezed extra column |
| 900–1279 | 56px icon rail plus one fluid main Chat or Changes pane | Chats opens a searchable named-session drawer; Run details opens drawer; returning restores prior selection/draft |
| Minimum window | 900×600, retaining current native floor | Check overflow and recovery at minimum, not just at wide screenshots |

At 1280px, 224 + 480 + 480 = 1184, leaving 96px for dividers/gutters; the implementation must compute actual usable width and omit Split if it cannot preserve those minimums. At ≥1600 the inspector is optional; it must not force those minimums to fail. Do not animate or hide content through contradictory CSS classes. Use explicit state for mode/drawers.

Workspace retains its separate existing 640×420 native minimum. Below 900px it uses a single file-list or selected-diff view with labelled Back to files navigation and reachable review actions; verify 640×420 explicitly rather than silently raising its minimum.

Token proposal: canvas `#10141C`, panel `#171D28`, raised/selected surface `#222B3A`, border `#303B4A`, text `#EDF2F7`, secondary text `#A1ADBF`, accent `#7878F5`; teal success, amber stale, red failure/discard. Semantic colors also have labels/icons. Keep the native system sans family; use SF Mono/Menlo for code. Body/transcript 14px, controls 13px, metadata minimum 12px; line height approximately 1.5. Dividers convey pane boundaries; 6–8px corners only where useful. Reset native button fills consistently. Focus ring contrast and text contrast must be measured on rendered surfaces; artwork is not verification.

Use 32px minimum desktop control hit areas; icon buttons have accessible names. Every pane scrolls independently. Code/diff horizontal scrolling is contained. Respect reduced motion. Avoid repeatedly auto-scrolling readers who are away from the end.

## State, drafts and core chat

Use explicit request identity (session/profile/repo/file plus generation) and reject obsolete responses. Session identity is captured before an await; it never changes as an incidental effect of a response. “No repository” is valid; do not silently select the first scanned repository. A manual repo override is scoped to the selected session and clearly shown.

Connection states: connecting → connected → stale/reconnecting → connected or unavailable. Mark snapshot stale after two consecutive failed polls or more than 10 seconds without a successful active-run poll; show last successful update time. Idle polls run every 15 seconds, active every 4 seconds; repository refresh follows a successful active snapshot and after local mutations/manual Refresh, with at most one pending refresh per context. Retry reads with a visible Retry action; an acknowledged mutation is never repeated by polling. A failed poll does not mark a run completed.

Drafts and selected file references persist per `(localBackendKey, profile, session_id)` in local storage; a new-chat draft has a separate key. `localBackendKey` is a non-secret stable namespace derived from the canonical configured Hermes home; it prevents another local home’s default profile reusing drafts. Keep at most 50 drafts for 30 days. Explain “Saved on this device for up to 30 days” beside draft settings; offer Disable draft saving and Remove saved drafts. Do not persist gateway tickets, tokens, fetched file contents or attachment bytes. Handle storage failure visibly and retain the current in-memory draft; show “Draft saved locally” only after a successful write. Plain focus/reopen does not reload. Explicit session targeting updates selection through an event/request without recreating the window. This plaintext on-device retention proposal is a user review choice, not a previously approved preference.

Submission states: idle → submitting → acknowledged/running or failed/unknown. Capture draft version, context and stable caller-generated submission ID once; pass that exact ID through task_submit to prompt.submit and retain the non-secret pending receipt metadata. Guard both Enter and button paths. Enter during IME composition never submits; Shift+Enter inserts newline. A reference-only prompt may submit when at least one valid reference exists. A receipt clears only the captured unchanged draft; newly typed content remains. On rejection preserve text/references and show actionable error. After ambiguous timeout show “Submission outcome unknown”; never automatically resubmit. An acknowledgement for the matching ID, or a gateway-documented exact submission-ID lookup, can resolve it. Resumed messages, matching text, or running=true alone are not proof; if no lookup contract exists, retain unknown with Check status and an explicit Send again action warning it may duplicate the original. Do not assume gateway idempotency. Use existing attach-before-submit socket sequence.

Stop is visible only for a running turn; activation immediately shows **Stopping…**, disables duplicate interruption and waits for authoritative state. Idle is **Idle**; completion is **Completed** only with completion evidence. Cancelled, failed and unknown remain distinct.

Model picker states “Main agent” scope and reports success/failure; it must not imply a session-only setting. Use actual profile identity or an honest generic label, not hardcoded names.

## References and full attachments

Initial release supports **Add file reference** and an actual `@` repository-file search. Chips display “reference,” canonical path and removal. Results are restricted to the current explicit repository and protected-file rules. A path reference gives the agent a path; it is not an upload and may be inaccessible to a remote context.

Binary image upload and full attachment delivery remain **deferred** until the gateway's supported payload and remote/local semantics are captured and tested. Hide the misleading image-attachment button in this scope. No fabricated attachment schema is introduced.

## Transcript and run evidence

Render safe Markdown with code blocks, tables and usable links. Raw HTML is not accepted; generated text and filenames never go through unsafe HTML interpolation. Recognize only allowlisted `https:`/`http:` link destinations, and open them through a supported narrowly scoped native handler. File links use explicit repository-aware navigation rather than arbitrary native URL execution. Unsupported protocols display as text.

Every answer and historical message remains retrievable. Page older messages in groups of 100, keep a maximum initial rendered window of 250, and use explicit Load earlier / Show full message / Show full output where needed. UI caps include visible counts and access to the remainder. If the gateway itself cannot return older content, say so rather than pretending a button can retrieve it. Preserve expanded activity blocks and scroll anchors across polls.

Run evidence is derived from matched tool-call IDs and explicit result records. Missing result = **Unknown**, never “ok.” Subagent records match their own call/result IDs, not an arbitrary later tool result. Display a current-run boundary only if authoritative metadata identifies it; otherwise label evidence **Session activity** and avoid invented per-turn duration/counts.

Tests are evidence from a specific tool result with command, result timestamp, exit status and provenance. Parse supported Rust/pytest/Jest summary forms only within that tool result, never user prose, and never combine historical maxima. Failed-only output must show failures; unrecognized/truncated output says **Result unknown** and offers raw output. Passing counts without a captured successful exit do not imply the run succeeded. No test run = **No test output**.

Labels: **Command output** for transcript-derived command history. A genuine PTY UI is a separate deferred feature. CPU/RSS is labelled **Gateway resources**, session token usage **Session totals**, not cost or per-run metrics. Context tree expands on demand with explicit loading/error states; all repository files remain navigable through search, even if tree pagination caps visible rows.

## Repository review and actions

Display the selected repository/profile/branch and change scope. Include tracked, staged, unstaged, nested untracked, deleted, renamed, binary and conflicted files. All changed files remain selectable. A refresh reloads the selected diff or clears an invalid selection; read failures show **Could not load changes**, never clean.

Separate **Working tree** from **Last turn**. Last turn compares the captured base reference to current working tree plus untracked files; if no valid turn base exists, show **Last-turn baseline unavailable**. Do not substitute HEAD or infer a baseline from a message.

Stage/Unstage act on selected scoped paths, have busy/error feedback, then refresh status/list/diff. **Discard unstaged changes…** states that the index is retained. The review dialog shows repo/path, scope and diff; Keep changes cancels. For an untracked file, use **Delete untracked file…**, preview the exact path and consequence. No recursive directory deletion: enumerate individual files for review. A cancelled dialog makes zero IPC mutation calls. Native preflight returns an opaque review token for canonical repository/path, HEAD, index and relevant file content. Revalidate immediately before mutation under per-repository serialization; changed scope/content rejects the token and requires a refreshed preview. External processes do not obey app locks, so do not claim the token eliminates all filesystem races. Staged-discard support is excluded from the first scope; it requires a separate explicit action and contract.

Existing PR → **Open pull request**, a real navigational action. New PR → **Review pull request…**, disabled during an active turn or when no committed branch changes/remote/upstream are available, with the reason visible. Dialog previews title, body, base, head and committed diff; only **Create pull request** publishes. A preflight reports unmet push/upstream prerequisites without automatically committing or pushing. Its review token captures repository, remote, base/head commit IDs and committed diff; revalidate before gh creation, reject changed data and refresh the preview. A repeat activation uses busy protection; ambiguous timeout reconciles existing PR state before permitting another creation. Unresolved remote status remains unknown rather than issuing a second create automatically.

Worktree creation accepts shipped `{base, branch, name}` and `{existingBranch}` contracts, uses a sanitized destination, and rejects unsupported/conflicting combinations explicitly. Automatic stage-all commit and upstream-changing push behavior are not introduced by the UI redesign; separate compatibility follow-up records those gaps.

## Native prerequisites

Parse forwarded launch options from the forwarded request, then show/unminimize/focus existing windows. Correct default launcher resolution for resolved Hermes home. Normalize camel/snake wire aliases for timeout/connection. Support active local profile routing explicitly; unsupported connection IDs fail clearly before network access. Do not claim cloud/multi-connection management.

Install Classic WebSocket listeners before activation/pumping; buffer any early ID-tagged frames until the socket is assigned. Closing during connection permanently prevents later OPEN and closes a late native handle exactly once; dispose listeners on all exits.

Read policy checks requested and resolved paths, rejects sensitive credential/key paths and nonregular files, applies bounded reads before allocation and enforces the same rules in preview, data URL and plugin-source entry points. Use synthetic temp credential fixtures, never actual user secrets. Directory dialogs honor `directories` without changing file-selection behavior.

Read retries use the existing retry predicate with a maximum of two retries and bounded backoff; only safe reads are eligible. Mutations are never automatically retried. A dedicated external-link opener permits http/https only, without widening general shell execution capability.

## Acceptance and release evidence

All findings map to a phase or explicit deferred disposition in the ledgers. Require:
1. Complete new-session → submit → acknowledged/running → stop/completion/error cases using an isolated gateway test profile; preserve drafts and prevent duplicate sends.
2. Delay/reorder A/B session/file responses; same-length message edits; stale/disconnected/reconnect; unknown outcome reconciliation.
3. Rust/pytest/Jest success, failure-only, old output, user-prose, missing result and large-output fixtures.
4. Throwaway repo fixtures covering staged/unstaged/nested untracked/deleted/renamed/binary/AA/DD conflicts, >12 files, last-turn working edits, refresh races and path markup.
5. Native screenshots/geometry and keyboard checks at 1680, 1600, 1599, 1440, 1280, 1279, 1100, 1024, 1000, 999 and 900px; height 600 and 900; all mode/panel combinations that exist at each size.
6. Focus entry/trap/restore, Escape, tabs, compact search, reduced motion, native reopen/forwarded target and honest unsupported-feature states; Workspace at 640×420 in addition to Task’s 900×600 floor.
7. Fresh Rust/Node tests, Tauri/WebKit acceptance and dated evidence; sandbox-blocked checks reported separately, no blanket “all tests pass.”

## Deferred register and review choices

| Item | Disposition |
| --- | --- |
| Full binary/image upload, remote attachment semantics | Separate gateway contract design; initial UI calls paths references |
| Agents/capabilities/plugin/job management | Keep truthful read-only evidence/inspection; full management deferred |
| 159 refused request channels / 45 feature events | Per-channel inventory retained; not silently in initial scope |
| Multi-connection/cloud registry and profile management | Explicit unsupported-state gating; active local-profile routing corrected |
| Commit auto-staging, first-push/upstream semantics | Separate compatibility work; no misleading support claim or auto-push added |
| Media Range/streaming/remote media, terminal lifecycle/env, preference serialization/events | Separate native compatibility follow-up; not represented as verified by this redesign |
| Full Classic screen-by-screen validation | Separate compatibility acceptance project; inventory-level review only |
| Dock resizing, new themes, mobile below 900px | Deferred; explicit modes and current native minimum suffice |

User review should confirm: dark indigo direction; primary Task/Classic roles; responsive mode structure; path references first; on-device draft retention/opt-out; reviewed PR/discard behavior; and the scope/deferred split. An implementation execution approach can be selected after that review. No implementation begins until the user approves.

