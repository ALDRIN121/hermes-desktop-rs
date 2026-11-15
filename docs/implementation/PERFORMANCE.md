# Performance baseline and budgets

Measured on this machine with `scripts/measure-startup.sh`, against the bundled
fixture app (debug profile, isolated `HERMES_HOME`, a local fixture gateway). The
script stamps first paint from inside the shell (`on_page_load` → Finished), so
the number includes process start, WebKit boot, bundle parse and first layout —
not just the window appearing.

## Baseline (3 cold runs, route `/`)

| Run | First paint | Settled | RSS (shell + WebKit helpers) |
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
| First paint | **2,500 ms** | Worst measured run (2,124 ms) plus headroom for a cold disk cache |
| Settled (window usable, first data painted) | **9,000 ms** | Worst measured run (7,340 ms) + headroom |
| RSS after settle | **200 MB** | Worst measured (116.5 MB); leaves room for a long transcript before the budget binds |

## What these numbers do *not* cover

- **Release profile.** Only the debug build was measured. A release build is
  expected to be faster; it has not been measured, so no claim is made.
- **Long transcripts, streaming events, repeated route switches, memory after
  reconnect cycles.** The plan asks for these; only cold start is measured.
- **Router-level streaming throughput and audio transport cost.** Not
  instrumented.

The honest summary: cold start has real numbers and a budget. The rest of the
performance phase is unmeasured, and is listed as such in
`NATIVE_UI_REVIEW.md` rather than assumed.
