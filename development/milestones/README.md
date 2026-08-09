# Development Milestones

This directory owns active and historical milestone scope, acceptance criteria, and qualification
state. Public language behavior belongs in `spec/`; compiler architecture belongs in
`development/docs/`.

The current milestone record is [v0.11.0](v0.11.0.md). Its planned Phase 0 introduces an intrinsic
`copy` generic requirement and callable requirement clauses through one resolved requirement model,
then migrates standard-library copying contracts such as `Vec.from_slice` away from
implementation-only rejection.

[v0.10.0](v0.10.0.md) is complete and historical. Phase 0 completed native value capabilities,
Phase 1 completed directory modules and explicit source composition, Phase 2 completed
source-backed callable contracts plus standard-library boundary normalization, and Phase 3
completed hierarchical visibility plus the implicit standard-library package. Its frozen Phase 3
record is [v0.10.0 Phase 3](v0.10.0-phase-3.md), and its published qualification is preserved in
[the release record](../releases/v0.10.0.md). The published baseline is v0.10.0.

[v0.9.0](v0.9.0.md) is complete and historical. Its qualified archive was published and audited on
2026-08-08.

Do not reuse or reopen a completed release qualification record as an active plan.
