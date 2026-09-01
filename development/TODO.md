# Nocter Development Handoff

## Current State

Nocter v0.24.0 is [published and externally audited](releases/v0.24.0.md). Its explicit-value-subject
migration, installation-image stabilization, independent source qualification, deterministic
packaging, and fresh installed-home matrix are complete. The exact source, artifact, publication,
and public re-download evidence is frozen. The `v0.24.0` tag and release asset must not be replaced.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
milestone, release, and review records rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

Implement [v0.25.0](milestones/v0.25.0.md) from the adopted `noalloc` specification. Phase 1 adds
syntax and declaration identity without effect inference. Phase 2 then introduces the sole checked
effect authority; later phases connect indirect behavior, standard contracts, and tooling without
letting Machine context propagation become a second semantic producer.

Do not cache `NodeId`, `SourceId`, frontend bindings, or `SourceIndex` as if they were reusable
semantic programs. Stable declaration identities, module-local semantic queries, feature-demand
editor analysis, cancellation, parallelism, and persistent caches remain later work.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
