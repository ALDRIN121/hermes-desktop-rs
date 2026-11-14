# Dynamic execution, models, and Rust architecture
Updated 2026-11-15. Planning/diagnostic evidence only. No product implementation has started.

## What is implemented today

The native Tauri shell, IPC, filesystem/Git/PTY services are Rust. The custom UI is HTML/CSS/JavaScript; the original Hermes renderer is React/TypeScript compiled to web assets. The actual Hermes agent/gateway remains the existing Python runtime. Therefore this is **not an entirely Rust application**, and preserving Hermes does not require rewriting its engine.

A tracked-file size census on this checkout found 15 Rust source files / 165,291 bytes and 1,062 shipped renderer asset files / 36,294,790 bytes. Asset count includes generated/vendor resources, not only authored application JavaScript. Six other JS/MJS files totalled 3,416,121 bytes, including the copied Electron main artifact. Repository language percentages measure stored source/assets, not execution time, memory efficiency or whether Electron is running.

## Architecture choices for user review

| Approach | Feasible? | Consequences |
| --- | --- | --- |
| Rust/Tauri native shell + existing Hermes React/TypeScript renderer | Yes; recommended proposal | Preserve mature feature modules and their state/contracts; fix Rust bridge parity, redesign navigation/settings at source, profile actual bottlenecks. |
| Rust-authored web frontend, e.g. Leptos compiled to WebAssembly | Yes, technically | Still a WebView/DOM/CSS application with browser bindings; existing React modules and UI-plugin SDK require migration or interoperability. A separate rewrite, not an automatic speed upgrade. |
| Rust native GUI, e.g. Iced | Yes, technically | Replace WebView-based shell; rebuild rich chat/Markdown/editor/browser/plugin surfaces, accessibility and audio integrations. Highest feature-parity risk for this existing product. |

Tauri explicitly supports HTML/CSS/JS and WASM frontends and documents Rust frontend integrations: [official frontend guide](https://v2.tauri.app/start/frontend/). Its native core/WebView separation is documented in [architecture](https://v2.tauri.app/concept/architecture/) and [process model](https://v2.tauri.app/concept/process-model/). [Leptos](https://book.leptos.dev/) is a Rust web framework; [Iced](https://docs.iced.rs/iced/) is a Rust GUI option. The effort/parity assessment above is an inference from this app's existing React/plugin/native contracts, not a comparative performance benchmark.

**No framework migration is approved.** The full-feature plan recommends the first approach; user can choose the Rust-authored alternatives before implementation.

## Dynamic run inspector: actual evidence path

The illustration “Running tests” is not proof that the current UI detects tests. The current Task view polls snapshots and uses a narrow regex over all message roles. It needs replacement.

Source-backed existing producers:
- [Gateway tool payloads](/Users/aldrinjoseph/.hermes/hermes-agent/apps/shared/src/gateway-contract.generated.ts:6021): tool.start carries tool_id/name/args; tool.complete carries the matching ID, duration and an unknown-typed result/result_text/summary. It is **not** a universal typed test report.
- [Original reducer](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/session/hooks/use-message-stream/gateway-event/tools.ts:110): handles tool.start/tool.complete, todo.updated and subagent events, invalidates workspace state after file mutations and refreshes background process state after terminal/process completions.
- [Progress handling](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/session/hooks/use-message-stream/gateway-event/tools.ts:79): tool.progress is tool progress, not automatically test-suite progress.
- [Custom RPC](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/src/task_ops.rs:31): one-shot call ignores pushed frames. Persistent subscribed/replay-aware data is needed.

Proposed pipeline: gateway execution event → scoped Rust transport (JSON and binary preserved) → existing feature store/reducer keyed by connection/profile/session/turn/tool/process → derived run/test model → inspector. Backend authority stays with Hermes; the UI does not decide success from prose or animation.

| UI claim | Required evidence | Fallback |
| --- | --- | --- |
| Command running | Matching tool/process start, no terminal result | If terminal tool only launched a background process, track that process separately |
| Running tests | Recognized test invocation linked to a running execution, or structured test adapter start | “Running command” when command/wrapper semantics are unknown |
| N tests passed/failed/skipped | Valid structured report (preferred) or supported parser of that execution's tool output | Counts unavailable; offer raw output |
| Suite passed | Final test report plus captured successful completion/exit; no unresolved child run | Exit 0 alone means command succeeded, not all tests passed |
| Suite failed | Captured nonzero exit or final report failure | Distinguish process failure from test assertion counts |
| Turn complete | Authoritative turn completion | A tool completing does not complete the turn |
| Children finished | Matching child lifecycle/roster terminal state | Parent stop does not imply children stopped |

Do not assume every result exposes exit_code: validate the particular terminal/process result contract. Do not invent a native test.start event. If a precise test count is needed for an arbitrary runner, add an adapter/report-file contract as explicit backend integration, not a regex guessing everything.

Required cases: foreground/background tests, wrappers/containers/remote commands, Rust/pytest/Jest/Node, streaming fragments/ANSI, parallel shards, reruns/retries, cancellation/crash, missing exit, report truncation, stale profile switches, duplicate/out-of-order/replayed events and reconnect gaps. No universal per-test live progress is claimed.

## Diagnostic actually run on 15 November

A temporary Node test file had one pass and one failure. A real node --test process returned exit 1 with “tests 2 / pass 1 / fail 1.” Running the **actual current parseTests function** against this output returned zero passed and zero failed. A failed-only pytest string likewise returned zero failures. User-role prose “Example: 99 passed in 1.2s” returned 99 passes.

This confirms a current defect; it does not validate a future event-driven UI. No real agent prompt or user repository mutation was used. Scratch test path was in the OS temporary directory. Existing shell/contract results in the first audit are historical 14 November evidence, not rerun results today.

## Model search and provider configuration

Preserve the actual Hermes picker, not the current custom flat menu:
- [model-picker.tsx:84](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/components/model-picker.tsx:84): controlled search, fuzzy ranking, custom model entry; query identity includes profile/session/owner connection.
- [models.ts:32](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/api/models.ts:32): dynamic model options with refresh/unconfigured/explicit filtering.
- [models.ts:91](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/api/models.ts:91): main-default assignment is different from the composer's current-chat selection callback. Preserve scope rather than wiring every selection to global main.
- [providers-settings.tsx](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/providers-settings.tsx), [custom endpoints](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/custom-endpoints-settings.tsx), [model APIs](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/api/models.ts): auth/keys/custom endpoints, auxiliary assignments and MoA remain reachable.

Flow: choose target profile/gateway → search model/provider → configure missing provider if needed → validate its actual route → select for “This chat” or “New chats” explicitly → display acknowledged effective model. Retain custom IDs, local models where runtime supports them, fallback models, effort/capabilities, auxiliary/delegation/MoA routes and pinned unavailable models. A model switch during an active turn must follow the gateway's supported semantics; do not promise hot switching without contract evidence. Preserve draft on rejection, reject late responses from old scopes, and show effective versus pending/default model distinctly.

Acceptance must cover unavailable providers, invalid keys, refresh failure, empty search, custom IDs, revoked auth, two gateways with identical names, active-run switching, stale acknowledgements and profile defaults versus current session. No provider changed or model call made in this review.

## Optimization requires measurement

No blanket “optimized” claim is supported today. Record cold/warm launch, idle/streaming CPU, per-process and aggregate RSS (Rust + WebView + gateway separately), input latency, frame times, event-to-paint latency, long-history memory and audio underruns on the same machine/fixture.

Proposed acceptance budgets: no idle busy loop; no duplicate subscriptions after reconnect; virtualized/paged long content; bounded output buffers and IPC batches; no blocking filesystem/Git on UI thread; event coalescing that flushes terminal states immediately. Establish numeric budgets from the measured baseline before accepting improvements. JS-to-Rust extraction is justified only by measured hot paths and a behavior-preserving benchmark. Preserve optional assets/plugins unless dependency analysis proves them unused.

