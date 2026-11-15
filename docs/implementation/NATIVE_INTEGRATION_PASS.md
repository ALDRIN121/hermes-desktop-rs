# Native integration acceptance pass

15 November 2026. Continuation from `f2a3948` on `codex/full-hermes`.
The agreed renderer remains inside the Rust/Tauri application. This is an
implementation and acceptance record, not full product completion.

## Implemented

- Native peer and single-session windows, with the source renderer's query
  parameters before the hash, encoded session routes, profile ownership,
  spectator mode and separate window permissions. Explicit active routes win
  over background profile reads. Unsupported remote routes fail instead of
  opening a window against a previous local owner.
- Cross-window relay and preference-change subscriptions with unsubscribe.
- Socket send/close ownership and window-destruction cleanup, including a
  pending handshake. A delayed-handshake regression verifies cancellation,
  server EOF, empty registry and refusal of late insertion. Independent review
  identified this race and reviewed its correction without further findings.
- MCP loopback OAuth callbacks: owner-bound listen/wait/cancel, bounded pending
  listeners, timeout, cleanup, browser-noise filtering and issuer forwarding.
  The gateway still verifies OAuth state and exchanges tokens.
- Plugin socket grants: namespace validation, one use, exact URL/window
  binding, expiry and private Rust header injection. Prepared URLs contain no
  grant. Socket diagnostic histories are bounded and ticket protocols masked.
- Preview watches now emit real, owner-routed change events. Polling detects
  content changes and file replacement; explicit stop and window destruction
  release the watch. The old implementation returned a stamp without watching.
- Durable prepared-submission recovery with private atomic writes and shared
  compare-and-set. The previous read returned an object where the renderer
  requires JSON text, causing a startup “Restore image draft” error. Invalid
  JSON/UTF-8 journals are quarantined with their bytes preserved.
- Dropped files now stage real bytes in a private native directory. Invalid
  names and files over 16 MiB fail explicitly. The Rust-specific staging and
  file-stream URL channels are host extensions, not invented upstream channels.
- Gateway media carries the selected profile and clip session, uses the actual
  `native-http` grant purpose, forwards byte ranges and preserves 206/416
  metadata. Response reads are bounded. A fake HTTP/control server verifies
  grant purpose, header, session and range. Downloads use the same grant and
  retain their session/profile selectors.
- All settings keeps literal dots in provider/connector keys, rejects non-finite
  numbers and JSON type changes, handles nullable JSON and masks credential
  fields. Credential values do not match search. Existing read-owner capture,
  delayed-save cache isolation and scope-switch draft reset remain covered.

## Verification

- Renderer settings: 82 files, 631 tests passed.
- TypeScript: passed with the documented 8 GiB Node heap.
- ESLint: modified settings files passed; source import check and its seven
  tests passed. Production renderer build passed (4.47 seconds warm build).
- Native regression: 22 contract + 68 shell tests passed. Bridge: 15/15.
  These are scoped checks, not a whole-tree green claim.
- Native fixture app: All settings renders inside the application; Cmd+Shift+N
  opens another complete Hermes window with the expected profile. The rebuilt
  primary and peer show no draft-recovery error. Typing in the peer and closing
  it leaves the primary draft unchanged. No prompt was submitted.
- Native WebKit media: explicit debug-only fixture panel, generated silence
  (PCM16 stereo, 48 kHz, 24 seconds, 4,608,044 bytes), muted playback at 4×.
  The native UI reached “PASS: 24.0 seconds”. The fake gateway recorded
  continuous ranges from bytes 44 through 4,608,043 with `session=media-fixture`,
  including requests across the 2 MiB boundary. The separate 100 MiB integration
  case verifies capping an open-ended upstream range to 2 MiB. Real remote
  gateways and other codecs remain unverified.
- The UI automation connection initially expired while selecting this fixture;
  resetting the UI session recovered it. The media app and its gateway were
  closed after acceptance. The ordinary native review fixture remains open.
- Legacy self-test hooks now require a debug build and validated isolated test
  instance; their filesystem paths use that fixture home and clipboard contents
  are omitted from diagnostics.
- The installed user app was not replaced. Fixture homes and bundles are
  isolated under the OS temporary directory and `target/fixtures`.

## Test side effect corrected

An older external-link test called macOS `open https://example.com/`, launching
Chrome during Rust regression runs. The test now calls only the pure URL
validator. The accidental Example Domain tab was closed through the UI.

An older offline-media test assumed no gateway was running and could resolve
the user's default home. It now owns an empty temporary home. The session/media
success test uses private fake control and HTTP servers.

## Remaining release blockers

The earlier 16-member absence list is reduced, but eight of those members still
need implementation: connection configuration apply/save/test, connection
OAuth login/logout, preview capture, external-open failure events and profile
preferences. That list is narrower than the entire native channel inventory;
other native families also remain in the full plan.

Real microphone/codecs/provider conversation, account OAuth/Telegram pairing,
remote registry/SSH/cloud, vault, embedded-browser capture/pop-outs, the complete
scheduler/delivery and agent/delegation workflows, full settings-inventory and
schema/revision acceptance, run replay/gap acceptance, release performance and
the complete renderer test tree remain open. Hardware/service checks require
an appropriate isolated live test setup. No full-parity, all-Rust or optimized
performance claim is made.
