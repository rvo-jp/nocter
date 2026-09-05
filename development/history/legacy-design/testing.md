# Test Ownership and Integration Boundaries

Tests are owned by the narrowest layer that can prove a contract. Higher layers retain a small
representative path through lower layers; they do not repeat the lower layer's complete matrix.
This rule keeps failures local and prevents process-heavy integration suites from growing with
every semantic case.

## Layer Ownership

| Layer | Owns | Does not own |
|---|---|---|
| parser and formatter units | grammar shape, recovery, exact spans, canonical source | native execution |
| resolver and typecheck units | identity, visibility, inference, ownership, diagnostics | backend instruction shape |
| IR and backend units | specialization handoff, evaluation order, drop transitions, ABI and encoding | packaged standard-library policy |
| CLI build | artifact paths, target selection, emission failures, executable format | semantic cases already executed by CLI run |
| CLI run | representative native behavior with minimal test surfaces | exhaustive standard-library API coverage |
| distributed home | installed-home discovery, package visibility, canonical `std` source, and representative public API behavior | compiler-only lowering permutations |
| analysis units | semantic facts, ranges, completion, hover, and presentation | JSON-RPC framing |
| framed LSP | lifecycle, transport, UTF-16 conversion, and a representative semantic result | one process per compiler feature |
| public examples and corpus | shipped examples and accepted source inventory | internal diagnostics fixtures |

## Redundancy Rules

- Do not keep a CLI build test when an identical source has a successful CLI run test. `run`
  already performs native compilation. Keep separate build tests only for build-specific artifact,
  output-path, target, or failure behavior.
- Do not keep a check-only test when the same declarations and calls are compiled by a stronger
  run test, unless the check test asserts a distinct diagnostic or remains a deliberate
  cross-target contract.
- A standard-library feature gets exhaustive semantic and lowering tests at the owning compiler
  layer and one representative distributed-home path. Additional distributed tests must prove a
  package, visibility, canonical-source, allocator, system-call, or public-API property unavailable
  to a minimal compiler fixture.
- Test all private names from one module in one recovering source when diagnostics remain
  independently identifiable. Do not launch the compiler once per private symbol.
- Send multiple framed LSP requests through one initialized process when they share the same
  installed home and package snapshot.
- Prefer one Nocter contract program with named helper functions and distinct exit codes over one
  Rust integration test per successful API call. Keep failure-atomic and process-aborting cases
  isolated when combining them would hide state or terminate the remaining checks.

## Adding Coverage

Before adding an integration test, identify the invariant and its owning layer. Search for an
existing fixture that can be extended. A new semantic branch normally adds a unit test and extends
one representative integration program; it does not add parallel check, build, run, distributed,
and LSP cases.

Removed syntax, diagnostic recovery, ambiguous selection, ownership rejection, and destructive
failure paths remain focused tests because combining them weakens source ranges or state
observability. Successful API calls and public-surface checks are the primary consolidation
candidates.

## Verification Profiles

Run focused owning-layer tests during implementation. Before a coherent compiler commit, run the
complete verification script. Release qualification retains the complete matrix, but its size is
not a milestone metric: test count may decrease when a stronger test subsumes weaker coverage.
Record behavior and boundary coverage, not a requirement that historical test totals only grow.

When the complete suite regresses materially, report time by integration binary. A small test
binary that dominates wall-clock time is a boundary-ownership problem; adding parallelism or
ignoring the tests does not resolve it.

## Compiler Performance Measurements

Integration tests run the compiler from the Cargo test profile. That profile uses optimization
level 1 because the suite exercises the compiler binary hundreds of times; test-profile debug
assertions remain enabled. Do not restore an unoptimized compiler merely to make test and
development profiles identical.

Use the process-cold check benchmark to investigate compiler cost independently from the Rust test
harness:

```sh
development/compiler/scripts/benchmark-check.sh
```

The script builds the release compiler, checks `examples/hello.nct` in three separate processes,
prints internal JSON timing events, and reports the median total. Pass another source as the first
argument. `NOCTER_BENCHMARK_RUNS` changes the sample count and
`NOCTER_BENCHMARK_PROFILE=dev` selects the development compiler.

`NOCTER_INTERNAL_TIMINGS=1` is an internal instrumentation boundary used by this script. It emits
source loading, compile-unit loading, resolution, opaque-result elaboration, typecheck/index,
buildability, backend, program execution, and total events. Normal compiler invocations perform no
clock reads and emit no timing output. Do not turn elapsed time into a correctness assertion;
compare medians on the same machine and use phase events to choose an implementation target.

Set `NOCTER_INTERNAL_TIMINGS=2` when a top-level phase needs investigation. Level 2 adds resolver
subphase events while retaining all level 1 events. This is developer instrumentation, not a stable
CLI or JSON schema.
