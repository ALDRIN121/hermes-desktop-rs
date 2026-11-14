# Native channel coverage

Review date: 2026-11-14. Review baseline: commit `0c272d7`.

This is a source inventory for the UI and functionality review, not a claim of complete feature parity or a record of runtime verification. No product implementation was changed to produce it.

## Recomputed totals

The canonical [channel inventory](../../../crates/hermes-contract/src/channels.rs#L3), [feature-event list](../../../crates/hermes-contract/src/channels.rs#L421), and [PORTED registry](../../../crates/hermes-contract/src/registry.rs#L21) produce these counts:

| Measure | Count |
| --- | ---: |
| All inventoried literal channels | 281 |
| Requests | 236 |
| Requests listed in `PORTED` | 77 |
| Requests outside `PORTED`, refused by dispatch | 159 |
| Inventoried feature events, runtime status unverified | 45 |
| Share of requests listed in `PORTED` | 32.6% |

The accounting is `281 = 236 + 45` and `236 = 77 + 159`. The event and ported sets are disjoint. The request denominator follows [PortLedger::measure](../../../crates/hermes-contract/src/registry.rs#L124), which excludes push-only events. Missing event behavior still represents a potential feature gap; excluding events from a request percentage does not remove that work.

[channel-inventory.tsv](channel-inventory.tsv) contains one row for each of the 281 canonical channels, preserving source order. It includes the channel, request/event type, status, canonical family, classification caveat, and source locations. Evidence paths in the TSV are relative to the repository root.

## Meaning of each status

| Status | Applies to | Meaning and limit |
| --- | --- | --- |
| `listed-ported` | 77 requests | Present in the canonical `PORTED` list. A handler is listed, but its complete behavior, side effects, error handling, event delivery, and UI integration are not runtime-verified by this inventory. |
| `refused` | 159 requests | Absent from `PORTED`; the dispatch fallback returns `unimplemented:<channel>`. This is an explicit refusal, not successful completion. |
| `unverified-event` | 45 feature events | Present in `EVENT_CHANNELS`. There is no request to refuse. A producer, payload, delivery path, and consumer behavior must be checked before marking the feature verified. |

The registry's own distinction is documented in [registry.rs](../../../crates/hermes-contract/src/registry.rs#L1), and refusal occurs in [commands.rs](../../../src-tauri/src/commands.rs#L886). All TSV rows carry their classification caveat so that exporting the inventory does not lose these limits.

In particular, **`listed-ported` includes noop, null, and disabled-capability answers and does not imply that a feature is verified**. Examples include:

- `hermes:feature-flags` reports local models disabled ([commands.rs](../../../src-tauri/src/commands.rs#L369)).
- `hermes:translucency:support` reports glass and translucency unsupported ([commands.rs](../../../src-tauri/src/commands.rs#L400)).
- `hermes:hud:windowing` returns null ([commands.rs](../../../src-tauri/src/commands.rs#L426)).
- `hermes:fs:reconcileDesktopPlugins` returns empty published/refused arrays ([commands.rs](../../../src-tauri/src/commands.rs#L555)).
- Boot status is a fixed ready response for a shell that attaches to an existing gateway; it does not verify an installer or managed bootstrap flow ([commands.rs](../../../src-tauri/src/commands.rs#L402)).

These can be deliberate limits. They must remain visible in feature-level review rather than being flattened into a single “ported” checkmark.

## Deferred capability families

The following groups account for all 159 refused requests. These are product-review groupings of canonical prefixes, not new backend interfaces or an implementation commitment. The TSV remains the per-channel authority. A family appears here only for its refused requests; a family may also contain listed handlers or feature events.

| Review group | Refused requests | Canonical families included |
| --- | ---: | --- |
| Connections, accounts, profiles, and remote access | 36 | `cloud`, `connection-config`, `connection`, `connections`, `profile`, `ssh-config`, `machine`, `secret-storage`, `get-remote-display-reason`, `freeTierChallenge` |
| Plugins, file grants, and OAuth | 13 | `plugin`, `plugin-profile-routes`, `pluginFiles`, `mcp-oauth`, `workspace` |
| Browser, preview, links, and media/file handoff | 20 | `capturePreview`, `normalizePreviewTarget`, `openPreviewInBrowser`, `preview-guest-activity`, `preview-guest-hidden`, `preview-signins`, `preview`, `previewShortcutActive`, `saveGatewayFile`, `saveImageFromUrl`, `resolveFavicon`, `fetchLinkTitle`, `watchPreviewFile`, `stopPreviewFileWatch`, `openExternal`, `saveClipboardImage` |
| Extra windows, HUD, pets, quick entry, and ambient behavior | 32 | `hud-modifier`, `hud`, `pet-overlay`, `quick-entry`, `window`, `ambient` |
| App lifecycle, bootstrap repair, updates, and uninstall | 17 | `app`, `backend`, `bootstrap`, `uninstall`, `update-hold`, `updates` |
| Native interaction, system state, appearance, and directory watching | 30 | `context-menu`, `deep-link-ready`, `desktop-metrics`, `devtools`, `f12ShortcutActive`, `find-in-page`, `minimize-to-tray`, `native-theme`, `power-battery`, `setting`, `spellcheck`, `startup-latency`, `stop-find-in-page`, `stream-work-wake`, `sync-status`, `titlebar-theme`, `translucency`, `vscode-theme`, `wake-indicator`, `watchDirectory` |
| Screenshots and microphone permission | 6 | `screenshot`, `requestMicrophoneAccess` |
| Active work, agent roster, and prepared submissions | 5 | `active-work`, `agents`, `prepared-submissions` |
| **Total** | **159** | |

For each family, the implementation plan should choose a disposition: supported in the primary UI, reachable through another surface, clearly unavailable, or deferred. A handler count alone cannot establish whether a visible UI control has a complete usable path.

## Feature events versus transport events

There are several distinct event layers. They must not be combined to inflate feature coverage:

| Layer | Examples | Source evidence | Relationship to the 45-event count |
| --- | --- | --- | --- |
| Inventoried native feature events | `hermes:ui-preferences:changed`, `hermes:window-state-changed`, `hermes:updates:progress`, `hermes:notification-activate` | [EVENT_CHANNELS](../../../crates/hermes-contract/src/channels.rs#L421) | These are the 45 rows marked `unverified-event`. |
| Rust-to-renderer WebSocket transport | `hermes-ws-message`, `hermes-ws-closed` | [gateway_ws.rs](../../../src-tauri/src/gateway_ws.rs#L86) | Separate transport channels, absent from the canonical 281. Their producers do not prove a native feature event exists. |
| Browser-facing WebSocket events | Standard `open`, `message`, `error`, and `close` dispatched by the injected WebSocket shim | [bridge.js](../../../src-tauri/src/bridge.js#L255) and [subscriptions](../../../src-tauri/src/bridge.js#L272) | Browser transport behavior, not additional canonical native feature events. Gateway messages may contain application events, but their semantics require separate verification. |
| Dynamic PTY output and exit events | `hermes:terminal:<id>:data`, `hermes:terminal:<id>:exit` | [terminal.rs](../../../src-tauri/src/terminal.rs#L119) and [bridge listeners](../../../src-tauri/src/bridge.js#L183) | Dynamic transport names, absent from the literal inventory. The six terminal request channels are counted separately among the 77 listed requests. |

The source shows producers for WebSocket transport and PTY output. This document does not promote any of the 45 inventoried feature events to verified status on that basis. Likewise, browser WebSocket delivery does not establish desktop notification activation, preview changes, power state, or any other native feature-event path.

The shell also exposes dedicated Tauri commands such as `task_call`, `task_submit`, `task_stats`, `open_task`, `open_workspace`, `shell_info`, diagnostics, and `hermes_ws_*` ([main.rs](../../../src-tauri/src/main.rs#L244)). They are outside this historical Electron-channel denominator. Task JSON-RPC deliberately ignores unsolicited gateway frames in its one-request client ([task_ops.rs](../../../src-tauri/src/task_ops.rs#L31)); it is not evidence of a feature-event subscription.

## Verification scope and next evidence

This artifact verifies the static list accounting, set membership, dispatch fallback, and cited implementation limits at the review baseline. It does not rerun historical checks from `PORT_REPORT.md`, verify a user journey, or certify all consumers of the shipped renderer bundle.

Feature-level completion should require a record of the initiating UI action, actual backend operation and scope, success and error states, relevant event producer/consumer where applicable, and a reproducible result. Record the commit, date, fixture, viewport, and evidence path with that result. Until that exists, retain the difference between a listed handler and a verified feature.

Static inventory checks used for this document:

- Exactly 281 unique canonical channels and 281 TSV data rows.
- All 77 `PORTED` entries and all 45 `EVENT_CHANNELS` entries belong to the canonical inventory.
- No channel is both listed-ported and a feature event.
- Exactly 236 requests, comprising 77 listed-ported and 159 refused rows.
- Deferred-family groups cover each refused row exactly once and sum to 159.
- Source paths and line locations in the TSV match the parsed canonical declarations or refusal branch.
