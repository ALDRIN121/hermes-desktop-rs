# Source-built Hermes renderer

The editable renderer is imported from upstream commit `3b421578588243f920886e6fe79a6faa87ea026e`. It retains the original React feature modules and generated gateway contracts. The Python gateway remains external; this tree does not contain user settings, credentials or sessions.

`upstream/UPSTREAM_SNAPSHOT.json` records each imported file's original hash, selected archive paths and archive hash. Local implementation changes intentionally differ from that original snapshot; they are reviewable in Git. Import never overwrites an existing destination.

The pristine snapshot's desktop source hash is `a8b4866557bfd351e263ecab4ca9f915e072b5fc20dc5e2715198a1da8ad702a`, computed using the pinned upstream freshness algorithm. It matches the original shipped renderer receipt in `ui/hermes-build.json`. This reconciles the source baseline; local redesign builds have their own source hash in `renderer/dist/renderer-build.json`.

## Build

Use Node 26 or newer and the exact pnpm version in `upstream/pm/lock.json` (currently 12.15.0). From `renderer/upstream`, install dependencies with `pnpm install --frozen-lockfile --filter hermes...`. Do not run npm install. The upstream workspace preserves its package policies and lockfile.

From the repository root, run `node scripts/build_renderer.mjs`. This builds `renderer/dist`, including the optional shell-owned Task and Workspace pages. Generated output and node_modules are ignored. No Electron main process is used by the Rust shell.

For this initial local build, offline pnpm installation lacked cached registry metadata. A filesystem clone of the existing local dependency installation was used without modifying that installation; the source lockfile hashes match. This is local build evidence, not yet proof of a clean dependency install in CI.

## Tests

- `python3 -m unittest discover -s tests -p 'test_*.py'`
- `node --test tests/ui/*.test.mjs`
- In `upstream/apps/desktop`: `pnpm exec vitest run --project ui`
- `cargo test --workspace --offline` (requires permission to bind local test sockets)

The nine settings groups preserve existing page IDs, callbacks and nested plugin settings. The Hermes Indigo theme is the default only for the Rust host; explicit existing theme choices remain available.

Generated review images remain the visual target. Source import/build and native transport fixes do not establish complete feature parity or final visual acceptance; progress is recorded in `docs/implementation/PROGRESS.md`.
