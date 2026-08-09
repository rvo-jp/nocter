# Development Milestones

This directory owns active and historical milestone scope, acceptance criteria, and qualification
state. Public language behavior belongs in `spec/`; compiler architecture belongs in
`development/docs/`.

The current milestone record is [v0.11.0](v0.11.0.md). Phase 0 completed intrinsic `copy` generic
requirements, callable requirement clauses, and standard-library copy-contract migration. Phase 1
completed required interface associated types, exact conformance bindings, semantic projections,
and compiler-backed editor integration. Phase 2 is implementing associated type bounds and equality
predicates, then migrating the iterator interfaces away from courier generic parameters.

[v0.10.0](v0.10.0.md) is complete and historical. Phase 0 completed native value capabilities,
Phase 1 completed directory modules and explicit source composition, Phase 2 completed
source-backed callable contracts plus standard-library boundary normalization, and Phase 3
completed hierarchical visibility plus the implicit standard-library package. Its frozen Phase 3
record is [v0.10.0 Phase 3](v0.10.0-phase-3.md), and its published qualification is preserved in
[the release record](../releases/v0.10.0.md). The published baseline is v0.10.0.

[v0.9.0](v0.9.0.md) is complete and historical. Its qualified archive was published and audited on
2026-08-08.

Do not reuse or reopen a completed release qualification record as an active plan.
