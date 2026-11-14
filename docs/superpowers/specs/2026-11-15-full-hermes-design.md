# Full Hermes experience and simpler configuration
**Status:** Approved for implementation by user instruction on 2026-11-15; generated images remain binding visual references.
**Updated:** 2026-11-15. This supersedes the limited-scope 14 November design where it deferred original Hermes functionality.

## User requirements

Keep Hermes under the hood and preserve all existing feature families. Make navigation/configuration easier without dropping advanced options. Include voice, scheduled jobs, Telegram/social integrations, skills/connectors, persistent agents, live delegated tasks, model search/switching and provider configuration. Run/test indicators must be dynamic and evidence-backed. Assess Rust frontend feasibility and actual performance before claiming optimization.

## Approaches considered

1. Reuse the original Hermes renderer modules in a unified source-built app with a Rust/Tauri native adapter — recommended for parity.
2. Rewrite presentation in Rust/WASM — feasible, but a separate renderer/plugin migration; still uses browser rendering.
3. Replace the WebView with a Rust native GUI — feasible, highest reconstruction and parity burden.

The user has not selected a framework migration. This specification proposes option 1. Hermes runtime remains Python; native privileged services remain Rust; React/TypeScript handles presentation. See [feasibility and evidence](../../reviews/2026-11-14-ui-audit/FEASIBILITY_AND_ARCHITECTURE.md).

## Evidence and scope boundary

Source inventory: upstream checkout 3b421578588243f920886e6fe79a6faa87ea026e. Shipped renderer stamp 2026-11-12, build sourceHash a8b4866557bfd351e263ecab4ca9f915e072b5fc20dc5e2715198a1da8ad702a. Build-input hash is not the Git SHA; reconcile source/build deltas before freezing acceptance.

[Settings audit](../../reviews/2026-11-14-ui-audit/SETTINGS_AUDIT.md) and [1,055-row source inventory](../../reviews/2026-11-14-ui-audit/settings-inventory.tsv) cover 918 finite schema keys, 53 hand-built manifest rows, 78 dynamic families and six extra control families. Counts are inventories, not claims that every field already has a working UI. Dynamic catalogs remain open-ended.
[Voice/jobs/agents](../../reviews/2026-11-14-ui-audit/VOICE_AUTOMATION_AGENTS.md), [connections/skills](../../reviews/2026-11-14-ui-audit/CONNECTIONS_SKILLS_PARITY.md) define detailed subfeatures/contracts.

“All features” means preserve the pinned Hermes desktop routes, native functions, relevant gateway capability families, supported advanced settings and extension contribution points. Backend-only features discovered in source remain accessible through typed advanced settings/feature routes where supported; source/build/version gaps require explicit reconciliation, not a false implemented label. Unknown future plugins are discovered dynamically. Platform-specific features keep their actual availability; an unavailable native port is an unresolved parity item, not permission to remove the capability.

## Product organization

One Hermes app, rather than a coding-only Task page plus a hidden full product. Global navigation:
- Chat: complete sessions, attachments, voice, model picker, coding split/review and true terminal/browser panes.
- Tasks & agents: separate tabs for persistent Agents, Live runs/subagents, and Boards when the Kanban plugin is enabled.
- Automations: Scheduled jobs, Run history, Webhooks.
- Connections: Messaging apps; Service accounts/connectors; MCP servers; gateway/profile ownership with clear subareas.
- Skills & tools: Skills, agent/desktop Plugins, Toolsets; links to connected accounts rather than duplicate editors.
- Library: artifacts, session import/export, memory/knowledge/starmap destinations and contributed content routes.
- Settings: simpler configuration with all existing options retained.

Keep old route/deep-link IDs redirectable, contributed routes mountable, command palette and skill/quick-command discovery intact. “Classic” may be a temporary comparison/migration route; it is not the final substitute for missing features.

## Simpler settings, full capability

Nine groups: General; Models & providers; Voice & audio; Connections & accounts; Workspace & browser; Memory & safety; Skills & extensions; Advanced & diagnostics; About & updates. Account/billing, passwords/logins, archived sessions and plugin pages retain explicit discoverable destinations within these groups.

Search covers friendly names, original labels and exact config keys, including advanced/conditional fields and plugin schemas. “All settings” is a schema-backed catalogue, not raw text with no validation. Unsupported/schema-unknown values remain visible and round-trip through a validated advanced editor; no silent key deletion. Preserve empty arrays/maps and explicit clear semantics.

Common setup uses required fields first, short explanations, inline validation, Test connection and scope/effect summaries. Advanced options expand within the relevant section; search opens the correct disclosure and highlights the field. Preserve enum/custom values, nested collections, dependencies and unknown provider/plugin options. Every inventory entry maps to a destination or justified runtime/platform state.

Always show owner gateway/profile and effective scope (device/window/profile/session). Keep vs replace vs clear secret operations distinct; retain masked values without sending masks as secrets. Do not copy credentials into diagnostics, images or local draft storage. Preserve existing sensitive reveal/authorization semantics.

Saving shows actual pending differences and whether a change applies now, next turn, next session, reconnect or restart. Do not override the original prompt-cache invariants by applying every setting mid-conversation. Conflict/version failures retain edits and demand refresh/merge. Switching scope never saves edits to the new target. Settings overview distinguishes configured/authenticated/reachable/enabled states.

## Required feature preservation

| Family | Required destination and behavior |
| --- | --- |
| Voice | Dictation, streamed STT/blob fallback, spoken replies, chained/live conversation, provider/voice selection, wake word, interruption/barge-in/stop phrases, devices/permissions, direct/relay and reconnect/cleanup. Preserve binary PCM and speech frames end to end. |
| Scheduled jobs | Create/edit/delete, enable/pause/resume, run now/stop, blueprints, schedule/timezone/next-run preview, model/agent/workspace/tools/skills overrides, limits/conditions/missed-run policy, multiple delivery destinations, durable run output/history. |
| Messaging | Dynamic adapter catalog; Telegram QR/manual and relay flows, pairing/allowlists/revocation, groups/topics/home/reply policies, lifecycle/test/readiness and other source-supported platforms. Desktop/backend-only setup gaps remain required integration work. |
| Service connectors | Dynamic catalog, multiple accounts/aliases, auth/reconnect/remove, per-tool and organization policy, revision conflicts, in-chat connection requests. Do not infer capabilities from logos. |
| MCP | stdio/HTTP servers, import/raw configuration, OAuth/bearer/headers/env/cwd, probes/logs/tools/resources/prompts, filters/reload and desktop callback ownership for remote gateways. |
| Skills/plugins/tools | Install/search/inspect/update/remove, scans/prerequisites/provenance/pins, user edit/lock, enable/bulk enable, slash discovery; agent and desktop halves, grants, contributed pages and activation timing. Social skills preserve actual read/write abilities rather than implying every platform can publish. |
| Persistent agents | Profile-backed identity, SOUL/model/capabilities, roster across gateways, canonical Bot Chat, side chats, groups/routines, cloning/import/export. |
| Live work | Authoritative task/subagent/process hierarchy and outcomes, background activity after parent stops, reconnect/replay, inspect task outputs and return to owning chat. |
| Boards | Kanban plugin remains optional but fully functional when enabled: durable tasks/dependencies/assignees/comments/events/attachments/runs and recovery. |
| Models | Search/rank/custom IDs, provider setup/auth/custom endpoints, session/default distinctions, fallback/auxiliary/delegation/MoA/local model surfaces where supported, effective/pending state and failure rollback. |
| Remaining original surfaces | Artifact/media/browser/preview, genuine PTY, memory/context, checkpoints/review, profiles/cloud/SSH, vault/keys/permissions, notifications/hotkeys/HUD/quick entry/pets, updates/install/diagnostics/web access/account/billing, existing extension routes. No silent feature retirement. |

## Dynamic execution contract

No “Running tests” from arbitrary prose or from idle=false alone. Tool starts/results/progress and process/subagent/turn lifecycle drive scoped stores. Background launch completion is not background test completion. Test counts need a recognized per-execution report/parser with provenance; arbitrary runner support cannot be guaranteed. Exit zero alone is command success, not suite success. Missing/truncated/disconnected data produces unknown/stale, preserving raw output.

UI states: queued, starting, running, waiting for input, stopping, cancelled, failed, succeeded and unknown/stale. Keep command outcome, suite outcome, child outcome and turn outcome separate. No fake percentage without a real denominator. Plans are agent plans, not test counters.

A future report adapter must define framework, execution ID, report path/schema, completion marker and exit association before advertising per-test live progress. Handle duplicate/replayed/out-of-order events and bound memory; do not repurpose MCP progress as universal test progress.

## Layout and migration

Retain the approved-for-review dark indigo visual proposal, readable type/focus/reduced-motion and responsive mode principles from v1; no approval is implied. On compact windows show one main content view and accessible navigation/drawers; do not squeeze all full-product destinations into four fixed columns. Preserve live terminal/audio lifetimes when panes hide. Proposed visual refinements are in [full-feature images](../../reviews/2026-11-14-ui-audit/FULL_HERMES_VISUALS.md).

v1 source findings and safe draft/review-action requirements remain relevant. Its vanilla-only architecture, read-only jobs/agents scope, attachment deferral and blanket Classic feature deferrals are superseded. All existing attachments/native capabilities are part of parity now, subject to actual contracts.

## Performance and acceptance

Measure Rust, WebView and gateway independently and together. Record startup, idle/streaming CPU/RSS, input latency, long-history rendering, event-to-paint delay, reconnection leaks, output throughput and audio continuity. Optimize measured hot paths; preserve existing functionality. No “all Rust” or “optimized” label without evidence.

Release gates: all inventory entries dispositioned and source/build reconciled; complete user journeys at local/default, named-profile and remote scopes; no credential/identity crossover; real foreground/background test fixtures reflected in native UI; provider/model error tests; all settings/unknown values round-trip; voice binary/permissions tested; schedule delivery through explicit test recipients only; extension lifecycle verified; accessibility/compact/native minimums; dated benchmarks/screenshots. Mock screens and source inspection alone cannot pass those gates.

Implementation authorized on 2026-11-15 with recommended Rust/Tauri + existing renderer + canonical gateway architecture. Implementation and verification progress is tracked in docs/implementation/PROGRESS.md.

