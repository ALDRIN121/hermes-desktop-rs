# Full Hermes implementation progress

Implementation authorized 15 November 2026. Working branch: `codex/full-hermes`. Generated UI images remain binding visual references; written design governs behavior.

## Current work

- Imported 5,066 tracked renderer/shared/workspace, build-policy and contract-fixture files from immutable upstream commit `3b421578588243f920886e6fe79a6faa87ea026e`, with per-file/archive SHA-256 provenance. No installed user state or uncommitted upstream changes copied.
- Reconciled the pinned pristine source against the shipped renderer: upstream's own desktop freshness algorithm produces exactly `a8b4866557bfd351e263ecab4ca9f915e072b5fc20dc5e2715198a1da8ad702a`, matching `ui/hermes-build.json`. The local redesign intentionally changes that hash; new builds record it and refuse source changes during a build.
- Added tested import and repeatable renderer build scripts. Source renderer production build succeeded; generated output is ignored in `renderer/dist`.
- Implemented nine settings navigation groups retaining existing route IDs, callbacks and nested plugin settings.
- Added Hermes Indigo theme for the Rust host, preserving original themes and explicit user choices.
- Fixed bridge listener ordering, late connection after close, close cleanup and binary view byte preservation. Rust IPC now transports binary frames without UTF-8 corruption and emits socket events to the calling window.
- Fixed renderer camel-case request aliases and launcher path duplication.
- Implemented the seven main navigation destinations, retaining plugin contributions and existing routes.
- Normal startup now opens the source-built unified desktop app. Legacy task/workspace previews are explicit tools; reopening an existing window restores it without reloading a plain launch. Forwarded task arguments use the forwarded invocation.
- Verified Settings and Voice & audio inside an isolated native application (`tauri://localhost`), after closing the temporary browser preview at the user's request. Native opening exposed a zoom event-name mismatch; the new regression test and rebuilt native app verify the fix.
- Added guided voice mode/provider setup with all existing conversation fields retained under Advanced voice options. Microphone checking uses the existing audio-level meter, stops on cancel/unmount/hide/timeout and releases late permission results. No real microphone/provider call has been exercised.
- Startup transcript capture is now opt-in through explicit self-test mode.
- Added run evidence and an optional inspector, tested using real Node test output (one pass, one fail). Missing exits, truncated output and background launch receipts do not produce invented success/counts. Owned process snapshots now settle background launches after the parent goes idle; recent terminal evidence survives ephemeral composer dismissal. Output tails establish exit status without inventing complete suite counts.
- Native profile resolution now uses the canonical private control protocol, retains the returned owner identity, and mints method-specific one-use grants. Foreign connections and conflicting selectors fail before any default-home access. Remote registry connections remain unported.
- Pinned-source provenance is enforced rather than asserted: `import_renderer.py --check` now accepts only recorded local edits, fails on an unrecorded change, fails on a deleted recorded file, and fails on any file that sits beside the snapshot without being recorded. `--record-local-changes` freezes the 13 modified and 12 added renderer files explicitly. New `--route <path>` opens one in-app screen in the real window for review; it is validated so only a hash route can be set.

## Verification so far

- Baseline Rust workspace: 11 contract + 21 shell tests passed with local-socket permission. Sandbox-only run cannot bind the Unix socket fixture.
- Import/provenance tests: 7 passed, including recorded-change acceptance, unrecorded-change refusal, stray-file refusal and deletion refusal. Live snapshot: 5,054 pinned files verified, 13 recorded modifications, 12 recorded additions.
- Native route flag: `launch::tests` RED then GREEN 2/2; Rust workspace now 21 contract + 30 shell tests passed.
- Renderer lint: 0 errors across all 24 touched files (6 warnings in the same classes pristine upstream test files already emit: `no-restricted-globals` for `document` and one `exhaustive-deps` on a cleanup that reads a ref on purpose). Before this pass the same files held 56 errors.
- Focused renderer suite after the lint rewrite: 10 files, 52 tests passed. TypeScript check passed with an 8 GiB Node heap.
- Source renderer production build passed; Rust host rebuilt; isolated `Hermes UI Fixture.app` rebuilt and launched twice (plain, then `--route`). Settings → Voice & audio renders in the native window: nine settings categories, guided voice setup, mode cards, microphone check, provider rows.
- New import tests: RED then GREEN 4/4.
- New bridge tests: RED then GREEN 6/6, including native zoom events and no automatic transcript-capture timers on ordinary launches.
- New native socket tests: RED then GREEN 2/2. Rust suite after socket changes: 11 contract + 23 shell passed.
- Focused original settings/model/plugin tests plus new navigation tests: 46 passed.
- New theme/navigation tests: RED then GREEN 5/5.
- Renderer TypeScript check passed with an 8 GiB Node heap after fixing a readonly test-fixture mutation. Earlier default-heap attempts did not pass.
- Rust suite after scoped routing: 20 contract + 25 shell tests passed, including a real fake-server REST request with the selected profile's private grant.
- Navigation/session-actions: 166 passed. Evidence/inspector: 8 passed.
- Broad renderer suite: 1,606 files passed; five files failed (missing pinned fixture/backend definitions and an older inspector test). The missing files were added from the same pinned revision; the session-actions rerun passed, four remaining file reruns passed 27 tests, and the canonical socket test passed separately with local-socket permission. A single final broad run remains pending.
- Source renderer production build passed after navigation/inspector changes (5.21s warm build). Visual review uses a read-only fake host, not the user's Hermes state.

## Important limits

This is implementation in progress, not full feature parity. The rebuilt renderer is now the development application's default UI; the installed user app has not been replaced. No live voice, scheduler, account-linking, model/provider mutation, social publication or PR action has been exercised. Native remote/auth/plugin/window integration and complete replay/gap/report reconciliation remain unfinished. The redesigned destinations dock beside the measured navigation rail. Full visual acceptance, the All settings catalogue and guided voice hardware/provider acceptance are still pending.

Two review-environment artifacts are recorded in `NATIVE_UI_REVIEW.md` instead of being called fixed: the DOM carries an `opacity: 0` "Waking up …" overlay (invisible, not a boot failure), and the backend pill reads amber "Backend checking" even though the fixture answers `setup.status`/`setup.runtime_check` — the readiness store still ends up null and the trigger is unresolved.

## Decisions

- Reused current checkout on a new branch because the review packet was uncommitted here; relocation remains possible.
- Executing integration inline, with test-first checks and later independent review.
- Kept an immutable source snapshot and provenance instead of importing live installed state.
- Reused a filesystem clone of existing Node dependencies after offline installation lacked registry metadata. Source lockfiles match; clean-install reproducibility remains to verify.

Detailed execution/recovery ledger: `.superpowers/sdd/2026-11-15-full-hermes-plan/progress.md` (ignored scratch). No merge or push requested/performed.
