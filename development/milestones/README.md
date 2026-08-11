# Development Milestones

This directory owns active and historical milestone scope, acceptance criteria, and qualification
state. Public language behavior belongs in `spec/`; compiler architecture belongs in
`development/docs/`.

The active milestone is [v0.13.0](v0.13.0.md). Phase 0 is complete: source-defined readonly and
readwrite index declarations make structural index requirements implementable by user-owned types
through the same semantic plan and static-call infrastructure. No later v0.13.0 phase is active.

The latest completed milestone is [v0.12.0](v0.12.0.md). Phases 0 through 3 are complete: closed
interpolation formatting was replaced with a source-defined `Format` contract, fixed
instance-owned equality now drives generic and collection comparison, and equality plus indexing
share uniform structural operator requirements with coercion-driven execution. Phase 3 completed
the semantic audit, candidate identity, incremental and clean verification, and archive
qualification. Its publication and independent public-asset audit are preserved in
[the release record](../releases/v0.12.0.md), and the published baseline is v0.12.0. No later
release has been qualified.

The latest completed milestone record is [v0.11.0](v0.11.0.md). Phases 0 through 8 completed
unified generic requirements, associated types and bounds, declaration-wide `where` clauses,
separate `instance` and `conform` declarations, declaration type patterns, independent destruction
declarations, path-sensitive aggregate cleanup for native control flow, and static opaque result
types for interface-based APIs. No later phase is active. Its published qualification is preserved
in [the release record](../releases/v0.11.0.md), and the published baseline is v0.11.0.

[v0.10.0](v0.10.0.md) is complete and historical. Phase 0 completed native value capabilities,
Phase 1 completed directory modules and explicit source composition, Phase 2 completed
source-backed callable contracts plus standard-library boundary normalization, and Phase 3
completed hierarchical visibility plus the implicit standard-library package. Its frozen Phase 3
record is [v0.10.0 Phase 3](v0.10.0-phase-3.md), and its published qualification is preserved in
[the release record](../releases/v0.10.0.md).

[v0.9.0](v0.9.0.md) is complete and historical. Its qualified archive was published and audited on
2026-08-08.

Do not reuse or reopen a completed release qualification record as an active plan.
