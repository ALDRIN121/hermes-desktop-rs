# Native application review — 15 November 2026

The UI is packaged in the Rust/Tauri desktop application. Product startup opens the unified Hermes window; it does not launch Chrome or serve the UI in a browser. The earlier browser visual-test tab was closed after the user clarified this requirement. Further visual acceptance uses the desktop app.

## Observed in the application

- Built renderer and Rust executable; assembled an isolated `Hermes UI Fixture.app` under `target/fixtures`, without replacing the installed user application.
- Connected over the actual Rust control-socket / REST / WebSocket adapter to a fake local gateway with a new temporary Hermes home. No real provider, account, schedule or session mutation.
- Observed the seven main destinations and indigo theme in the native WebKit window.
- Opened Settings with the application shortcut, then Voice & audio. The native accessibility tree identified `tauri://localhost#/settings?tab=config%3Avoice&page=conversation`.
- Observed all nine settings categories, docked beside the global navigation; conversation/transcription/speech tabs, profile selector, local gateway label and existing voice controls were present.
- The first native settings opening exposed a missing `zoom.onChanged` bridge member. Added a failing bridge test, corrected the event contract and native event emission, rebuilt and verified Settings opens successfully.
- Disabled automatic transcript capture on ordinary launches; detailed startup diagnostics require explicit self-test mode.

## Second native pass — guided voice screen (`--route`)

Synthetic key input to the app window was refused by the approval gate, so the pass used the new launch flag instead of driving the UI:

```
hermes-desktop-rust --route '/settings?tab=config%3Avoice&page=conversation'
```

`--route` is validated (`launch::route`): a leading `/`, no `//`, no whitespace/quotes, ≤512 chars, ASCII path/query characters only, so it can only ever set an in-app hash route. It is applied to the built window with `location.hash`, and to an already-open window on a forwarded invocation.

Observed in the native window at 1360×900 (`computer_use` capture, dark theme):

- Global rail: SESSIONS/BOTS tabs, Hermes wordmark, Chat · Tasks & agents · Automations · Connections · Skills & tools · Library · Settings, "No sessions yet", "+ New project".
- Settings rail, all nine categories, "Find any setting…" search, Voice & audio expanded and selected.
- Detail pane: "Voice & audio — Talk to Hermes and choose how it listens and speaks.", Dictation/Voice conversation/Spoken replies, "Applies to · default · Local gateway", Standard/Live conversation mode cards (Standard selected, purple border), the **Get voice ready** stepper (1 Microphone · 2 Speech providers · 3 Try voice in Chat), the microphone row with "Not tested" + "Test microphone…", and the speech-to-text provider row below the fold.
- Conversation-mode cards are aligned and text is legible; the segmented tab row clusters its three pills to the left of a full-width container (upstream styling, not introduced here); the stepper's third item sits slightly right of an even rhythm; the bottom provider row is cut by the fold and is reachable by scrolling.

### Review-environment artifacts (not product defects, not yet fully isolated)

- **"Waking up …" is invisible.** `ChatSwapOverlay` renders that string with `opacity: 0` when no swap is pending (measured `opacity: "0"` in the DOM). It appears in `innerText` and in screenshots described from text, so it reads as a stuck boot overlay when it is not painted. Do not chase it as a boot failure.
- **Backend pill reads amber "Backend checking".** `runtimeReadinessDisplay(null)` is `'checking'`, so the readiness state was empty at capture time. The fixture *does* answer `setup.status` (`provider_configured: true`) and `setup.runtime_check` (`ok: true`) over the socket — both appear in its RPC log — yet the store still ends up null. The read-only fixture refuses `pet.info`, `billing.state`, `subscription.state`, `wake.status`, `onboarding.state`, `free_tier.status`, `mcp.servers.status`, `commands.catalog` and `client.capabilities`, and repeats `pet.info` about ten times a second. The remaining trigger is **unresolved**; it may be fixture-only or an upstream status-path issue. It is recorded here rather than claimed fixed.

## Still required

### Concept fidelity — Voice & audio vs `04-full-hermes-settings-v1.png`

Matches the generated concept: three-column layout with the global rail, a settings rail docked beside it, and the detail pane; the left-rail destinations and their order (Chat · Tasks & agents · Automations · Connections · Skills & tools · Library · Settings); search under the "Settings" heading with the "Find any setting…" placeholder; the nine categories in the concept's exact order with Voice & audio selected; page title → description → three sub-tabs; the Standard / Live conversation radio cards with Standard selected; the "Get voice ready" 1–2–3 stepper; a microphone row with "Not tested" + "Test microphone…"; provider rows; a collapsible "Advanced voice options" panel.

Deltas still open (structure is right, detail is not yet the concept):

- The microphone state badge, the "Setup needed" amber banner and the row icons are now present (badge amber until audio is detected, banner only while a speech provider is unset). Still missing: the concept's "Save changes" footer — settings apply on edit, so a disabled Save button would be a lie.
- The scope control reads "Applies to · default · Local gateway"; the concept labels it "Personal profile · Local gateway".
- The concept's step 3 now matches ("Test conversation"). Conversation-mode cards still sit full width instead of beside the section heading, and the segmented sub-tab row leaves empty space to the right of its three pills.

The guided voice setup is integrated but has not been exercised against a real microphone or provider. The All settings catalogue, complete native connector/auth/media/window parity, full run replay/gap/report handling, compact-window and keyboard acceptance and performance baselines remain incomplete. This is a native implementation checkpoint, not a full-product acceptance claim.
