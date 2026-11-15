# Feature acceptance status

Updated 15 November 2026. The implementation remains Rust/Tauri with the
pinned React renderer and canonical Hermes gateway. No GPUI port exists or is
being assumed. UI acceptance runs in the desktop application.

“Screen opens”, automated contract checks and real service operation are
different evidence. None of the rows below is a blanket release approval.

| Feature family | Evidence available | Acceptance still required |
| --- | --- | --- |
| Navigation and settings | Seven destinations and nine settings categories observed in the native fixture app; current settings regression suite: 82 files, 628 passing tests | Compact-window, complete keyboard and full inventory/dynamic-schema acceptance |
| All settings | Typed catalogue renders natively; new owner tests cover captured write scope, resetting drafts on owner change and delayed completion against the old cache | Revision conflicts, secret keep/replace/clear, every unknown-key/type case |
| Voice | Guided setup renders natively; tests cover microphone lifecycle and mode/provider controls | Real microphone, native permissions/codecs, provider transcription/playback, live conversation and reconnect |
| Models and providers | Existing model/provider editors retained; included in settings regression coverage | Actual provider authentication, unavailable-model recovery and active-session switching against a controlled runtime |
| Automations | Native destination and editor retained; fake data is explicitly isolated | Managed CRUD, timezone/DST, pause/resume/run/stop, run history and multi-target delivery against an isolated gateway |
| Connections, Telegram and MCP | Existing screens retained and native destination observed | QR/manual/relay expiry and cancellation, account policies, OAuth callbacks and remote ownership |
| Skills and plugins | Catalogues and contributed destinations retained; native page opens | Install/update/unload/grant lifecycle with fake services and complete native adapter coverage |
| Tasks, agents and boards | Native work board observed; board and process-evidence tests pass | Persistent-agent canonical chats, parallel delegation, child navigation, parent stop and reconnect against the gateway |
| Run/test inspector | Real passing/failing Node output exercised; background exits do not invent complete test counts; scoped process evidence covered | Native event replay/gaps, cancelled/background executions and structured report adapters end to end |
| Native media and windows | Rust range-response tests; existing shell tests pass | Live remote playback, additional-window wiring, capture/preview, hotkeys, vault and remaining native members |
| Performance | Recorded document-load and post-load RSS samples preserved with corrected definitions | First rendered frame, actual interactive readiness, long transcripts, streaming, reconnect churn and release-build comparison |

Fresh transport checks also pass 10/10: queued final frames survive local close,
and caller buffer reuse cannot change bytes already accepted by `send`.

Detailed historical evidence and remaining adapter gaps:
[work report](PHASES-6-11-REPORT.md), [native review](NATIVE_UI_REVIEW.md),
[performance definitions](PERFORMANCE.md), [progress](PROGRESS.md).
