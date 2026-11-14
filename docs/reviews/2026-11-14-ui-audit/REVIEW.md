# Hermes UI and functional review

> Historical 14 November scope. The [current full-feature packet](README.md) supersedes feature exclusions, read-only-only management and Classic fallback as a final destination. Original defect evidence remains valid.


**Date:** 2026-11-14 (Asia/Kolkata). **Baseline:** `0c272d7`. **Status:** review complete; design and implementation remain **proposed, awaiting user review**.

The app has useful session, execution and repository surfaces, but it is not functionally complete. The immediate priority is trustworthy state and actions: users must see failures, know which session/repository they are acting on, retain their drafts, see current changes, and distinguish an unknown result from a successful result. Visual polish should follow that foundation.

No product source was changed during this review. No live prompt was sent, real work reverted, commit pushed, PR created, dependency installed or app rebuilt/relaunched. Existing unit tests did execute shell commands and filesystem/Git operations in temporary fixtures.

## Review packet

- [Refined images, first-pass critique, rerender and prompts](VISUAL_PROPOSALS.md)
- [Complete UI control and feature ledger](FEATURE_LEDGER.md)
- [All 281 native channels and coverage](CHANNEL_COVERAGE.md) / [machine-readable inventory](channel-inventory.tsv)
- [Proposed design and behavior specification](../../superpowers/specs/2026-11-14-hermes-ui-reliability-design.md)
- [Phased implementation plan](../../superpowers/plans/2026-11-14-hermes-ui-reliability-plan.md)

## Scope and confidence

| Surface | Reviewed | Confidence limit |
| --- | --- | --- |
| Default Task | All custom HTML/CSS/JS, Rust task RPC/submit, native running window | Existing session inspected; no fresh send/interrupt/model mutation exercised |
| Workspace review | All custom HTML/CSS/JS and Git handlers | Static review and temporary-repo unit/harness evidence; not every native interaction exercised |
| Classic Hermes | Bridge, canonical channel inventory, selected shipped Electron contracts and their Rust replacements | Broad capability inventory; not exhaustive interaction testing of every compiled Classic screen |
| Native shell | Startup/window/tray/CLI routing, discovery, API, files, PTY, media, preferences | Tests plus source; Finder/notifications/seek/reopen scenarios remain acceptance work |
| Documentation | README, PORT_REPORT, existing design and plan | Historical results were not treated as current proof |

Evidence terms below: **live** = observed in the currently running native app; **code** = traced source path; **harness** = existing function executed in isolation; **risk** = plausible ordering/exploit impact not reproduced end to end. P1 merits early correction; P2 should be resolved or explicitly deferred before claiming the affected flow supported.

## Principal findings

| ID | Priority / evidence | Finding and user impact | Source at baseline |
| --- | --- | --- | --- |
| F01 | P1 code | Task feedback is invisible: `setStatus` writes a nonexistent `#status`; send, model, interrupt and PR failures disappear. | [task.js:1321](../../../ui/task.js#L1321), [task.css:327](../../../ui/task.css#L327), [task.html](../../../ui/task.html) |
| F02 | P1 code | Session polling rerenders only when message count/running changes and never refreshes repository files/tree/PR/sidebar. Same-length streaming/plan edits and changed code can stay stale. | [task.js:1331](../../../ui/task.js#L1331) |
| F03 | P1 code; race risk | Session, file and diff reads have no complete stale-response guards. A late A response can overwrite current B content and context. | [task.js:277](../../../ui/task.js#L277), [task.js:998](../../../ui/task.js#L998), [codex.js:116](../../../ui/codex.js#L116) |
| F04 | P1 code | Enter can submit more than once while pending; IME composition can submit; an eventual receipt can clear text typed afterward. Attachment-only prompts are rejected. | [task.js:1743](../../../ui/task.js#L1743), [task.js:1753](../../../ui/task.js#L1753) |
| F05 | P1 code + live wide-toggle observation | Files toggle cannot reliably reopen at ≤1000px; Code/View diff do not ensure visibility. Wide agent toggle changes a class that only affects narrow layouts. Live wide toggle left inspector visible. | [task.js:1655](../../../ui/task.js#L1655), [task.js:1703](../../../ui/task.js#L1703), [task.css:346](../../../ui/task.css#L346) |
| F06 | P1 code + parser harness | Failed-only and Rust test summaries are missed; counts/durations mix historical runs; missing tool results can display “ok”; idle is treated as completed. Unknown and failed states are not trustworthy. | [task.js:1142](../../../ui/task.js#L1142), [task.js:471](../../../ui/task.js#L471), [task.js:580](../../../ui/task.js#L580) |
| F07 | P1 code | Workspace Revert immediately discards tracked edits or deletes an untracked file. No scope review precedes it; staged work is inconsistently retained. | [codex.js:159](../../../ui/codex.js#L159), [git_ops.rs:519](../../../src-tauri/src/git_ops.rs#L519) |
| F08 | P1 code | API/profile/connection arguments do not reliably route requests. Nondefault requests can reach default-profile data. Unsupported routing must fail visibly rather than silently cross contexts. | [commands.rs:104](../../../src-tauri/src/commands.rs#L104), [commands.rs:428](../../../src-tauri/src/commands.rs#L428), [commands.rs:478](../../../src-tauri/src/commands.rs#L478) |
| F09 | P1 code | Reopen focuses then navigates an existing Task without showing/unminimizing it. Navigation drops draft/UI state; forwarded session/repo flags are read from original startup argv. | [main.rs:180](../../../src-tauri/src/main.rs#L180), [main.rs:198](../../../src-tauri/src/main.rs#L198), [main.rs:307](../../../src-tauri/src/main.rs#L307) |
| F10 | P1 code + harness | Classic WebSocket fires open before registering listeners, permitting early-frame loss; close during connection can later reopen and leak the native socket. | [bridge.js:256](../../../src-tauri/src/bridge.js#L256), [bridge.js:315](../../../src-tauri/src/bridge.js#L315), [gateway_ws.rs:88](../../../src-tauri/src/gateway_ws.rs#L88) |
| F11 | P1 code | File preview/attachment APIs omit shipped checks for secret paths, sensitive symlink targets and regular files. Nonregular reads can also hang. No actual secrets were read or exploit attempted. | [fs_ops.rs:84](../../../src-tauri/src/fs_ops.rs#L84), [fs_ops.rs:124](../../../src-tauri/src/fs_ops.rs#L124), [electron-main.mjs:18534](../../../ui/electron-main.mjs#L18534) |
| F12 | P1 code + function harness | Default launcher duplicates `.hermes` under an already resolved Hermes home, threatening Finder launches without shell PATH. | [discovery.rs:14](../../../crates/hermes-contract/src/discovery.rs#L14), [main.rs:23](../../../src-tauri/src/main.rs#L23) |
| F13 | P1 code | Worktree add reads `baseRef`/required `branch`, but the shipped caller sends `base` or `existingBranch`. Normal new/existing-branch flows fail. | [git_ops.rs:349](../../../src-tauri/src/git_ops.rs#L349), [electron-main.mjs:26993](../../../ui/electron-main.mjs#L26993) |
| F14 | P2 code + live | Duplicate session rows occur when recent/messaging lists overlap. Plain-text transcript displays Markdown markup instead of usable links, tables and code. Session buttons show inconsistent native gray fills. | [task.js:168](../../../ui/task.js#L168), [task.js:535](../../../ui/task.js#L535), [task.css:141](../../../ui/task.css#L141) |
| F15 | P2 code | Silent caps hide full answers/history, code/diffs and files: 4,000 message chars, last 250 messages, first 12 selectable files, 400 code lines and 800 diff lines. No complete-content route. | [task.js:413](../../../ui/task.js#L413), [task.js:535](../../../ui/task.js#L535), [task.js:976](../../../ui/task.js#L976), [task.js:1095](../../../ui/task.js#L1095) |
| F16 | P2 code | Tasks opens Workspace, Explore lists used subagents, Inbox duplicates recents, Capabilities shows used tools. At ≤1279px anonymous session dots and hidden search destroy discoverability; Cmd-K beside New chat actually focuses search. | [task.js:1501](../../../ui/task.js#L1501), [task.js:1636](../../../ui/task.js#L1636), [task.css:476](../../../ui/task.css#L476), [task.js:1688](../../../ui/task.js#L1688) |
| F17 | P2 code | Dialogs lack a focus lifecycle/name relationship; file/subagent rows and title expansion are mouse-only; collapse/tab state lacks appropriate semantics; motion has no reduced-motion override. | [task.js:1465](../../../ui/task.js#L1465), [task.html:183](../../../ui/task.html#L183), [codex.js:104](../../../ui/codex.js#L104), [task.css:457](../../../ui/task.css#L457) |
| F18 | P2 code | File/image “attachments” append local paths only; @ inserts a character. Model picker changes main scope without making scope clear; profile identity is hardcoded. | [task.js:1675](../../../ui/task.js#L1675), [task.js:1753](../../../ui/task.js#L1753), [task.js:1441](../../../ui/task.js#L1441), [task.html:62](../../../ui/task.html#L62) |
| F19 | P2 code + live presence | “Open PR” creates one via `gh pr create --fill`; an existing PR pill cannot be opened. Live clean tree still offered creation. | [task.js:1256](../../../ui/task.js#L1256), [task.js:1732](../../../ui/task.js#L1732), [git_ops.rs:690](../../../src-tauri/src/git_ops.rs#L690) |
| F20 | P2 code | Workspace Refresh keeps old selected diff; failed status/list reads can look like a clean tree. Missing cwd silently falls back to a scanned repo; repo override can stick across sessions. | [codex.js:63](../../../ui/codex.js#L63), [codex.js:90](../../../ui/codex.js#L90), [task.js:874](../../../ui/task.js#L874) |
| F21 | P2 code; active-markup risk | Filenames are interpolated as HTML in Workspace rows and Task diff headers. Markup-like filenames can corrupt UI; executable impact depends on WebView policy. | [codex.js:106](../../../ui/codex.js#L106), [task.js:1087](../../../ui/task.js#L1087) |
| F22 | P2 actual-source Rust harness | Last-turn list/diff compares base…HEAD, omitting uncommitted agent edits. Commit context only reads cached diff. | [git_ops.rs:401](../../../src-tauri/src/git_ops.rs#L401), [git_ops.rs:494](../../../src-tauri/src/git_ops.rs#L494), [git_ops.rs:567](../../../src-tauri/src/git_ops.rs#L567) |
| F23 | P2 code | Folder requests always get file picker; camel-case timeoutMs/connectionId fields are ignored; declared HTTP retry helper has no production caller. | [fs_ops.rs:564](../../../src-tauri/src/fs_ops.rs#L564), [types.rs:68](../../../crates/hermes-contract/src/types.rs#L68), [commands.rs:127](../../../src-tauri/src/commands.rs#L127), [transport.rs:41](../../../crates/hermes-contract/src/transport.rs#L41) |
| F24 | P2 code | New untracked directories collapse into rows with no useful diff; conflict detection misses AA/DD; first push lacks upstream setup; commit no longer stages when expected. Explicit semantics required. | [git_ops.rs:75](../../../src-tauri/src/git_ops.rs#L75), [git_ops.rs:184](../../../src-tauri/src/git_ops.rs#L184), [git_ops.rs:544](../../../src-tauri/src/git_ops.rs#L544) |
| F25 | P2 code | Existing self-tests include unconditional true checks and weak proxy assertions. Passing them does not demonstrate control behavior or a complete task lifecycle. | [task.js:1806](../../../ui/task.js#L1806), [task.js:1811](../../../ui/task.js#L1811), [task.js:1883](../../../ui/task.js#L1883) |

Secondary items retained for explicit accounting: context is a capped root listing with decorative arrows; scheduled jobs and active jobs duplicate cron objects; “Terminal” is transcript-derived output rather than PTY; subagent outcome matching is heuristic; preview limits apply after reading the whole file; terminal EOF registration and inherited-environment handling need acceptance checks; preference writes have no serialization and change subscriptions are inert; media lacks proper ranges/remote hosts; both custom HTML files omit viewport metadata; Workspace lacks responsive rules. These remain in the ledgers and plan/deferred register.

## Verification performed

| Check | Observed result | Meaning |
| --- | --- | --- |
| `cargo test -p hermes-desktop-rust --offline` | **21 passed / 0 failed** | Existing shell tests, including real PTY and temporary Git/fs fixtures |
| `cargo test --workspace --offline` | Contract **10 passed / 1 failed** after successful compilation | The failed socket-ticket test could not bind a Unix listener: sandbox `Operation not permitted` at discovery.rs:199. It is **not** a verified all-pass workspace run. |
| Existing parseTests extracted into Node harness | `2 failed in 0.42s` and Rust failed-only summary both yielded zero failures | Confirms F06 parser defect |
| Actual bridge in Node VM | close-before-open → `["close","open"]`, OPEN state, zero backend close calls; no listeners at onopen | Confirms F10 lifecycle ordering |
| Actual Rust Git functions against temporary fixture | Uncommitted tracked edit visible; Last turn and commit-context diff empty | Confirms F22 |
| Actual discovery function | Default path contained `/.hermes/.hermes/` | Confirms F12 |
| Native Task screenshot/AX inspection | Duplicate sessions, raw Markdown, clean-tree PR action, wide toggle unchanged; session menu opened then closed | Existing app state only; no production mutation exercised |

Harness results were reported by independent audit agents executing actual functions. They are diagnosis evidence, not committed regression tests. No full native breakpoint sweep was performed in this review; proposed sizes are acceptance requirements. Native images were observed inline and are not copied into this packet because they contain real session data. Generated review assets use fictional content.

## Coverage and documentation drift

The registry lists **281 channels: 236 requests plus 45 events**. **77 requests are declared ported (32.6%) and 159 are refused**. “Ported” includes empty/no-op/disabled answers and is not a claim that the complete associated feature works. Feature events have not been established as fully produced/consumed; dedicated WebSocket/PTY transport events do not prove those 45 feature events.

Classic remains a fallback, but unsupported native APIs can still fail there; a fallback is not feature parity. The full inventory assigns every channel a disposition.

README describes Classic as primary although code defaults to Task; the old design describes a preexisting layout that has already been replaced; breakpoint amendments conflict; planned inert controls now have handlers; PORT_REPORT test counts and Task evidence are stale. These historical documents are left unchanged during approval review. The proposed specification supersedes their conflicting requirements **only if approved**.

## Recommended decision

Approve a reliability-first UI scope in six groups: **N** native prerequisites; **U1** state/composer; **U2** navigation/responsiveness/accessibility; **U3** transcript/run evidence; **U4** repository review/actions; **V** acceptance/documentation. Full connection/profile management, multimodal upload, agents/capability management and the 159 refused channels remain separate feature projects.

Review the refined wide image, compact/state board, design decisions and deferred register before implementation. No implementation authorization is inferred from delivery of this packet.

