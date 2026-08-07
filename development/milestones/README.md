# Development Milestones

This directory owns the scope, acceptance criteria, qualification, and publication state of the
active release candidate. Public language behavior belongs in `spec/`; compiler architecture
belongs in `development/docs/`.

The active candidate is [v0.7.0](v0.7.0.md). Phase 0 is complete: result allocation is absent from
the public callable contract, while allocation provenance remains compiler-owned dataflow used by
region escape checking, generic dispatch, and lowering. Phase 1 is active and stabilizes that
contract through the exact unpublished release candidate.

Do not reuse or reopen a completed release qualification record as an active plan.
