# Performance Measurement

This directory owns repeatable, non-normative measurements of user-visible compiler and editor
latency. Measurements guide optimization; they do not define language or tooling correctness.
Correctness remains owned by `spec/` and the compiler conformance suite.

## Scenarios

`run.js` measures three paths with released or candidate installed compilers:

- a process-cold check of the single-file `examples/hello.nct` program;
- a process-cold check of the representative `examples/text-search` package;
- a persistent LSP session that alternates a body-only edit in `examples/text-banner/banner.nct`
  and requests checked hover after each edit.

The editor request follows every change on the same ordered protocol stream. Its latency therefore
includes source publication, required invalidation and semantic work, and hover projection. The
source change preserves the declaration surface but changes a string constant in a function body.
This deliberately exercises incremental body analysis rather than a whitespace-only fast path.

On Darwin, process scenarios also use `/usr/bin/time -l` to record peak resident bytes. Other
hosts retain elapsed time and report resource measurements as unavailable.

## Running a Baseline or Comparison

Run an installed compiler from the repository root:

```sh
node development/benchmarks/run.js \
  --compiler v0.36.0=dist/.nocter/nocter \
  --output /tmp/nocter-v0.36.0-performance.json
```

Compare two valid installed homes by naming both compiler binaries:

```sh
node development/benchmarks/run.js \
  --compiler released=/path/to/released/.nocter/nocter \
  --compiler candidate=/path/to/candidate/.nocter/nocter \
  --output /tmp/nocter-performance-comparison.json
```

Each binary must remain inside its own valid installed home. The benchmark never bypasses compiler
or standard-library integrity checks. Samples alternate compiler order to reduce systematic drift.
Use `--samples`, `--warmups`, `--lsp-samples`, and `--lsp-edits` to change the explicitly recorded
sample counts.

Run measurements on an otherwise idle machine, preserve power and thermal conditions, and compare
the same scenarios in one invocation. Commit a result only as milestone evidence together with the
host description, compiler digests, sample configuration, repository revision, clean-worktree
state, and scenario-source digests emitted by the runner. Do not turn elapsed-time thresholds into
conformance tests.

## Two Independent Signals

Wall-clock measurements answer whether a user-visible operation became faster. The computation
kernel's execution and reuse counters answer whether a source change performs more semantic work.
Those counters are deterministic and belong in focused Rust tests beside the incremental contract.
They must not be reconstructed from timing output, and benchmark code must not reach into compiler
private state.

Do not add internal phase timers until an external scenario demonstrates a bottleneck that cannot
be attributed with existing query counters and ordinary profiling. If instrumentation becomes
necessary, expose it through one observation boundary; do not scatter benchmark policy through
semantic crates.

## Recorded Baselines

- [`v0.36.0-arm64-darwin.json`](baselines/v0.36.0-arm64-darwin.json) — Apple M1 released baseline
  for the v0.37.0 performance milestone
