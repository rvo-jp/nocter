# Nocter Development Handoff

## Current State

Nocter v0.36.0 is published and externally audited. v0.37.0 Phase 0 through Phase 3 are complete,
and Phase 4 is closed as unnecessary. Incremental relation reuse reduces measured body-edit latency
by 24.5 percent and the fifty-edit session by 24.7 percent while keeping major cold-path latency
regressions below five percent.

## Next Work

Continue v0.37.0 Phase 5 qualification from the completed compiler and verification-throughput
gates: run documentation, packaging, installed-home, and final design-review gates; compare the
final candidate with v0.36.0; and record the exact release-ready result without changing public
release identity.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
