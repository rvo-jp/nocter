# Performance Measurement

This directory owns repeatable, non-normative measurements of user-visible compiler latency,
editor latency, generated executable size, and native application runtime. Measurements guide
optimization; they do not define language, tooling, or standard-library correctness. Correctness
remains owned by `spec/`, checked standard-library contracts, and the compiler conformance suite.

The [measurement boundary](methodology.md) separates external latency evidence from deterministic
compiler query counters and defines the evidence required for a valid comparison.

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
host description, compiler and installed-home identities, sample configuration, repository
revision, clean-worktree state, and scenario-source digests emitted by the runner. Do not turn
elapsed-time thresholds into conformance tests. Schema 2 records the installed manifest digest and
its standard-library tree identity; the retained v0.36.0 baseline uses the earlier schema 1 and
remains a historical record.

## Native Product Measurements

`native-run.js` builds each workload once through every compared installed compiler, records the
complete executable size and SHA-256 identity, then alternates warm and measured executions. The
workload matrix uses existing public applications rather than benchmark-only compiler paths:

- `line-frequency` covers synchronous text, Map/Set, allocation, and filesystem work;
- `json-normalize` covers fallible JSON parsing and generation;
- `archive-inspect` covers bounded gzip and tar processing;
- `binary-record` covers asynchronous file I/O and portable codec validation;
- `subprocess-pipeline` covers child processes, async transfer, and timeout cancellation;
- `async-http` covers a complete loopback HTTP exchange; and
- `http-service` covers the bounded operational application, durable state, networking, and
  structured shutdown.

The runner creates deterministic line, JSON, and gzip/tar inputs outside the repository. Its result
records their byte lengths and digests together with exact source, compiler, installed-home, host,
executable, and repository identities. Every retained sample records its alternating round,
position, and elapsed duration rather than retaining only a summary. Workload output is validated
after timing; an invalid execution is not a performance sample.

```sh
node development/benchmarks/native-run.js \
  --compiler released=/path/to/released/.nocter/nocter \
  --compiler candidate=/path/to/candidate/.nocter/nocter \
  --output /tmp/nocter-native-performance.json
```

Use `--samples` and `--warmups` to change the recorded counts. Native results compare complete
compiler-produced executables; they do not time an IR interpreter, a compiler-private entry point,
or a benchmark-specific standard-library implementation.

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
