# Full Hermes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans only after user approval. Steps use checkboxes for tracking.

**Status:** Implementation authorized by the user on 2026-11-15; in progress on codex/full-hermes. Generated UI concepts remain binding visual references.
**Goal:** Preserve full Hermes functionality in a simpler, measurable Rust/Tauri desktop experience.
**Architecture:** Proposed Rust native services + source-built existing Hermes React renderer + canonical Python Hermes gateway. Framework migration is a separate user decision.
**Tech Stack:** Existing Tauri/Rust and upstream renderer React/TypeScript/Vite/pnpm workspace; pinned source/lockfile and generated gateway contracts. This supersedes v1's no-build/vanilla-only constraint.
**Spec:** [Full Hermes design](../specs/2026-11-15-full-hermes-design.md).

## Global constraints

- No product code, scaffold, dependency install or migration until explicit user approval.
- Every existing feature family is in scope; phases determine order, not removal.
- Preserve gateway ownership, profiles, credentials, prompt-cache/activation semantics and extension identity.
- Never infer success/test counts from user prose, idle state or a background-launch receipt.
- Never retry an ambiguously delivered mutation automatically.
- Keep advanced settings, unknown keys and plugin/provider schema fields.
- Use isolated fixtures/temporary Hermes homes and explicit test recipients, not real user data.
- Keep measured and proposed performance claims separate.

## Review focus

1. Delayed scope-A responses or auth callbacks must not alter scope B — phases 2/4/7/8.
2. Foreground tool completion versus still-running background tests/subagents — phases 3/9.
3. Schema omissions, unknown values and masked/clear credential semantics — phases 4/5/8.
4. Binary audio, microphone lifecycle and hidden-pane cleanup — phase 6.
5. Source/build mismatch and optional native/plugin features silently lost during migration — phases 1/10/11.

## Phase 1: Freeze parity baseline and reproducible renderer source

**Inputs:** source/build hashes and complete audits in the review packet.
**Files:** proposed renderer source provenance manifest and workspace under a repository-owned pinned upstream checkout; root build scripts; tests for source/import reproducibility. Exact imported workspace closure must be resolved before moving files, using upstream package.json/pnpm lock/tsconfig/Vite imports.

- [x] Reconcile source HEAD 3b421578… against shipped build a8b486…; the pristine pinned import reproduces the exact upstream desktop source hash. Local redesign builds record their own hash.
- [ ] Map each built-in route, native channel, settings row/key, dynamic family and plugin contribution to the target destination, owner, API/event, acceptance ID and implementation status.
- [ ] Establish a reproducible source import/build using the existing upstream workspace and frozen lockfile, without editing the user's installed ~/.hermes tree. Preserve required shared packages, schemas, assets and plugin registries.
- [ ] Prove rebuilt renderer routes/features load on a fixture host before substituting it for shipped assets. Preserve attribution/provenance and confirm update workflow.
- [ ] Resolve architecture choice with user review if Rust/WASM/native GUI is preferred; do not silently execute the React plan after a different choice.

**Deliverable:** versioned baseline and buildable renderer source; not a feature parity claim.

## Phase 2: Rust native compatibility and scope authority

**Files:** src-tauri/src/{main,commands,bridge.js,gateway_ws,fs_ops,git_ops,terminal,system_ops}; crates/hermes-contract; source renderer host adapter/types. Keep successful existing contracts unless versioned explicitly.

- [ ] Add failing contract cases for scope/connection/profile routing, camel-case aliases, forwarded launch, show/minimize/draft retention, safe file reads/ranges, Git review semantics and folder dialogs; use v1 audit cases.
- [ ] Implement explicit owner routing; unsupported versions fail visibly, never fall through to local/default.
- [ ] Transport WebSocket text AND binary without String(data)/lossy UTF-8; cover ArrayBuffer/typed-array boundaries, early frames, close-during-connect and cleanup.
- [ ] Port missing native capabilities by functional family: auth callbacks/keychain, grants/plugin sockets, connection/agent registry, media ranges/preview, microphone/hotkeys/windows/notifications/updater lifecycle.
- [ ] Classify every 281 inventoried channel and any additional source methods; a no-op/null answer is not parity evidence.
- [ ] Verify Rust tests and actual native adapters against fake services, including refusal/error paths.

**Deliverable:** typed Rust host adapter and owned scoped transport. Reuse source native contracts, not ad hoc invoke payloads.

## Phase 3: Event-driven runs and test evidence

**Files:** upstream session event reducers, run/subagent/process stores; new feature-owned test-evidence reducer/adapters; Rust gateway transport. Reuse tool.start/tool.complete/tool.progress/todo.updated/subagent and turn events.

**Proposed internal interface:** ExecutionKey includes connection/profile/session/turn/toolId/processId; normalizeTestEvidence(execution,result,report) returns command state, suite state, nullable counts and provenance. Final field names must be derived from generated contracts; result:unknown is validated per tool.

- [ ] Prove failing fixtures for missing results, parent stop with live children, delayed background exit, old reports, user prose, repeated events and reconnect gaps.
- [ ] Replace current snapshot-length/regex inference with subscribed/replay-aware stores; reconcile snapshots without stale overwrites.
- [ ] Add validated adapters for Rust/pytest/Jest/Node or explicit report-file formats; detect wrappers/containers only when evidence identifies them. Unknown runner → raw command/output, counts unavailable.
- [ ] Run real isolated passing/failing/cancelled/background test commands and capture exit/report IDs. Replay those actual events/results through reducers and inspect the native UI.
- [ ] Assert 1 pass + 1 fail cannot render success/zero results; no report cannot render invented counts; late failure updates the correct execution. UI screenshot evidence must accompany assertions.

**Deliverable:** dynamic truthful inspector; no universal arbitrary-test detection claim.

## Phase 4: Unified navigation and state-preserving shell

**Files:** upstream app/routes.ts, sidebar/navigation/layout and related stores; Task/Workspace integration.
- [ ] Map old routes/deep links to Chat, Tasks & agents, Automations, Connections, Skills & tools, Library and Settings.
- [ ] Preserve command palette, contributed routes and skill/quick-command completion.
- [ ] Integrate code/review workflow with complete original chat/attachments/terminal/browser rather than replacing them with simplified approximations.
- [ ] Test rapid navigation/profile switching, drafts, retained live panes, compact drawers, keyboard focus/reduced motion and native window minima.
**Deliverable:** all destinations reachable in one app; no Classic fallback used to conceal missing primary functionality.

## Phase 5: Settings and model/provider configuration

**Files:** upstream settings/{index,constants,helpers,settings-manifest,settings-search,config-settings,subpages} and field editors; api/models.ts; model-picker.tsx; provider/custom-endpoint stores; inventory coverage tests.
- [ ] Generate coverage assertions from all 1,055 inventory rows plus dynamic runtime schemas; every key/control maps to a destination or justified conditional state.
- [ ] Implement nine-category navigation, synonyms/exact-key search, All settings catalogue, legacy redirects and contextual advanced disclosure.
- [ ] Test unknown keys/enums/provider options, empty collections, secret keep/replace/clear, scoped differences, revision conflicts, restart/next-session activation and unsaved edits across scope changes.
- [ ] Retain searchable/ranked/custom model picker and provider auth/keys/custom endpoints/local-model availability; current-chat selection and new-chat defaults remain distinct.
- [ ] Verify invalid auth/unavailable models, stale provider lists, active-turn switch semantics, rollback, two same-named remote profiles, fallback/auxiliary/delegation/MoA routes.
**Deliverable:** simpler common setup with complete searchable configuration; no credentials modified in acceptance outside isolated homes.

## Phase 6: Full voice

**Files:** source voice hooks/stores, voice settings/provider fields and audio APIs; Rust audio/socket/permissions lifecycle.
- [ ] Preserve dictation streaming/fallback, chained/live conversation, STT/TTS providers, custom voices, wake ownership, stop phrases/barge-in, devices and direct/relay policy.
- [ ] Verify byte-for-byte PCM/speech transport, SDP handling where required, capture permissions/denial, stop/cancel, reconnect, background/hidden-pane ownership and late speech.
- [ ] Test on target WebKit/macOS; browser-level tests alone cannot prove codecs/mic/native lifecycle. Provider-dependent tests require a separate approved test setup, never automatic paid calls.
**Deliverable:** microphone→provider→test setup flow and functional voice modes with honest readiness.

## Phase 7: Scheduled jobs, delivery and webhooks

**Files:** source cron page/API/controller, job editor/blueprints, run history, webhook modules and profile scope adapters.
- [ ] Replace cron-session approximations with managed job CRUD and runs.
- [ ] Preserve pause/resume/run-now/stop/delete, timezone and missed-run policies, advanced job fields/overrides, blueprints and multi-target delivery.
- [ ] Link Telegram/service setup without discarding the unsaved job; distinguish saved/active/paused/job execution/delivery success.
- [ ] Test timezone/DST boundaries, offline gateway, duplicate activation, paused next-run, concurrent edits, failed delivery, missing credentials, stopped run and history navigation with test recipients.
**Deliverable:** complete automation management. A paused job has no claimed upcoming execution.

## Phase 8: Messaging, service accounts, skills, plugins and MCP

**Files:** existing messaging/setup/pairing modules; capabilities/{connectors,skills,plugins,toolsets}; api adapters; Rust OAuth/grant/plugin lifecycle.
- [ ] Preserve dynamic catalogs and source-qualified identities, not hard-coded brand grids.
- [ ] Exercise Telegram QR/manual/relay setup, expiry/cancel/access review and other source adapters; account readiness and direct/relay exclusivity remain distinct.
- [ ] Retain hosted multi-account/policy/tool-selection flows and revision-safe updates.
- [ ] Preserve MCP transport/import/unknown keys/env/headers/OAuth/probes/resources/tools/filter semantics; include:[] means zero tools. Callback is owned by desktop machine even for remote backend.
- [ ] Preserve skills search/content/edit/lock/hub/scan/install/update/bulk/profile/slash flows, social-skill prerequisite/read/write boundaries, plugin pins/consent/prerequisites/agent+desktop halves/grants and activation timing.
- [ ] Test install/disconnect/profile switch/OAuth cancellation and plugin disable/unload cleanup against local fake services; no social publication or real account linking in automated fixtures.
**Deliverable:** existing extension/connectivity capabilities remain manageable and understandable.

## Phase 9: Persistent agents, live subagents and Boards

**Files:** source agents/roster/subagent stores, hermes-bots and kanban plugins, delegation routing.
- [ ] Keep persistent profile-backed agents distinct from delegated runs and durable board tasks.
- [ ] Preserve canonical Bot Chat identity, side chats, SOUL/models/tools, profile clone/import/export, groups and gateway-qualified rosters.
- [ ] Display lifecycle-driven child trees, background status, errors/results and owner navigation after parent stop/reconnect.
- [ ] Preserve optional Kanban task/dependency/comment/attachment/run orchestration and diagnostics when enabled.
- [ ] Test same-named agents on multiple gateways, parallel children, late terminal events, compaction/canonical-chat reopen, disabled/late-loaded board plugin and unknown backend routes.
**Deliverable:** live inspectable work and complete agent/task management.

## Phase 10: Remaining native and product families

**Files:** mapped source routes/modules and their Rust adapters identified in phase 1.
- [ ] Retain artifacts/media/browser/preview, genuine terminal, memory/context, imports/exports, checkpoints/review, vault/security, profiles/SSH/cloud/web access, notifications/hotkeys/HUD/pets/quick entry, account/billing and update/install/diagnostics.
- [ ] Specify each native replacement and its exact acceptance before implementation; do not equate a contributed React screen with a functional native integration.
- [ ] Run family-level success/error/cancel/version-skew scenarios. Required unsupported items remain unresolved release blockers, not silently deferred features.
**Deliverable:** no residual parity holes hidden by navigation or fixed capability flags.

## Phase 11: Performance, independent review and release proof

- [ ] Measure original renderer/shell and proposed build under identical fixtures; log Rust/WebView/gateway separately, cold/warm startup, long transcript, streaming events/audio and memory after repeated switches/reconnects.
- [ ] Set numeric acceptance budgets from baseline; bound queues/IPC/output, virtualize long lists, avoid blocking native I/O and clean subscriptions. Optimize identified hot paths in Rust only with a preserving benchmark.
- [ ] Run Rust and upstream UI/schema/contract tests in their supported runners; execute native WebKit tests and actual test-run inspector scenarios, all nine Settings groups, every route and plugin family.
- [ ] Verify all inventory entries and dynamic-family coverage, complete scoped workflows, keyboard/contrast/reduced-motion and compact states; record blocked provider/hardware checks as unverified.
- [ ] Update actual support docs and obtain user review of final evidence. No blanket all-pass, all-Rust, universal test detection or optimization claims.

## Handoff

This is a phased full-product program, larger than the original ten-task coding-workspace plan. Domain audits provide exact existing APIs; contracts not yet proven must be settled before their implementation task, never invented from a mockup. Each phase becomes a focused implementation checklist after architecture/design approval. The 14 November plan remains historical correctness input only; do not execute its feature exclusions or vanilla-only module split as the full-product destination.

