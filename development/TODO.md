# Nocter v0.3.0 Handoff

The completion criteria and implementation boundaries are owned by the
[v0.3.0 Development Contract](docs/v0.3.0.md) and
[Nested Outcomes and Executable Process Context](docs/outcomes-process-context.md). Git owns
chronological implementation history.

## Current Baseline

- branch: `develop`
- released baseline: `v0.2.0`
- completed milestone gates: `v0.3.0 Phase 0`, `v0.3.0 Phase 1 Typed Literal Core`,
  `v0.3.0 Phase 2 Explicit Iteration and Collection Access`, and
  `v0.3.0 Phase 3 Owned String Interpolation and Formatting`,
  `v0.3.0 Phase 4 Public Provenance Contracts and Generic Interface Bounds`, and
  `v0.3.0 Phase 5 Nested Outcomes and Executable Process Context`
- active milestone gate: none
- target: `arm64-darwin`
- required Phase 0 items: none
- required Phase 1 items: none
- required Phase 2 items: none
- required Phase 3 items: none
- required Phase 4 items: none
- required Phase 5 items: none

Phase 5 added structural composed-outcome lowering, independent failure and absence control-flow,
callee-saved `argc`/`argv`/`envp` process context, executable UTF-8-checked `args` and `env`, ambient
`cwd`, recoverable `try_cwd`, and compiler-owned LSP presentation of nested return/provenance facts.

## Current Objective

Phase 5 is complete. Stop at this boundary; define any later v0.3.0 phase in the development
contract before implementation begins.
