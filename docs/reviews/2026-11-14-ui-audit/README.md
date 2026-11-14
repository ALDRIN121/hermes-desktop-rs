# Hermes full-feature review packet

**Updated 15 November 2026. Ready for review; no implementation has started.**

Your requirement is now the full Hermes experience with simpler configuration, not a limited coding UI. Voice, scheduled-job management, Telegram and other messaging/connected services, skills/plugins/MCP, models/providers, persistent agents, live subagents and task boards are all part of the phased destination. Original feature families are not silently removed because the Rust bridge is incomplete.

Start here:

- [Current full-product design](../../superpowers/specs/2026-11-15-full-hermes-design.md)
- [Current phased implementation plan](../../superpowers/plans/2026-11-15-full-hermes-plan.md)
- [New Settings and refined automation images](FULL_HERMES_VISUALS.md)
- [Dynamic run/test feasibility, models and Rust architecture](FEASIBILITY_AND_ARCHITECTURE.md)
- [Settings audit](SETTINGS_AUDIT.md) and [1,055-row source inventory](settings-inventory.tsv)
- [Voice, automations, live agents and Boards](VOICE_AUTOMATION_AGENTS.md)
- [Messaging, connectors, skills, plugins and MCP](CONNECTIONS_SKILLS_PARITY.md)

The settings inventory includes **918 finite schema keys**, **53 hand-built settings rows**, **78 dynamic families**, and **6 extra control families**. These are coverage counts, not proof every setting already works. The dynamic service/plugin catalog remains extensible.

The native shell is Rust; the UI is web technology, and the existing Hermes runtime is Python. The recommended architecture reuses the original renderer with a Rust native adapter. Rust/WASM or a native Rust GUI are feasible alternatives requiring explicit review; language percentages are not optimization evidence.

A real temporary test command with one pass and one failure exposed that the current UI parser reports zero results and can also count user prose as success. The new inspector must derive state from execution IDs/events/results and supported per-run reports, with honest unknown states. This diagnostic was run; the future dynamic UI has not been implemented or verified.

The new images use fictional content and are illustrative. Settings and automation setup were reviewed, including a rerender correcting paused scheduling and multi-target delivery. No microphone capture, real schedule, account link, provider switch or social message was initiated.

Original evidence remains available: [14 November audit](REVIEW.md), [123-row custom UI ledger](FEATURE_LEDGER.md), [281-channel inventory](CHANNEL_COVERAGE.md), [earlier composition studies](VISUAL_PROPOSALS.md). The limited-scope v1 design/plan are historical correctness inputs; their feature exclusions and vanilla-only architecture are superseded by the new proposal.

Verification today covers doc links/counts/assets, unchanged product source and the real test-parser diagnostic. The earlier 21/21 shell tests and 10-passed/1-sandbox-blocked contract result are dated 14 November and were not rerun as evidence for an unimplemented redesign.

Please review the full scope, simpler settings structure, new images, architecture options and phased plan before any implementation begins.

