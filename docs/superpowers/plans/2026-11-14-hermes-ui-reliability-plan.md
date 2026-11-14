# Hermes UI Reliability Implementation Plan

> Historical limited-scope proposal. Superseded for architecture and feature scope by the [full Hermes design](../specs/2026-11-15-full-hermes-design.md) and [full Hermes plan](../plans/2026-11-15-full-hermes-plan.md). Do not execute the exclusions or vanilla-only structure below as the full-product destination.


> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Status:** DRAFT — awaiting user review; do not execute yet.

**Goal:** Deliver the approved Hermes UI direction with reliable state, truthful execution evidence, complete local change review and explicit action semantics.

**Architecture:** Keep Tauri/Rust and the two custom vanilla-JavaScript surfaces. Extract small state, gateway/repository adapter and evidence modules from `task.js`; share repository rendering/action contracts with Workspace without rewriting the compiled Classic bundle. Native fixes supply the correctness/security prerequisites; fixture-driven Node tests and native WebKit acceptance verify behavior.

**Tech Stack:** Existing Rust/Tauri, serde/ureq/tungstenite, vanilla JavaScript/HTML/CSS, Node built-in test runner; no new dependency is preapproved.

**Spec:** [2026-11-14-hermes-ui-reliability-design.md](../specs/2026-11-14-hermes-ui-reliability-design.md). Read the spec and [audit](../../reviews/2026-11-14-ui-audit/REVIEW.md) before executing.

## Global Constraints

- Implementation requires explicit user approval of this packet.
- Retain Rust/Tauri and the custom vanilla JavaScript/HTML/CSS surfaces; do not edit generated Classic assets.
- No dependency installation or toolchain migration is required by this proposal.
- Never retry a mutation automatically after an ambiguous delivery or timeout.
- Never display success, a clean tree, or completed tests when the result is unknown or retrieval failed.
- All mutation targets must include the captured session/profile/repository identity.
- No real user repository or live account is used for destructive acceptance tests.
- All hidden panels remain reachable by labelled controls and keyboard navigation.
- Generated images are visual concepts; this written specification governs behavior.

## Review Focus

Each condition below is exercised by the named owning task; this list records the highest-risk classes for final independent review.

1. Late A responses after selecting B must never overwrite B's content, repository or submission target — Tasks 4/5/8.
2. Ambiguous send/PR timeout and newly typed draft must preserve data without duplicate publication — Tasks 5/9.
3. Missing/failed-only/historical test output must never become success — Task 7.
4. Mixed staged/unstaged/untracked/conflicted paths and hostile filenames must remain safely reviewable — Tasks 3/8/9.
5. Reopen/resize/disconnect must preserve drafts and keep all modes/panels reachable — Tasks 1/4/6/10.

## File and interface map

Existing `ui/task.js` remains the DOM composition/wiring entry point; `ui/codex.js` remains Workspace wiring. Create only the focused units needed by these tasks:

| File | Responsibility |
| --- | --- |
| `ui/task-state.js` | Pure context/request/draft/submission/layout transitions |
| `ui/task-data.js` | Task invoke/API adapter; coordinated reads and pending-send reconciliation |
| `ui/task-evidence.js` | Tool-call matching, test summaries and provenance |
| `ui/task-content.js` | Safe transcript/Markdown node construction and complete-content access |
| `ui/review-data.js` | Shared repository snapshots, selected diff refresh, PR preflight |
| `ui/review-ui.js` | Shared safe file/diff DOM and reviewed mutation dialogs |
| `tests/ui/*.test.mjs` | Built-in Node tests of state/adapters/evidence with controlled clocks/replies |
| `ui/task-fixtures.js` | Allowlisted fictional fixture payloads for explicit self-test mode only |
| `docs/reviews/<implementation-date>-acceptance/` | Actual dated screenshots, commands and limitations |

Public JavaScript structures use JSDoc typedefs and named exports (no TypeScript build added):
- `ContextKey = {localBackendKey:string, profile:string, sessionId:string|null, repo:string|null}`. The backend key is a non-secret stable namespace for the canonical Hermes home, produced with shell_info.
- `RequestKey = ContextKey & {generation:number, resource:string, path?:string}`.
- `Draft = {text:string, references:string[], version:number, updatedAt:number}`.
- `ConnectionState = {phase:'connecting'|'connected'|'stale'|'unavailable', lastSuccessAt:number|null, failures:number}`.
- `Submission = {id:string, context:ContextKey, draftVersion:number, phase:'submitting'|'acknowledged'|'failed'|'unknown'}`.
- `LayoutState = {mode:'chat'|'split'|'changes', sessionsOpen:boolean, runOpen:boolean}`.
- `ToolEvidence = {callId:string, name:string, result:string|null, exitCode:number|null, timestamp:number|null, outcome:'running'|'success'|'failed'|'unknown'}`.
- `RepoSnapshot = {repo:string, branch:string|null, files:Array, fetchedAt:number, error:string|null}`. `files` retains current native review fields and explicit staged/unstaged/conflict classification.
- `FileChunk = {text:string,nextOffset:number,eof:boolean,fileIdentity:string}`; offsets are bytes and UTF-8 text boundaries are preserved.
- `ReviewToken = string`, an opaque native token bound to canonical repository/paths and reviewed HEAD/index/content (or PR remote/base/head/committed diff).

Task boundaries are deliverables, not individual styling passes. Tasks 2/3 may run independently after Task 1 interfaces settle. Tasks 4–9 must respect the listed producers/consumers.

### Task 1 (N): Reliable native launch and scoped API transport

**Files:** Modify `src-tauri/src/main.rs`, `commands.rs`, `crates/hermes-contract/src/{types,discovery,transport}.rs`; Rust unit tests beside existing tests.

**Interfaces:** Produce `TaskLaunchOptions` with optional session/repo/width/height and selftest/audit/allowlisted fixture fields; `parse_task_launch_options(args: &[String]) -> Result<TaskLaunchOptions,String>`; change `open_task_window(app: &tauri::AppHandle, options: TaskLaunchOptions) -> Result<(),String>`. API request serde accepts both timeoutMs/timeout_ms and connectionId/connection_id. Scoped path helper `scoped_api_path(path: &str, profile: Option<&str>) -> Result<String,String>` must encode a local profile once and reject conflicting explicit scope.

- [ ] Add failing Rust cases for default/custom resolved Hermes home launcher, forwarded target options versus original argv, camel/snake aliases, conflicting query/profile scope, unsupported connection ID and read-only retry eligibility. Assert POST is attempted once on timeout.
- [ ] Run `cargo test --workspace --offline`; confirm the new cases fail for behavior, distinguishing existing Unix-socket sandbox restrictions.
- [ ] Implement launch option parsing from each incoming request. Plain reopen performs show/unminimize/focus without navigation; explicit targets use a scoped app event consumed by Task. Normalize toolbar/tray defaults to Task and retain explicit Classic action.
- [ ] Route launch/window-creation failure to a visible native diagnostic with the existing log location. Inject failure in the native acceptance harness; a separate recovery window is not part of this scope.
- [ ] Correct launcher path composition, profile/query encoding, unsupported-connection rejection and timeout use. Expose non-secret localBackendKey through shell_info. Wire existing retry predicate to safe reads only: maximum two retries with bounded backoff; never retry mutation.
- [ ] Run the owning Rust cases and full workspace suite. In Task 10 verify plain reopen/minimized/hidden and forwarded targets on native macOS with an unsent fictional draft.
- [ ] Commit only reviewed files with `fix: preserve task launch state and API scope`.

### Task 2 (N): Classic WebSocket lifecycle

**Files:** Modify `src-tauri/src/bridge.js`, `gateway_ws.rs`, relevant `commands.rs`; create `tests/ui/bridge-socket.test.mjs`.

**Interfaces:** Retain renderer WebSocket facade and `hermes_ws_open/send/close` command shapes. Define internal `IpcSocket._subscribe() -> Promise<void>`; native activation must not pump events before the listener/ID association is safe. An optional explicit activation command is acceptable only if both bridge and Rust tests cover the handshake. A shared bridge diagnostic uses safe DOM to display unsupported-operation failures with channel/availability; it does not modify compiled Classic screens or claim per-control gating.

- [ ] Add failing VM harness tests executing the real bridge: listener setup delayed; first frame arrives immediately; close before native-open resolves; close after subscription starts; native open rejects; late unlisten promise resolves.
- [ ] Run `node --test tests/ui/bridge-socket.test.mjs`; require an observed missing-frame/reopen failure before changing code.
- [ ] Install/await listeners before exposing OPEN, buffer early ID-tagged frames, prevent late OPEN after close, close a late native handle once and dispose listeners on every exit.
- [ ] Show shared unsupported-operation diagnostics on dispatch refusal and missing bridged methods; retain only known existing capability flags. Add harness cases proving unsupported calls are not false success and diagnostic text is escaped. Screen-by-screen Classic gating remains deferred.
- [ ] Rerun the harness; assert one terminal close, zero messages after close, no listener leak and correct early message delivery. Run Rust socket tests under an environment allowing local sockets.
- [ ] Commit `fix: make bridged sockets preserve lifecycle ordering`.

### Task 3 (N): Bounded protected file reads and folder/link contracts

**Files:** Modify `src-tauri/src/fs_ops.rs`, `commands.rs`, `bridge.js` where a safe link method is exposed; Rust tests in existing modules.

**Interfaces:** `resolve_readable_path(path: &Path) -> Result<PathBuf,String>` validates requested/canonical paths and regular-file status; `read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>,String>` reads at most limit plus a truncation probe. Preserve existing preview caps/successful response shapes. Add shell-owned `read_file_chunk(path:String,offset:u64,expected_identity:Option<String>) -> Result<FileChunk,String>` with a fixed maximum 65,536-byte text chunk, valid UTF-8 boundaries, explicit EOF and identity-change rejection; invalid text reports binary rather than lossy fake code. New narrow command `open_external_url(url: String) -> Result<(),String>` accepts http/https only.

- [ ] Add failing synthetic-fixture tests for ordinary text, .env/SSH/AWS/private-key paths, symlink aliases to denied targets, FIFO/nonregular file rejection and oversized input bounded before whole-file allocation. Never open user secrets.
- [ ] Run the relevant Rust tests, then implement the shared policy in preview/text/data-URL/plugin-source reads.
- [ ] Add failing chunk-reader cases spanning the 512KiB preview cap, multibyte UTF-8 at a page boundary, EOF, offset bounds, binary input and file replacement/change between pages. Implement bounded full-text retrieval through paging, preserving preview policy and caps; no whole-file allocation.
- [ ] Honor `directories:true` using the folder picker; preserve single/multiple file semantics. Add option-parsing coverage and native acceptance cases for both picker types (no unattended real selection).
- [ ] Add http/https acceptance and javascript/file/custom-protocol rejection tests for the external opener; wire only the explicit command, without general shell permission expansion.
- [ ] Run `cargo test -p hermes-desktop-rust --offline`; verify bounded errors and policy consistency, then commit `fix: protect native file reads and selection contracts`.

### Task 4 (U1): Context identity, refresh, visible feedback and persisted drafts

**Files:** Create `ui/task-state.js`, `ui/task-data.js`, `tests/ui/task-state.test.mjs`, `tests/ui/task-data.test.mjs`; modify `ui/task.js`, `task.html`, `task.css`.

**Interfaces:** Export `makeRequestKey(context:ContextKey,generation:number,resource:string,path?:string):RequestKey`, `isCurrent(key:RequestKey,current:RequestKey):boolean`, `mergeSessions(rows:Array):Array`, `connectionAfterPoll(previous:ConnectionState,result:{ok:boolean,now:number},running:boolean):ConnectionState`, `loadDraft(storage,context:ContextKey,now:number):Draft`, `saveDraft(storage,context:ContextKey,draft:Draft,now:number):{saved:boolean,error:string|null}`. Task adapter produces `refreshContext(context:ContextKey,generation:number):Promise<void>` and accepts injected invoke/clock for tests.

- [ ] Write failing tests: A→B delayed responses rejected; same-length content/todo edits trigger update; session dedupe uses profile+ID/newest activity; New chat repeated activation creates one session and refreshes/selects its row; repo absence stays null; override scoped per session; two failed active polls/10s mark stale.
- [ ] Add draft round-trip tests for text/references, separate local-home/profile/new-chat keys, 50-record/30-day bounds and expiry, disable-saving/remove-all, unavailable/quota storage and no secret/token/file-byte persistence. Run both Node files to verify failures.
- [ ] Extract state/adapter boundaries; add a real visible live/status region and inline operation errors with Retry only where safe. Show last success time; never map read failure to clean/completed.
- [ ] Coordinate nonoverlapping refreshes at active 4s/idle15s plus explicit post-mutation/manual refresh; compare snapshot content/revision, not only length/running. Dispose obsolete timers and handle Task target events from Task 1.
- [ ] Guard New chat creation, refresh the deduplicated sidebar after receipt, select the created identity and focus its composer; retain errors and previous content if creation fails.
- [ ] Persist drafts after editing and restore per identity; disclose on-device plaintext 30-day retention, offer Disable draft saving/Remove saved drafts, and show saved status only on successful write. Keep this proposed default visible for user approval.
- [ ] Run both Node suites and a fixture disconnected/reconnect native check; commit `fix: keep task state fresh and preserve drafts`.

### Task 5 (U1): Transactional composer, references and interruption

**Files:** Modify `ui/task-state.js`, `task-data.js`, `task.js`, `task.html`; modify `src-tauri/src/task_ops.rs` / command wrapper to accept stable submission ID if required; create `tests/ui/task-composer.test.mjs`.

**Interfaces:** `beginSubmission(context:ContextKey,draft:Draft,id:string):Submission`; `applyReceipt(submission:Submission,currentDraft:Draft,receipt:{submissionId:string,status:string}):{submission:Submission,draft:Draft}`; `submitPrompt(context:ContextKey,draft:Draft,submissionId:string):Promise<object>`. Pass the caller's ID through task_submit/task_ops::submit to prompt.submit instead of generating a different ID in Rust; wrap the matched acknowledgement with that ID. Retain existing resume-before-submit WebSocket sequence. Use Task 4 draft storage.

- [ ] Write failing tests for repeated Enter/button while pending, IME Enter, Shift+Enter, reference-only prompt, failed send retaining draft, new typing before receipt, session switch during send and unknown timeout without automatic resubmit. A mismatched receipt ID or resumed running/messages alone must not acknowledge the pending submission.
- [ ] Run `node --test tests/ui/task-composer.test.mjs`; implement one in-flight guard and captured context/draft version/ID. Allow new typing without clearing newer versions.
- [ ] Keep ambiguous delivery unknown unless a matching acknowledgement or a gateway-documented exact ID lookup resolves it; document lookup availability from actual contract evidence. If unavailable, Check status retains unknown and an explicit Send again warns of possible duplication. Do not assume idempotency or invent a reconciliation endpoint.
- [ ] Replace misleading attachment controls with Add file reference and reference chips. Add repository search for @ through existing safe readDir/path policy; label scope and unavailable remote path semantics. Do not invent binary payloads.
- [ ] Implement immediate Stopping… with duplicate-interrupt guard and eventual authoritative result; make model main-agent scope explicit with error feedback.
- [ ] Rerun tests; in Task 10 use isolated gateway fixture/profile to observe a full text submit, reconciliation and interrupt. Commit `fix: protect prompt submission and clarify references`.

### Task 6 (U2): Coherent responsive modes, truthful navigation and keyboard behavior

**Files:** Modify `ui/task.html`, `task.css`, `task.js`, `codex.html`, `codex.css`, `codex.js`; add layout transitions to `task-state.js`; create `tests/ui/task-layout.test.mjs`.

**Interfaces:** `resolveLayout(width:number,state:LayoutState):{mode:string,railWidth:number,inspector:'docked'|'drawer'|'closed',splitAvailable:boolean}` enforces spec minima. `setLayoutAction(state:LayoutState,action:object):LayoutState` is the only owner of open/mode transitions. Existing view-diff/navigation actions request the correct visible mode.

- [ ] Write failing boundary tests for 1600/1599/1280/1279/1000/999/900, available-width split eligibility and repeated open→close→open of Chats/Run/Changes. Test explicit View diff on a hidden panel.
- [ ] Run `node --test tests/ui/task-layout.test.mjs`; implement the spec table, remove contradictory hidden/open CSS states, apply compact session drawer rather than anonymous dots.
- [ ] Apply approved tokens/type, reset native button fills and use labelled controls. Replace duplicate/misleading destinations with unique sessions, Scheduled jobs, Agents used, Tools used, Classic Hermes; real Chat/Split/Changes modes.
- [ ] Correct shortcuts; add accessible tab/expanded states, focus entry/trap/return, Escape, keyboard file/subagent activation, reduced motion and viewport metadata. Task retains 900×600 minimum; Workspace retains 640×420 and uses list-or-diff navigation below 900px.
- [ ] Run layout tests; Task 10 captures actual geometry/keyboard evidence across all required sizes. Commit `feat: add coherent workspace modes and accessible navigation`.

### Task 7 (U3): Complete safe content and trustworthy execution evidence

**Files:** Create `ui/task-content.js`, `ui/task-evidence.js`, `tests/ui/task-evidence.test.mjs`, `tests/ui/task-content.test.mjs`; modify `ui/task.js`, `task.css`.

**Interfaces:** `collectToolEvidence(messages:Array):ToolEvidence[]`; `parseTestEvidence(tool:ToolEvidence):{framework:string|null,passed:number|null,failed:number|null,durationMs:number|null,outcome:string,sourceCallId:string}`; `deriveRunPresentation(snapshot:object,evidence:ToolEvidence[]):object`. `renderMessageContent(document,text:string):DocumentFragment` builds safe DOM; `messageWindow(total:number,loaded:number):{start:number,end:number,hasEarlier:boolean}` follows spec initial250/page100.

- [ ] Write failing fixtures for Rust/pytest/Jest passed and failed-only, user prose mentioning test success, two historical runs, missing exit/result, truncation, explicit cancellation and idle with no completion evidence. Verify matched subagent/tool IDs.
- [ ] Run `node --test tests/ui/task-evidence.test.mjs`; implement bounded per-result parsing/provenance with unknown instead of invented success or mixed maxima. Label unavailable per-run boundaries Session activity.
- [ ] Add content tests for raw HTML/markup filenames, unsafe link protocols, code fence/table/list/link rendering and long message/history access. Use deterministic escaping/node construction; if existing safe Markdown utilities cannot be reused without importing generated internals, implement the documented minimal Markdown subset with raw HTML disabled. Any dependency alternative requires a separate reviewed decision.
- [ ] Remove silent truncation; support explicit full content/history windows. Persist expansion keys and preserve scroll anchor unless reader is following the end. Expand context on demand; keep repository search available beyond display caps.
- [ ] Rename Terminal to Command output, label Gateway resources and Session totals, remove duplicate plan/status and fake capabilities. Run evidence/content tests plus native markup/long-content fixture checks.
- [ ] Add read-only Scheduled jobs details with identity/profile/schedule/next/last run and explicit unknown fields; expose every job through pagination rather than a hidden four-row cap. Fixture-test selection and verify zero scheduling mutation calls.
- [ ] Commit `feat: show complete conversation content and sourced run outcomes`.

### Task 8 (U4): Complete repository data, worktree contracts and stable review

**Files:** Modify `src-tauri/src/git_ops.rs`; create `ui/review-data.js`, `tests/ui/review-data.test.mjs`; modify `ui/task.js`, `codex.js`; Rust Git fixture tests beside existing tests.

**Interfaces:** `loadRepositorySnapshot(repo:string,request:RequestKey):Promise<RepoSnapshot>`; `loadSelectedDiff(repo:string,path:string,scope:string,request:RequestKey):Promise<object>` consume Task 4 identity guards and native channel names. Rust `review_list/review_diff` preserve successful legacy payloads while propagating failures as explicit Err/rejected IPC, never empty success; model binary/empty/unreadable separately. `loadFilePage(path:string,offset:number,identity:string|null):Promise<FileChunk>` consumes Task 3's protected chunk reader. Worktree options accept `base`, `branch`, `name`, `existingBranch`.

- [ ] Add failing temporary-repo cases: uncommitted tracked and nested untracked Last turn against base; invalid/missing turn base; staged+unstaged; deletion/rename/binary/AA/DD; >12 files; valid new-branch and existing-branch worktree requests.
- [ ] Run owning Rust Git tests. Fix worktree option normalization, safe destination handling, working-tree baseline comparison and complete untracked-file enumeration. Keep staged/unstaged scopes explicit.
- [ ] Add failing Node cases for stale file response, external edits, deleted selected file, failed status/list/diff read versus legitimately empty/binary diff, complete-file selection and repo-switch/refresh ordering. Include large-file paging, multibyte boundaries and identity-change reset. Run `node --test tests/ui/review-data.test.mjs`.
- [ ] Share adapter behavior between Task and Workspace; Refresh reloads selected diff, all files are accessible, failure cannot render clean, filenames use safe DOM. Use protected text paging rather than claiming truncated preview is the whole file; on changed identity ask to reload from the start. Show baseline-unavailable and binary/deletion states explicitly.
- [ ] Run Rust + Node owning tests and fixture UI review, then commit `fix: make repository review complete and current`.

### Task 9 (U4): Reviewed discard, stage and pull-request actions

**Files:** Create `ui/review-ui.js`, `tests/ui/review-actions.test.mjs`; modify `ui/{task,codex}.js`, both custom HTML/CSS files, `src-tauri/src/{git_ops,commands}.rs`.

**Interfaces:** `reviewDiscard(document,target:{repo:string,path:string,scope:'unstaged'|'untracked'},diff:object):Promise<boolean>`; native discard preflight returns ReviewToken and mutation requires it. `getPrPreflight(repo:string):Promise<{eligible:boolean,reason:string|null,base:string,head:string,committedFiles:Array,reviewToken:ReviewToken}>`; `reviewPullRequest(document,preflight:object):Promise<{title:string,body:string,base:string,head:string,reviewToken:ReviewToken}|null>`. Existing PR creation channel accepts reviewed fields; native handler passes arguments directly (no shell interpolation). `createPr` never pushes/commits automatically. Native code revalidates tokens immediately before mutations under per-repo serialization.

- [ ] Add failing adapter/fixture tests: cancelling discard calls zero mutations; tracked unstaged discard retains index; untracked deletion names one file; staged scope rejected; Stage/Unstage refresh; markup filename safe; busy guard; file/index/HEAD changes while dialog is open invalidate the token without discard.
- [ ] Run `node --test tests/ui/review-actions.test.mjs` and owning Rust temporary-repo tests. Replace immediate Revert with the scoped review dialog and accurate backend operation.
- [ ] Add PR tests: existing PR opens; zero committed head/base delta, active run, missing remote or missing upstream produces a visible disabled reason; a clean working tree with committed branch delta remains eligible. Reviewed title/body/base/head/token reach native handler; changed head/base/remote/diff invalidates review; Cancel creates nothing; repeated click creates once; unknown remote outcome does not automatically retry.
- [ ] Implement review/preflight and narrow link opener from Task 3; replace `gh pr create --fill` with explicit reviewed arguments. Never silently commit or push to satisfy prerequisites.
- [ ] Run Node/Rust tests with a stubbed GitHub command boundary. Any live PR publication is a separate user-authorized action, not required for acceptance. Commit `feat: make repository mutations explicit and reviewable`.

### Task 10 (V): Fixture acceptance, native verification and consistent docs

**Files:** Create `ui/task-fixtures.js`, appropriate `tests/ui/fixtures/` data and dated acceptance directory; modify `ui/task.js` selftest/audit and fixture adapter wiring, `README.md`, `PORT_REPORT.md`, prior design/plan status banners.

**Interfaces:** `loadTaskFixture(name:string):object` accepts only checked-in fictional fixture names under explicit selftest mode and bypasses the live gateway entirely. Selftest reports named assertions with expected/actual values and nonzero failure count; geometry checks inspect actual visible elements.

- [ ] Replace unconditional/proxy assertions with user-visible outcomes. Include no-session, clean tree, running, failed-only tests, missing evidence, disconnected preserved draft, rapid context switches, >12 files, long answer/diff and safe-mutation cancellation fixtures.
- [ ] Run `node --test tests/ui/*.test.mjs`, `cargo test --workspace --offline` and formatting checks `cargo fmt --all -- --check`. Report existing/environmental failures separately rather than silently changing unrelated source.
- [ ] Run native Task Tauri/WebKit fixture audit at 1680/1600/1599/1440/1280/1279/1100/1024/1000/999/900 widths and 600/900 heights. Verify Workspace separately at 640×420 and 900×600. Inspect every available layout mode/drawer combination. Capture screenshots and actual clipping/minimum-pane geometry, with explicit pass criteria.
- [ ] Verify keyboard search/shortcuts/dialog focus/links, reduced motion, native directory/file dialogs, tray/hidden/minimized reopen and forwarded session/repo targeting with a fictional saved draft. Test unknown profile/connection feedback and retained OS Quit/Cmd-Q behavior without claiming new resource-cleanup guarantees.
- [ ] If an isolated gateway test profile is available, observe new chat, text/reference submission, duplicate guard, interrupted turn, model-scope error and reconnect reconciliation. If unavailable, mark those checks unverified and do not claim complete release readiness.
- [ ] Reconcile README/PORT_REPORT with actual primary surface, channel dispositions, test counts and new limitations. Mark old design/plan superseded by approved spec; preserve historical evidence as dated history. Map all F01–F25 findings and ledger rows to implemented/tested/deferred outcomes.
- [ ] Perform whole-change review, fix material findings, rerun affected checks and record final evidence. Commit `test: verify Hermes workspace behavior and document support`.

## Coverage and deferrals

| Findings / scope | Owner |
| --- | --- |
| F01–F04, F20 context/state portions | Tasks 4/5/8 |
| F05, F14 session rows/button styles, F16/F17 | Tasks 4/6 |
| F06/F15/F14 Markdown; subagents/context/resources | Task 7 |
| F07/F19/F21; Stage/Unstage | Tasks 8/9 |
| F08/F09/F12/F23 timeout/retry | Task 1 |
| F10 | Task 2 |
| F11/F23 directory selection | Task 3 |
| F13/F22/F24 review/conflict/untracked portions | Task 8 |
| F18 | Task 5; full multimodal/management explicitly deferred |
| F25 and documentation drift | Task 10 |
| F24 commit auto-staging / first-push/upstream; media/terminal/preferences; full Classic parity | Spec deferred register and complete channel ledger; separate follow-up, not implied fixed by Task 10 |

This is a reliability/UI project with native prerequisites, not a 159-channel port-completion promise. Before implementation, user review may narrow or split those prerequisites; do not remove them silently while retaining the affected support claims.

## Self-review and execution handoff

Spec coverage, step granularity, type consistency, review-focus tests and plan proportion have been checked. The plan names testable interfaces without providing product implementation bodies. Generated illustrations are supporting direction; actual geometry/state acceptance is in Tasks 6/10.

Recommended after approval: subagent-driven execution with independent review at each deliverable, because native/renderer contracts and mutation semantics cross boundaries. Native execution in this chat is also possible. The user has not chosen an execution approach. **Deliver this plan for review and wait; do not start Task 1 or install dependencies.**

