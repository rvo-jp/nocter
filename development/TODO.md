# Nocter v0.3.0 Handoff

The completion criteria and implementation boundaries are owned by the
[v0.3.0 Development Contract](docs/v0.3.0.md) and
[First-Class Outcome Values](docs/outcome-values.md). Git owns
chronological implementation history.

## Current Baseline

- branch: `develop`
- released baseline: `v0.2.0`
- completed milestone gates: `v0.3.0 Phase 0`, `v0.3.0 Phase 1 Typed Literal Core`,
  `v0.3.0 Phase 2 Explicit Iteration and Collection Access`, and
  `v0.3.0 Phase 3 Owned String Interpolation and Formatting`,
  `v0.3.0 Phase 4 Public Provenance Contracts and Generic Interface Bounds`, and
  `v0.3.0 Phase 5 Nested Outcomes and Executable Process Context`, and
  `v0.3.0 Phase 6 First-Class Outcome Values`, and
  `v0.3.0 Phase 7 Protocol-Driven Collection Iteration`, and
  `v0.3.0 Phase 8 Explicit Sequence Spread and Composable Element Packs`, and
  `v0.3.0 Phase 9 Composable Iterators and Collection Builders`, and
  `v0.3.0 Phase 10 Callable Values and Interface Default Methods`
- active milestone gate: `v0.3.0 Stabilization`
- target: `arm64-darwin`
- required Phase 0 items: none
- required Phase 1 items: none
- required Phase 2 items: none
- required Phase 3 items: none
- required Phase 4 items: none
- required Phase 5 items: none
- required Phase 6 items: none
- required Phase 7 items: none
- required Phase 8 items: none
- required Phase 9 items: none
- required Phase 10 items: none

Phase 9 added capability sets, conditional conformances, statically specialized generic iteration,
allocation-transparent standard adapters, unknown/exact-size vector builders, stored optional
ownership support, and capability-set LSP presentation and recovery.

Phase 10 added method-level generics, interface default methods, explicit-capture closure values,
static callable specialization, lazy callback-driven iterator defaults, recursive nested cleanup,
consuming-receiver ownership transfer, provenance and allocation propagation, and compiler-backed
editor integration.

## Current Objective

Qualify v0.3.0 as one stable release without expanding its feature set. Optional and fallible
outcome identity is now preserved through IR, including stored borrow payloads and contextual
generic specialization. User-facing compiler diagnostics no longer describe implementation limits
as a bare `v0` contract, and call-lowering failures distinguish unavailable targets from borrow and
scalar materialization failures.

The ownership, allocation, and region cleanup audit now enforces allocator restoration before
outer-owner destruction, reverse region release after destruction, and non-unwinding `never`
termination independently of tail-call eligibility. Native and packaged-home probes cover normal,
`return`, `break`, `continue`, propagation, recovery, and immediate termination edges.

The editor identity audit now gives destructor and generic-parameter declarations explicit name
spans instead of reconstructing editor ranges from larger syntax spans. Resolver diagnostics,
typecheck facts, specialization lookup, hover, semantic tokens, document symbols, and IR indexing
share those declaration identities. Sequence-spread hover likewise carries the parsed operator span
through typecheck facts instead of deriving three bytes from an expression span. Paired analysis and
JSON-RPC tests cover destructor keyword/receiver separation and protocol selection ranges.

The active audit is malformed-input resilience and recovery across parser, analysis, and LSP
request boundaries. Subsequent loops cover packaging qualification and responsibility hotspots
against the stabilization gate.
