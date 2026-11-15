# Performance baseline and budgets

Measured on this machine with `scripts/measure-startup.sh`, against the bundled
fixture app (debug profile, isolated `HERMES_HOME`, a local fixture gateway). The
script stamps WebKit document-load completion inside the shell
(`on_page_load` → Finished). This is not a measured compositor paint or proof
that React has rendered and the application is usable. The historical JSON
field `firstPaintMs` names this load timestamp. `settledMs` is sampled after a
fixed four-second wait following that timestamp, not a readiness assertion.

## Baseline (3 cold runs, route `/`)

| Run | Document loaded | Post-load sample | RSS (shell + WebKit helpers) |
|-----|-------------|---------|------------------------------|
| 1 | 2,124 ms | 7,340 ms | 91.1 MB |
| 2 | 979 ms | 5,196 ms | 99.8 MB |
| 3 | 1,441 ms | 5,731 ms | 116.5 MB |
| **Median** | **1,441 ms** | 5,731 ms | 99.8 MB |

Raw output: `perf-baseline.json` (regenerate by running the script).

## Budgets

These are ceilings for this debug build; a release build should sit under them
comfortably. Exceeding one is a finding to investigate, not a rounding error.

| Metric | Budget | Basis |
|--------|--------|-------|
| Document-load timestamp | **2,500 ms** | Worst measured run (2,124 ms) plus headroom for a cold disk cache |
| Post-load sampling timestamp | **9,000 ms** | Worst measured sample (7,340 ms) + headroom; does not prove usability |
| RSS after settle | **200 MB** | Worst measured (116.5 MB); leaves room for a long transcript before the budget binds |

## What these numbers do *not* cover

- **Release profile.** Only the debug build was measured. A release build is
  unmeasured; no comparative speed claim is made.
- **First rendered frame and actual interactive readiness.** These need
  renderer-owned markers and native observation. The existing event and fixed
  wait do not establish them.
- **Long transcripts, streaming events, repeated route switches, memory after
  reconnect cycles.** The plan asks for these; only cold start is measured.
- **Router-level streaming throughput and audio transport cost.** Not
  instrumented.

The honest summary: cold start has real numbers and a budget. The rest of the
performance phase is unmeasured, and is listed as such in
`NATIVE_UI_REVIEW.md` rather than assumed.
