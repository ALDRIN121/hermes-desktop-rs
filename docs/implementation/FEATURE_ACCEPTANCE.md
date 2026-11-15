# Feature acceptance status

Updated 15 November 2026. The implementation remains Rust/Tauri with the
pinned React renderer and canonical Hermes gateway. No GPUI port exists or is
being assumed. UI acceptance runs in the desktop application.

“Screen opens”, automated contract checks and real service operation are
different evidence. None of the rows below is a blanket release approval.

| Feature family | Evidence available | Acceptance still required |
| --- | --- | --- |
| Navigation and settings | Seven destinations and nine settings categories observed in the native fixture app; current settings regression suite: 82 files, 631 passing tests | Compact-window, complete keyboard and full inventory/dynamic-schema acceptance |
| All settings | Typed catalogue renders natively; owner isolation, literal dotted keys, finite-number/JSON typing and secret-search regressions pass | Revision conflicts, schema-based secret keep/replace/clear and unknown enum/type cases |
| Voice | Guided setup renders natively; tests cover microphone lifecycle and mode/provider controls | Real microphone, native permissions/codecs, provider transcription/playback, live conversation and reconnect |
| Models and providers | Existing model/provider editors retained; included in settings regression coverage | Actual provider authentication, unavailable-model recovery and active-session switching against a controlled runtime |
| Automations | Native destination and editor retained; fake data is explicitly isolated | Managed CRUD, timezone/DST, pause/resume/run/stop, run history and multi-target delivery against an isolated gateway |
| Connections, Telegram and MCP | Existing screens retained and native destination observed | QR/manual/relay expiry and cancellation, account policies, actual account OAuth and remote registry ownership; native MCP callback fixtures pass |
| Skills and plugins | Catalogues and contributed destinations retained; native page opens | Install/update/unload/grant lifecycle with fake services and complete native adapter coverage |
| Tasks, agents and boards | Native work board observed; board and process-evidence tests pass | Persistent-agent canonical chats, parallel delegation, child navigation, parent stop and reconnect against the gateway |
| Run/test inspector | Real passing/failing Node output exercised; background exits do not invent complete test counts; scoped process evidence covered | Native event replay/gaps, cancelled/background executions and structured report adapters end to end |
| Native media and windows | Native peer creation/draft isolation observed; generated 24-second silent WAV played to completion in native WebKit through authenticated, session-scoped range requests; Rust media and window tests pass | Real remote gateway playback, additional codecs, single-session/spectator UI acceptance, capture/browser pop-outs, hotkeys, vault and remaining native members |
| Performance | Recorded document-load and post-load RSS samples preserved with corrected definitions | First rendered frame, actual interactive readiness, long transcripts, streaming, reconnect churn and release-build comparison |

Fresh bridge checks pass 15/15. Rust regression: 22 contract + 68 shell tests pass.
Coverage includes queued final frames, binary snapshots, window ownership and
delayed-handshake cancellation. Settings: 82 files / 631 tests. Import tests: 7/7.
TypeScript, modified-settings lint, renderer build and native build passed.

See the [native integration pass](NATIVE_INTEGRATION_PASS.md) for independent
review findings, native observations, corrected test side effects and remaining
release blockers.

Detailed historical evidence and remaining adapter gaps:
[work report](PHASES-6-11-REPORT.md), [native review](NATIVE_UI_REVIEW.md),
[performance definitions](PERFORMANCE.md), [progress](PROGRESS.md).
