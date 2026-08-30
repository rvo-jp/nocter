# Nocter Development Handoff

## Current State

Nocter v0.21.0 is [published and externally audited](releases/v0.21.0.md). The exact source,
artifact, publication, and public re-download evidence is frozen; its tag and release asset must
not be replaced.

The active [v0.22.0 milestone](milestones/v0.22.0.md) adds a strict owning JSON DOM, exact number
tokens, parsing, and generation in ordinary standard-library source. Phases 0 and 1 are complete
and reviewed. The public contract, implementation boundary, exact Number representation, lexical
cursor, Unicode escape foundation, and typed allocation/input failure channel are closed; Phase 2
is the next implementation checkpoint.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
milestone, release, and review records rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

Implement v0.22.0 Phase 2 from the adopted JSON contract and
`development/docs/json-implementation.md`:

- implement the owning `Value` enum without compiler-recognized recursive storage;
- build one non-recursive parser over the Phase 1 cursor and an explicit `Vec<Frame>` stack;
- route every nested String, Vec, and Map allocation through the selected allocator;
- reject duplicate decoded object names before Map insertion;
- qualify every root kind, whitespace, malformed boundary, deep nesting, cleanup, and affinity;
- review Phase 2 ownership, partial-state cleanup, and absence of recursive native parsing.

Do not add generation during Phase 2. Do not introduce floating point, `char`, a token array,
public failure wrappers, recursive JSON input parsing, or a compiler-known JSON declaration.

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
