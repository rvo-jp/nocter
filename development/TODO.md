# Nocter Development Handoff

## Current State

Nocter v0.21.0 is [published and externally audited](releases/v0.21.0.md). The exact source,
artifact, publication, and public re-download evidence is frozen; its tag and release asset must
not be replaced.

The active [v0.22.0 milestone](milestones/v0.22.0.md) adds a strict owning JSON DOM, exact number
tokens, parsing, and generation in ordinary standard-library source. Phase 0 is complete and
reviewed. Its future-direction public contract and implementation boundary are closed; Phase 1 is
the next implementation checkpoint.

The active compiler is under `development/compiler/`. Current architecture belongs to
`development/docs/` and colocated crate `README.md` files. Completed scope and evidence belong to
milestone, release, and review records rather than this handoff.

The documentation-authority migration is complete: all workspace crates own local contracts,
central design documents own cross-crate boundaries only, and generated documentation validates
that every workspace member carries the required README contract sections.

## Next Work

Implement v0.22.0 Phase 1 from the adopted JSON contract and
`development/docs/json-implementation.md`:

- add the package-internal Unicode-scalar encoder under the existing UTF-8 owner;
- add the package-internal active-context recoverable allocator adapter under `std/mem`;
- implement the one byte cursor, strict number scanner, exact Number storage and i64/u64 conversion;
- implement JSON escape decoding and typed internal input/allocation failure classification;
- qualify valid, boundary, invalid, ownership, allocation, formatting, and semantic-editor behavior;
- review Phase 1 authority, cleanup, and absence of compiler JSON knowledge before closing it.

Do not add container DOM parsing during Phase 1. Do not introduce floating point, `char`, a token
array, public failure wrappers, or a compiler-known JSON declaration.

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
