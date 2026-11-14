# Voice, automation, agents, and task parity

Planning evidence only, captured 2026-11-14. Every capability below is required for the redesigned Rust experience. Phasing controls implementation order; it does not remove advanced features from the destination.

## Provenance and confidence

- **Original source inspected:** `/Users/aldrinjoseph/.hermes/hermes-agent`, HEAD `3b421578588243f920886e6fe79a6faa87ea026e` (`3b4215`). Only source files were read. No user configuration, credentials, schedules, transcripts, databases, or other session state were inspected.
- **Rust checkout inspected:** `/Users/aldrinjoseph/Developer/hermes-desktop-rs`, HEAD `0c272d7064917d8b22150decdf5e99787ebf2073`, plus the working tree's custom shell files.
- **Shipped renderer:** [hermes-build.json](/Users/aldrinjoseph/Developer/hermes-desktop-rs/ui/hermes-build.json:1) records stamp `2026-11-12T14:10:31.718Z`, source hash `a8b4866557bfd351e263ecab4ca9f915e072b5fc20dc5e2715198a1da8ad702a`, and preparation paths in the original source checkout. That source hash is a build-input hash, not the Git commit above. Current source and shipped bundle must not be treated as proven identical. No compiled-bundle reverse engineering was needed for this inventory.
- **Confirmed** means demonstrated by the cited source. **Gap** means the current custom implementation lacks the source contract, or the Rust transport contradicts it. **Unknown** means runtime behavior was not tested; a static route or visible control alone is not a passing feature test.
- No gateway calls, recordings, provider requests, job mutations, agent controls, or browser actions were performed for this report.

The original desktop contract assigns native capability to the shell, presentation to the renderer, and agent behavior to the canonical gateway. Closing a viewer disconnects it; it must not terminate the gateway owner. Calls and new sessions retain their owning connection and profile. See [desktop backend contract](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/AGENTS.md:5).

## Current-surface matrix

| Capability | Original source contract | Rust Classic renderer | Custom Task / Workspace |
|---|---|---|---|
| Dictation | Record, meter, partial transcript, final insertion into draft, STT warm-up, recording limit, empty-speech feedback, blob fallback | UI/source reuse is not runtime proof. Native microphone bridge is absent; local binary streaming is incompatible with Rust's current socket shim | No dictation or microphone flow in `task.js` / `task.html`; `codex.js` is a repository review surface |
| Spoken replies | Toggle, streamed speech, direct-provider/relay/whole-text fallback, interruption, owner-scoped playback, TTS lease | JSON REST transport exists; binary relay streaming is incompatible; playback/capture and background behavior remain unverified | No speech-output flow |
| Hands-free voice | Chained and GPT-Live engines; wake word; barge-in; stop phrases; captions/activity; wake pause/re-arm | Renderer contains the experience; WebKit capture, WebRTC, autoplay and native lifecycle parity are unknown; wake-indicator and microphone native methods are absent | No voice conversation or wake flow |
| Scheduled jobs | Job list/detail, create/edit/delete, pause/resume, Run now/Stop, run history, blueprints, delivery targets, advanced runtime fields | Generic JSON REST exists and can carry these routes; per-profile/connection routing must be verified, not assumed | A compact display consumes the sidebar response's `cron` session slice. It does not fetch first-class jobs or implement job actions |
| Live subagents | Lifecycle events, scoped roster reconciliation, nested identity, live tools/progress, terminal results, steer, individual stop, bounded transcript tail | Main-window text event transport exists; ownership/reconnect behavior still needs runtime verification | Delegation summaries are inferred from transcript tool calls; no native roster/events/control contract |
| Durable task board | Optional Kanban plugin, boards/tasks/dependencies/runs/logs/diagnostics/orchestration, live events | Plugin UI reuse exists in source. Native plugin socket, binary file transfer and connection scoping require parity verification | Custom Task window is not the Kanban board and provides none of its durable task-management contract |

Evidence for the custom limitations: [sidebar load](/Users/aldrinjoseph/Developer/hermes-desktop-rs/ui/task.js:159), [scheduled display](/Users/aldrinjoseph/Developer/hermes-desktop-rs/ui/task.js:238), [heuristic subagent extraction](/Users/aldrinjoseph/Developer/hermes-desktop-rs/ui/task.js:609), [subagent modal](/Users/aldrinjoseph/Developer/hermes-desktop-rs/ui/task.js:788), [session polling](/Users/aldrinjoseph/Developer/hermes-desktop-rs/ui/task.js:1331), and [Workspace scope](/Users/aldrinjoseph/Developer/hermes-desktop-rs/ui/codex.js:1).

## Voice inventory and contracts

### Recording and transcription

1. **Dictation edits a draft.** Start/stop recording, report `recording` versus `transcribing`, show elapsed time and a live audio meter, display partial speech, and insert the settled transcript into the composer. Empty speech produces useful feedback. Recording is bounded by the configured maximum; current desktop clamps the cap to 1–600 seconds.
2. **Warm the correct STT engine while recording.** Acquire an input lease when capture opens, await warm-up before transcription, and release the same owner's lease when the take settles or is cancelled. Cold model downloads must not become a transcription timeout or an indefinitely busy microphone.
3. **Live dictation with a recorded fallback.** Send microphone PCM while the user speaks; receive partial and final text. If the stream is unavailable, drops, or returns no usable result, transcribe the recorded blob. Retain the first buffered PCM chunks while the stream opens.
4. **One operation has one owner.** Resolve the recording's `(connection, profile)` when the microphone opens. Warm-up, provider config, streaming, final upload, and lease release retain that owner if the user changes profiles during the take. Late results cannot insert into another session or resurrect a cancelled take.
5. **Capture failures are distinct states.** Unsupported media APIs, denied permission, no device, no speech, capture failure, provider/transcription failure, and cancellation need separate recovery. Native permission success alone is insufficient; actual capture must work.

Evidence: [recorder state and fallback](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/hooks/use-voice-recorder.ts:89), [owner, warm-up and cap](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/hooks/use-voice-recorder.ts:170), [capture permission and microphone open](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/hooks/use-mic-recorder.ts:290), [stream protocol client](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/lib/voice-stream.ts:29).

### Speech output and conversations

- **Read replies aloud:** a visible toggle and playback state; the latest unspoken reply is selected without replaying a backlog. Voice conversations speak every unspoken assistant bubble in the turn in order, including narration and the final answer. Speech must not duplicate across surfaces.
- **Speech pipeline:** direct provider when supported, gateway streaming PCM relay, then complete-text synthesis/data-URL fallback. Sentence speech overlaps text generation. Stop/barge-in tears down current playback; stale requests cannot restart it.
- **Chained conversation:** microphone → STT → normal Hermes turn → TTS → re-listen, with silence detection, thinking sound, speech activity, and a clear end control. Speaking during generation or playback can interrupt the agent turn and preserve the user's full interjection through pre-roll capture.
- **Stop phrases:** exact configured phrases end the conversation instead of becoming a normal prompt. Empty phrase lists disable this mechanism; custom phrases affect the displayed hint. The setting must not be reduced to a hardcoded English word.
- **GPT-Live conversation:** source provides an alternate full-duplex WebRTC voice engine. The voice layer delegates real work to the same Hermes chat/model/tool/approval machinery. Recent chat history seeds the call, transcript/captions remain visible, progress and answers return to the voice layer, and results from obsolete delegations are dropped. Engine selection is latched at conversation start; a setting change takes effect next call.
- **TTS leases:** read-aloud and chained conversation acquire/release independent output leases. Local engines can preload/download on first use and unload after the last surface releases its lease. Toggle feedback must not claim synthesis succeeded merely because warm-up was requested.
- **Keyboard access:** `composer.dictate` controls dictation; `composer.voice` toggles conversation for the intended composer. Disabled or unfocused targets cannot acquire the microphone accidentally.

Evidence: [voice menu](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/voice-menu.tsx:110), [ordered reply selection and voice submit](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/hooks/use-composer-voice.ts:131), [barge-in capture](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/lib/voice-barge-in.ts:1), [voice hotkeys](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/hooks/use-composer-voice.ts:311), [TTS leases](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/hooks/use-composer-voice.ts:407), [speech streaming/fallback](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/lib/voice-playback.ts:387), [GPT-Live WebRTC](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/lib/voice-live.ts:284).

### Wake word

- Persistent on/off, current listening/pending/unavailable state, phrase and refusal reason; “another surface owns the listener” is different from off or missing engine.
- Server-side detection can consume **client microphone PCM**, so remote/headless gateways do not need a sound device. Source client capture resamples to 16 kHz mono and feeds `wake.feed`. Dead tracks, stalled callbacks, sustained digital silence and refused feeds stop the false “listening” state and release the lease.
- `wake.detected` stops wake capture before opening conversation capture. It can start a new session or continue the current one, and route to another profile's phrase. Wake listening pauses before a conversation opens its microphone and is reconciled/re-armed after the call ends.
- Advanced runtime settings include surface ownership, capture location, input device, detection provider, phrase, sensitivity, confirmation frames, new-session behavior, profile routing, and provider-specific model/keyword paths. Supported configured engines include automatic selection, openWakeWord, sherpa and Porcupine; provider installation and credentials may be required.

Evidence: [client capture and watchdog contract](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/lib/wake-client-capture.ts:1), [wake ownership/refusal handling](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/store/wake-word.ts:153), [detected-event routing](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/contrib/wiring.tsx:833), [pause/re-arm barrier](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/hooks/use-composer-voice.ts:346), [all wake configuration defaults](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/config_defaults.py:1244).

### Voice API / event matrix

All applicable HTTP calls carry the operation's profile and connection scope. Endpoint existence does not prove this Rust shell routes a secondary or remote profile correctly.

| Contract | Payload / result and use | Evidence |
|---|---|---|
| `POST /api/audio/transcribe` | `{data_url, mime_type}` → `{ok, transcript, provider}`; normal empty speech is an empty result; size/MIME validation and temporary-file cleanup | [audio router:89](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/audio.py:89) |
| `GET /api/audio/voice-config` | Profile's direct STT/TTS wire config, or `mode: relay`; credentials are held in client memory only; local/plugin providers remain relay-backed | [audio router:152](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/audio.py:152) |
| `WS /api/audio/transcribe-stream` | Initial `{sample_rate}`, binary signed-16-bit little-endian mono PCM, `{eos:true}`; server `partial`, `final`, `error`; disconnect cancels | [audio router:521](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/audio.py:521) |
| `POST /api/audio/speak` | `{text}` → audio data URL, MIME and provider | [audio router:298](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/audio.py:298) |
| `WS /api/audio/speak-stream` | Incremental `{text}`, `{done:true}`, `{stop:true}`; server `start` metadata, **binary PCM**, `end`, or `fallback` | [audio router:604](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/audio.py:604) |
| `POST /api/audio/stt-lease`, `/tts-lease` | `{lease,active}` warm-up/release; long cold-start budgets; releases retain their original owner | [API:221](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/api/system.ts:221), [STT router:487](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/audio.py:487) |
| `GET /api/audio/elevenlabs/voices` | Account voice IDs/labels augment editable custom voice inputs | [provider fields:112](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/voice-provider-fields.tsx:112) |
| `GET /api/audio/voice-live/status` | Non-secret selected mode/readiness | [audio router:183](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/audio.py:183) |
| `POST /api/audio/voice-live/session` | Byte-exact SDP offer plus history → session/SDP answer; project key stays on backend | [audio router:193](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/audio.py:193) |
| `wake.status/start/stop/pause/resume/feed` JSON-RPC | Ownership, persistent enablement and mic feeding; `wake.start` can take minutes on first install | [wake methods:551](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/methods_voice.py:551), [feed:687](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/methods_voice.py:687) |
| `wake.detected` gateway event | Phrase fired; release capture, route profile/session, begin conversation | [wiring:833](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/contrib/wiring.tsx:833) |

### Voice settings: simplify presentation, preserve the full model

The existing Settings source separates Conversation, Transcription and Speech. Preserve that understandable grouping while making common setup one flow: **Microphone → Understand me → Speak back → Try voice**. Show selected engine/provider, readiness and one short test result. A composer menu retains Start voice, Dictate, Speak replies and Wake word with current-state indicators. A recording strip needs an explicit stop/cancel and visible transcription state; calls need clear listening/thinking/speaking/interrupted/reconnecting states.

Keep an **Advanced** expansion per group, searchable by both friendly label and exact config key. Preserve custom model/voice entry, provider-specific language and synthesis controls, direct/relay policy, recording limit, streaming, transcript echo, silence timing, barge-in sensitivity/grace, thinking/beep sounds, stop phrases, wake ownership/capture/device/engine settings and model paths. The original curated form does not expose every runtime knob; schema-backed access must retain those capabilities rather than narrowing the backend to the form.

Confirmed curated STT choices include local, Groq, OpenAI, Mistral, xAI, ElevenLabs and DeepInfra. The curated TTS list includes Edge, OpenAI, ElevenLabs, xAI, MiniMax, Mistral, Gemini, NeuTTS, KittenTTS and Piper; the runtime registry additionally includes DeepInfra and plugin providers ([registry](/Users/aldrinjoseph/.hermes/hermes-agent/agent/tts_registry.py:25)). Avoid a frozen dropdown as a provider/model compatibility contract. Use runtime catalogs/availability and editable custom values.

Evidence: [voice page grouping](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/config-subpages.ts:72), [all curated voice fields](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/constants.ts:793), [provider/model options and custom input](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/constants.ts:250), [advanced runtime voice defaults](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/config_defaults.py:1212), [scope-safe diff autosave](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/voice-provider-fields.tsx:46).

## Scheduled jobs inventory and contracts

### First-class jobs and executions

- List jobs across profiles or one concrete profile; search and select a job; show schedule, enabled/paused/error/completed/running state, last/next run and overdue condition.
- Create manually or from typed **automation blueprints**, including plugin-provided blueprints. Blueprint catalog and creation target the same concrete profile; “all profiles” is not a writable owner.
- Edit name, prompt, schedule, delivery, model/provider override and missed-run policy in the existing desktop form. Defaults can be restored without preserving a stale model pin. An unavailable previously pinned model remains visible for correction.
- Schedule presets include daily, weekdays, weekly, monthly, hourly, every 15 minutes and custom input. Backend parsing also accepts duration/interval strings (`30m`, `every 2h`), natural weekday/time phrases, five-field cron and ISO one-shots.
- Pause/resume controls the future schedule. **Stop controls a live execution** and is separate from pause. Run now has immediate queued feedback, de-duplicates an in-flight trigger, waits for the real persisted result and can be stopped while pending.
- Run history includes real agent sessions, failed/cancelled attempts and script-only output documents. Session rows retain connection/profile ownership when opened. A live or properly closed agent run can be resumed; an unclosed run no longer owned by a live scheduler is view-only. `is_active` is an activity window, not proof of scheduler ownership.
- Sidebar job rows support compact next-run countdown, run-history quick peek, Manage, Run now, pause/resume and delete. Counts and lists come from **jobs**, not a list of past cron sessions.
- Multi-target delivery uses backend-provided configured targets. Preserve multiple destinations, local output, origin/home routing and explicit destinations; flag a configured platform missing its cron home target. Do not silently drop an existing saved destination because it no longer appears in the current target catalog.
- Source change events (`cron.changed`, `sessions.changed`) refresh current-owner caches; bounded polling is the fallback. Stale requests from a previous profile/connection cannot publish into the new owner's list.

Evidence: [Cron API](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/api/cron.ts:20), [schedule presets](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/cron/index.tsx:117), [form/model preservation](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/cron/cron-job-model.ts:136), [run/stop ledger semantics](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/cron/run-control.ts:3), [run transcript policy](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/cron/open-cron-run.ts:20), [sidebar job contract](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/sidebar/cron-jobs-section.tsx:30), [scoped cache tokens](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/store/cron.ts:20), [change event handling](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/session/hooks/use-message-stream/gateway-event/lifecycle.ts:77).

### Complete authored job model

The new form must preserve fields it does not edit, and provide an advanced route for all supported authored fields. Existing desktop TypeScript shapes are narrower than the runtime. Do not treat them as the full Hermes capability boundary.

| Field family | Required retained fields / semantics |
|---|---|
| Identity and lifecycle | `name`, `prompt`, `paused`, `paused_reason`, `pinned`; enable/pause state; finite `repeat` versus unlimited and completed counter |
| Schedule | `schedule`, display, `timezone` IANA pin or inherit profile zone, `grace_minutes` including meaningful `0`, `catch_up` true/false/default |
| Agent selection | `skills` / legacy `skill`, `model`, `provider`, `base_url`, `reasoning_effort`, `enabled_toolsets` |
| Execution context | `workdir` and its project instructions, `context_from` one or several jobs' last outputs |
| Script execution | `script` file, `interpreter`; script output can feed the prompt; `no_agent` makes script output the whole job |
| Change monitors | `monitor_script` or `monitor_url`; unchanged output suppresses the expensive agent run; mutually exclusive with each other and incompatible with script-only `no_agent` |
| Results / delivery | `deliver`, `origin`, `attach_to_session`, `failure_deliver`; multiple destinations and continuation/mirroring policy |

Exact runtime source: [create_job signature and semantics](/Users/aldrinjoseph/.hermes/hermes-agent/cron/jobs.py:1809), [authored field ownership](/Users/aldrinjoseph/.hermes/hermes-agent/cron/job_definition.py:13). REST create currently exposes a subset through [CronJobCreate](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_models.py:325); updates accept a dictionary through the existing normalization path. UI expansion may require extending a generated contract/REST model for fields that currently enter through CLI/tool paths. Preserve API/CLI-backed advanced behavior until that route is implemented; do not discard it during save.

Scheduler-owned fields such as next-run bookkeeping, pending occurrences, execution ledger ownership and repeat completion counts remain backend-owned. Preserve timezone/catch-up semantics, at-most-once accounting, per-profile ticker scope, restart-safe worker ownership, script/inactivity timeouts and gateway survival beyond a viewer window. Background delegated tasks are process-local; durable scheduled work belongs to cron. See [scheduler invariants](/Users/aldrinjoseph/.hermes/hermes-agent/cron/AGENTS.md:1).

### Cron API matrix

| Method and route | Meaning |
|---|---|
| `GET /api/cron/jobs?profile=<name|all>` | First-class job snapshot |
| `GET /api/cron/jobs/{id}` | Job detail |
| `POST /api/cron/jobs` | Create |
| `PUT /api/cron/jobs/{id}` with `{updates}` | Edit |
| `DELETE /api/cron/jobs/{id}` | Delete, with scheduler/run semantics enforced by backend |
| `POST .../{id}/pause`, `/resume` | Future schedule state |
| `POST .../{id}/trigger` | Run now; long synchronous result, not an instant “done” acknowledgment |
| `POST .../{id}/cancel` | Request stop; `{cancelled, execution_id, job_id, detail?}`; poll targeted attempt until terminal |
| `GET .../{id}/runs?limit=N` | History/session/output rows |
| `GET /api/cron/delivery-targets` | Configured local/platform destinations and missing-home metadata |
| `GET /api/cron/blueprints?profile=...` | Built-in and profile plugin templates |
| `POST /api/cron/blueprints/instantiate?profile=...` | Typed values become a real job |
| `POST /api/cron/fire` | Existing external fire webhook capability; keep its auth/trigger behavior backend-owned |

Routes: [cron router:734](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/cron.py:734), [webhook:810](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/cron.py:810), [blueprints:929](/Users/aldrinjoseph/.hermes/hermes-agent/hermes_cli/web_routers/cron.py:929). Renderer request/ownership examples: [cron API:31](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/api/cron.ts:31).

### Friendlier scheduling flow

Use a permanent **Scheduled jobs** destination with New job and templates, a scannable list, and a detail pane with **Overview / Runs / Advanced**. New job asks “What should Hermes do?”, “When?”, “Where should results go?” and shows a human-readable schedule with owner, timezone and next occurrence before save. Model override and missed-run policy remain easy to find. Advanced groups preserve the full authored model above, including script-only and change-monitor jobs.

Keep Run now, Stop run and Pause schedule separately labelled. In Runs, distinguish Queued, Running, Succeeded, Failed, Cancelled and output-only records. Show a short error with expandable details. A job's output opens the owning transcript/output without changing another profile's selection silently. “Saved” and “Run succeeded” are different receipts.

## Subagent and run visibility inventory

### Identity, hierarchy, state, and controls

- Native lifecycle is `subagent.spawn_requested`, `subagent.start`, `subagent.thinking`, `subagent.tool`, `subagent.progress`, `subagent.complete`. Keep `subagent_id`, `parent_id`, `child_session_id`, `delegation_id`, depth, task index/count, model and owning session. Group concurrent batches by delegation ID; preserve nested trees.
- State includes queued/running/completed/failed/interrupted, terminal summaries/errors, timestamps, duration, current/last tool, output tail, files read/written, token counts and cost where the backend supplies it. Do not manufacture precision for unavailable metrics. Current payload projection contains token/files/duration fields; current per-event cost availability was not proven.
- The global Agents panel includes running/queued children from every visible/known session and terminal rows from the active session. Its count and tree use the same derivation. A background child's progress cannot disappear when its parent's chat loses focus.
- `subagent.list` hydrates live records and durable failed-delegation rows after reload/reconnect; reconcile snapshots without overwriting newer stream events or terminal states. Live lineage can survive UI-session reminting/compression even when exact-generation control authority no longer applies.
- `subagent.roster` reports per-profile conversations with live children or background processes, including conversations not currently attached. Source refreshes on connect/focus and while visible, with backoff and missing-method handling.
- Per-child **Steer** queues text, **Stop** requests a hard interrupt, and **Transcript** reads a bounded live tail. “Queued” does not promise delivered steering: a child that already passed its final tool batch may miss it. Control/tail use exact transport/session-generation authority and fail closed rather than acting through the current ambient gateway.
- **Stop parent turn does not stop background children.** Accept terminal child-completion events after parent interruption so a finished child cannot remain stuck “running”. Keep active children across subsequent parent turns while retiring finished display rows at the correct boundary.
- Backend also exposes delegation pause/status and saved spawn-tree snapshots. Current React Agents page is a live tree, not proof that every saved-tree RPC already has a desktop control. Preserve backend capability and expose it in advanced history/controls if rebuilding that experience.

Evidence: [event types](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/session/hooks/use-message-stream/utils.ts:195), [wire fields and reasoning policy](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/tool_progress.py:461), [state merge](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/store/subagents.ts:211), [tree/panel scope](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/store/subagents.ts:422), [Agents presentation](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/agents/index.tsx:84), [reload snapshot](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/status-stack/use-subagent-snapshot.ts:13), [roster hydration](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/contrib/hooks/use-live-work-roster.ts:72), [controls](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/chat/composer/status-stack/subagent-controls.tsx:21), [post-interrupt terminal events](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/session/hooks/use-message-stream/gateway-event/tools.ts:223).

### Agent API and settings matrix

| Contract | Meaning / constraint | Evidence |
|---|---|---|
| `subagent.list {session_id}` | Live/lineage-visible children plus durable failed tasks; attached session required | [methods:29](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/methods_subagents.py:29) |
| `subagent.roster {profile?}` | App-wide owner-session counts for live children/background work; profile fence | [methods:121](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/methods_subagents.py:121) |
| `subagent.steer {session_id,subagent_id,text}` | Exact-owner queued/rejected receipt | [methods:2210](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/methods_session.py:2210) |
| `subagent.interrupt {session_id,subagent_id}` | Exact authorized live object; `{found}` receipt | [methods:163](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/methods_subagents.py:163) |
| `subagent.tail {session_id,subagent_id}` | Up to 16 KiB, available/text/truncated; exact-generation authority | [methods:187](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/methods_subagents.py:187) |
| `delegation.status`, `delegation.pause` | Active children, paused spawning, max depth/concurrency; pause future spawning | [methods:2195](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/methods_session.py:2195) |
| `spawn_tree.save/list/load` | Saved tree/history backend capabilities | [methods:2228](/Users/aldrinjoseph/.hermes/hermes-agent/tui_gateway/methods_session.py:2228) |
| `GET /api/delegation/routing` | Selected backend, availability/blocked reason, model/profile routes and host profiles | [API:48](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/api/delegation.ts:48) |
| `PUT /api/delegation/backend` | Default, plugin model-map/profile routes, or registered third-party backend | [API:66](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/api/delegation.ts:66) |
| `PUT /api/delegation/backends/model_map`, `/profile_routes` | Preserve per-route model/provider/base URL/reasoning/service tier/credential-preview/fallback config, or profile routes | [API:20](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/api/delegation.ts:20) |

Simplify **Model → Subagents** to “Use the main model” plus an Advanced routing section. Retain delegation model/provider overrides, iteration limit, concurrent-child limit, child timeout and reasoning effort. Keep the `advanced-delegation` plugin's model-map and profile-route modes, plugin enablement/status and unavailable saved routes. A missing/off routing plugin must display a repair path, not silently run the default backend. Non-default routing modes own model choice; do not present `delegation.model` as an effective override there. See [curated settings](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/constants.ts:715), [routing UI](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/delegation/routing-section.tsx:40), [routing availability/model ownership](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/app/settings/delegation/routing-data.ts:64).

For the redesigned workspace, the run rail should show actual child identities and statuses, with goal, parent/batch context and current activity. Selecting a child opens its tool/output/transcript detail with Steer and Stop when authorized. A global **Agents / Running work** view includes other sessions and background processes without navigating away automatically. Make stale/reconnecting/authority-unavailable explicit. Transcript text is supporting evidence, never the authority for “running” or “done”.

## Durable Kanban task parity

Kanban is an optional desktop plugin, **off by default**, which contributes `/kanban`, sidebar navigation, palette actions, a rebindable new-task shortcut and a live ready/running count. The custom Task window is a chat/repository viewer; its title does not supply this feature.

Required inventory:

- Multiple boards, board switching and pin/reorder, metadata/project/default-directory association, archived task view, board import/export and recoverable board archive. Selected board and caches are scoped by connection; selection must not mutate the server's global current-board pointer.
- Create/edit/delete and bulk task changes; title/body, priority, tenant, assignee; ready/triage/blocked/completed and system-owned running/review/scheduled states. Preserve dependency/parent-child links and completion counts. Do not offer drag/create actions into system-owned lanes.
- Card liveness includes current run time, worker PID/heartbeat and stale-worker state. Task detail includes result/summary, comments, events, related tasks, attachments, workspace/branch, runs, errors, model/provider/reasoning override, and bounded worker stdout/stderr.
- Typed blocking reason, repeated blocking count, failure count, diagnostics with actual recovery actions; reassign/reclaim and unblock behavior. Never paint every blocked task as the same vague error.
- Orchestrator/default-assignee selection, automatic decomposition, profile descriptions and per-task estimates (only on explicit request because an auxiliary model call is involved).
- Namespace-scoped REST, event cursor resumption, snapshot reconciliation, board/detail polling fallback and native completion notification. A board edit nudges the dispatcher so ready work can begin promptly.
- Runtime durable board/worker ownership remains separate from process-local delegated subagents. Gateway restart, stale worker reclaim, cross-profile delivery and board isolation remain backend-owned.

Evidence: [plugin surface/default](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/plugins/kanban/plugin.tsx:1), [board/connection authority](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/plugins/kanban/api.ts:1), [event cursor and invalidation](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/plugins/kanban/api.ts:104), [task/run/detail fields](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/plugins/kanban/types.ts:5), [locked states/liveness](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/plugins/kanban/ui.tsx:49), [orchestration](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/plugins/kanban/types.ts:218), [runtime board invariants](/Users/aldrinjoseph/.hermes/hermes-agent/cron/AGENTS.md:117).

API namespace: `/api/plugins/kanban`, normally `?board=<slug>`. Current desktop uses `/board`, `/tasks/{id}`, `/tasks/{id}/log`, `/boards`, `/profiles`, `/projects`, `/orchestration`; create/patch/delete tasks, `/tasks/bulk`, comments, reassign, reclaim, attachments; board create/edit/archive/import/export; estimates; dispatch; profile descriptions. Event socket is `/events?board=...&since=<cursor>` in that namespace. Source router also exposes `/links`, diagnostics, active workers, individual run inspect/terminate, home-channel subscription and stats capabilities. Preserve them when consolidating advanced task controls; distinguish backend capability from current React controls. See [desktop API reads/writes](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/plugins/kanban/api.ts:426), [backend task/link APIs](/Users/aldrinjoseph/.hermes/hermes-agent/plugins/kanban/dashboard/plugin_api.py:359), [run diagnostics/control](/Users/aldrinjoseph/.hermes/hermes-agent/plugins/kanban/dashboard/plugin_api.py:882), [delivery/subscriptions](/Users/aldrinjoseph/.hermes/hermes-agent/plugins/kanban/dashboard/plugin_api.py:1196).

The friendly destination is **Tasks**, with Board, Running and History views sharing one durable task identity. Task detail keeps Summary, Activity, Runs and Advanced controls. Keep the optional plugin enablement choice, offer a clear enable action when needed, and retain advanced orchestration and board transfer without cluttering the task-creation happy path.

## Confirmed native gaps and verification boundaries

1. **Binary WebSocket incompatibility is confirmed.** Original STT sends ArrayBuffer PCM; original TTS expects binary PCM. Rust [bridge.js:310](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/src/bridge.js:310) coerces every outbound message to `String(data)`; [commands.rs:222](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/src/commands.rs:222) accepts only `String`; [gateway_ws.rs:138](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/src/gateway_ws.rs:138) sends `Message::Text`; incoming binary becomes lossy UTF-8 at [gateway_ws.rs:99](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/src/gateway_ws.rs:99). Restoring a microphone icon or retaining the renderer cannot pass streaming voice parity. Preserve bytes, binary type, buffering, close/error and lifecycle semantics end to end.
2. **Native microphone/wake helpers are catalogued but absent from Rust implementation.** `hermes:requestMicrophoneAccess`, wake-indicator and stream-work channels appear in [channel inventory:206](/Users/aldrinjoseph/Developer/hermes-desktop-rs/crates/hermes-contract/src/channels.rs:206), [wake inventory:266](/Users/aldrinjoseph/Developer/hermes-desktop-rs/crates/hermes-contract/src/channels.rs:266) and [stream-work inventory:234](/Users/aldrinjoseph/Developer/hermes-desktop-rs/crates/hermes-contract/src/channels.rs:234). They are not exposed by the injected Rust bridge or handled by its dispatcher, which falls through to `unimplemented` at [commands.rs:886](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/src/commands.rs:886). Original macOS microphone access explicitly calls `askForMediaAccess` at [Electron main:15363](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/electron/main.ts:15363). Absence of the helper does not by itself prove browser capture fails; actual WebKit permission/capture remains **unknown**.
3. **Autoplay, WebRTC, recording codecs, AudioContext/AudioWorklet, background paint and microphone device lifecycle are unverified in the Rust WebKit host.** Original Electron deliberately supports wake-started audio without a preceding gesture ([window policy:34](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/electron/session-windows.ts:34)) and retains the hidden voice owner for streaming ([throttle:112](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/electron/stream-throttle.ts:112)). Tauri config currently declares no equivalent media entitlement/usage-description policy in [tauri.conf.json](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/tauri.conf.json:1). Packaging and OS media integration need explicit verification, not an inferred failure or success.
4. **Custom RPC requests discard push events.** [task_ops.rs:31](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/src/task_ops.rs:31) explicitly ignores gateway events, closes after one response, and the custom UI polls `session.resume`. This cannot replace subagent lifecycle, task events, live voice, approvals or background activity. Introduce a persistent owner-aware subscription contract while retaining snapshots as reconciliation.
5. **Current custom “subagent done” is not authoritative.** [task.js:609](/Users/aldrinjoseph/Developer/hermes-desktop-rs/ui/task.js:609) takes a goal from the first delegated task and finds any subsequent tool message in a small transcript window. It can associate an unrelated result and loses multi-task identity, nesting, real failure/cancel status, background work, owner and controls. Replace with native lifecycle and durable rosters; keep transcript-derived text only as a clearly labelled legacy display fallback.
6. **Current custom Scheduled rows are not job management.** [task.js:174](/Users/aldrinjoseph/Developer/hermes-desktop-rs/ui/task.js:174) takes `response.cron` from the sidebar sessions endpoint, then displays four rows. The original [cron store:5](/Users/aldrinjoseph/.hermes/hermes-agent/apps/desktop/src/store/cron.ts:5) explicitly distinguishes jobs from run sessions. The destination must use `/api/cron/jobs` and execution history.
7. **Owner routing needs its own pass.** Rust native grants are minted from shell home ([commands.rs:60](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/src/commands.rs:60)); `getConnectionFor` currently reaches the same connection branch ([commands.rs:428](/Users/aldrinjoseph/Developer/hermes-desktop-rs/src-tauri/src/commands.rs:428)). Do not claim multi-profile/remote voice, cron or child-control parity from common transport availability. Verify both launch-profile and secondary-profile scenarios against the canonical owner contract.

## Required phases and concrete acceptance cases

All phases are release requirements. Features remain recorded as unimplemented/unverified until their acceptance cases pass; an Advanced disclosure, Classic fallback or backend CLI is not evidence that the new user-facing flow is finished.

### Phase 1 — trustworthy owners, transport, and visible work

- **P1.1 Binary audio:** byte-for-byte PCM through local Rust socket transport in both directions; text control frames remain text; recording and playback streams stop without residual traffic after cancel/close.
- **P1.2 Owner isolation:** open equivalent jobs/chats on two profiles, with one a secondary served by the same gateway and another on a separate connection. Record, play, list/run/stop a job and inspect/steer a child. Every request and result stays with its owner through a profile/connection switch; stale completions cannot overwrite the new view.
- **P1.3 Lifecycle reconciliation:** spawn multiple nested children and a background process, switch chats, hide/reveal the pane, reload and reconnect. Live roster/count/tree agree; terminal children settle exactly once; failed durable delegation rows remain discoverable. Missing method, disconnected and empty are distinct states.
- **P1.4 Parent/child stop:** stop the parent while a background child continues; the child stays visible, receives a terminal event later, and never gets labelled done from an unrelated tool result. Individual Stop affects only the targeted child; rejected authority remains visible.
- **P1.5 Viewer ownership:** closing or switching a viewer does not stop the canonical gateway or its cron ticker. Reopening restores truthful live work rather than starting another owner.

### Phase 2 — complete voice experience

- **P2.1 Dictation:** permission granted/denied, missing device, silence, long clip, max-duration cap, cancellation and failed transcription each produce the right state/recovery. Partial transcript settles once into the intended editable draft. A failed live stream falls back to the saved take without losing first words.
- **P2.2 Speech:** direct-provider, streaming relay and whole-text fallback each speak; the first wake-started reply is audible. Stop and barge-in prevent late audio resurrection and duplicate playback across windows. Provider failure preserves the text answer.
- **P2.3 Chained calls:** several speech turns, narration plus final answer, silence re-arm, interjection during generation and during playback, custom/disabled stop phrases, wake pause/resume and ending the call all work without two capture owners contending for one microphone.
- **P2.4 Live calls:** selected GPT-Live engine negotiates WebRTC, shows captions, seeds current chat context, delegates tool work to the intended Hermes session and returns progress/results to the matching delegation. Engine changes apply next call; absent credentials/provider failures present repair guidance.
- **P2.5 Wake:** local and remote-client capture, another-surface ownership, unavailable provider, installation wait, profile-routed wake, dead track/stalled capture and reconnect do not leave a false listening indicator. Ending a conversation re-arms only when persistent config says enabled.
- **P2.6 Settings:** all original voice/STT/TTS/wake values, custom voices/models and unknown provider-specific fields survive opening, editing one field, saving, resetting a default and changing owner. Show pending/saved/failed honestly; do not persist provider keys in renderer storage/logs.

### Phase 3 — complete scheduling and execution history

- **P3.1 Create/edit round trip:** presets, interval, natural phrase, cron and ISO one-shot; finite/unlimited repeat; timezone across a daylight-saving boundary; inherit/default versus explicit zero grace and true/false/default catch-up. Human-readable preview and next occurrence agree with backend.
- **P3.2 Advanced preservation:** round-trip skills, model/provider/base URL, reasoning, toolsets, workdir, context chaining, pre-run script/interpreter, script-only mode, change-monitor mode, delivery/failure targets and session-attachment policy. Editing name or schedule does not erase any unedited authored field or scheduler bookkeeping.
- **P3.3 Execution controls:** repeated Run now, scheduled fire racing Run now, already-running refusal, Stop while triggering, run completing before Stop returns, pause while a run continues, and delete with a live run all settle against the intended execution ledger. No premature success toast.
- **P3.4 Delivery:** local, configured Telegram/other platform, explicit/multiple targets and missing home target retain their exact meaning. Results and failures reach configured destinations through the owning profile; a disconnected destination remains visible for repair.
- **P3.5 History:** successful, failed, cancelled, script-only/output-only, live, properly closed and orphaned runs render accurately. Open the correct remote/profile transcript. An unowned unclosed run is view-only and its verdict re-evaluates when authoritative ownership changes.
- **P3.6 Background continuity:** schedules remain scheduled across app close/reopen and gateway reconnect; changed events refresh the correct owner immediately, with bounded polling/recovery when event support is absent.

### Phase 4 — durable task boards and advanced delegation

- **P4.1 Plugin lifecycle:** Kanban off/on/unload/re-enable and unavailable backend have explicit states; navigation, shortcut, status count and event subscriptions appear/dispose once.
- **P4.2 Durable board:** create/edit/bulk/move/assign/link/unlink/comment/attach and archive/import/export round-trip on the intended board/connection. System-owned lanes reject invalid transitions; dependency completion and partial bulk failures remain truthful.
- **P4.3 Worker/run recovery:** ready → running → review/done, typed block, repeated failure, stale heartbeat, reclaim/reassign and run termination target actual backend authority. Detail exposes logs, attempts, diagnostics, workspace and dependencies without replacing durable state with transcript guesses.
- **P4.4 Resume events:** switch boards/connections, miss events, reconnect and resume from cursor. Snapshot and event stream reconcile; completion notifications do not replay old tasks; other boards' work does not leak into the selection.
- **P4.5 Delegation settings:** Default, model-map, profile routes and third-party routes retain advanced fields and owner scope. Disable/remove a selected routing plugin: show blocked state and repair; never silently substitute Default. Save ordering cannot undo newer edits.

These acceptance cases require controlled fixtures or an isolated test home plus explicit runtime verification during implementation. This planning pass did not execute them against user state.
