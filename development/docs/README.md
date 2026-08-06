# Nocter Development Documents

This directory contains implementation design and completion criteria. The public language
[specification](../../spec/README.md) is the sole authority for language semantics; do not duplicate
those rules here.

The current release is **v0.4.0**. It includes source-native package roots, deterministic exact
dependency graphs, immutable package-wide LSP snapshots, and the completed stabilization audit.
The previous v0.3.0 language milestone and v0.2.0 contract remain historical records. Do not use
`v0` as shorthand for a release name or work scope.

Active development is **v0.5.0 Phase 5: Package Authoring and Stabilization**. Phase 0's
published-artifact audit, Phase 1's explicit package test targets, Phase 2's native test
declarations, Phase 3's package-wide editor index, and Phase 4's practical standard library are
complete.

## Documents

- [v0.5.0 Development Plan](v0.5.0.md): completed published-artifact, native testing,
  package-wide editor, and practical standard-library records; active stabilization; and non-goals
- [v0.4.0 Release Record](v0.4.0.md): completed Phase 0 through Phase 2 and stabilization
  records, qualification, and non-goals
- [Packages, Dependencies, and Locks](packages.md): package files, semantic identities,
  dependency graphs, exact locks, stores, and compiler boundaries
- [Immutable LSP Snapshots](lsp-snapshots.md): editor generations, package contexts, source
  overlays, invalidation, and request consistency
- [v0.3.0 Release Record](v0.3.0.md): completed Phase 0 through Phase 10 records,
  stabilization criteria, release qualification, and explicit limits
- [Body-Bearing Interface Implementations](interface-implementations.md): canonical conformance
  member identity, lookup, validation, migration, and editor boundaries
- [Construction Surfaces](construction-surfaces.md): compiler-owned construction entries,
  default selection, lowering reuse, and editor boundaries
- [v0.2.0 Release Record](v0.2.0.md): immutable completion criteria for the released baseline
- [Architecture](architecture.md): compiler phase responsibilities and boundaries
- [Region, Provenance, and Allocation Context](region-provenance.md): shared storage-origin,
  allocation-effect, lexical-region, and lowering design
- [Typed Literals and Composable Element Packs](typed-literals.md): literal shapes, definitions,
  spread segments, context selection, and lowering boundaries
- [Explicit Iteration and Collection Access](iteration.md): readonly and owned iterator invariants,
  optional access, shifting, cleanup, and LSP boundaries
- [Owned String Interpolation and Formatting](interpolation.md): runtime declarations, semantic
  plans, formatting policy, lowering, cleanup, and LSP boundaries
- [Public Provenance Contracts and Generic Interface Bounds](provenance-contracts.md): explicit
  result origins, generic capability lookup, static specialization, and editor boundaries
- [Nested Outcomes and Executable Process Context](outcomes-process-context.md): recursive callable
  result channels, native ABI, process storage, and ambient/recoverable process APIs
- [First-Class Outcome Values](outcome-values.md): recursive stored outcome layout, callable
  bridging, active-payload ownership, consumers, and tooling
- [Protocol-Driven Collection Iteration](iteration-protocol.md): trusted protocol roles, collection
  conversion, loop ownership, cleanup, and editor boundaries
- [Composable Iterators and Collection Builders](iterator-composition.md): capability sets,
  conditional conformance, adapter state, collection construction, and editor boundaries
- [Callable Values and Interface Default Methods](callable-default-methods.md): method generics,
  required/default methods, closure ownership, callable specialization, and iterator chains
- [Allocator and Ownership](allocator-ownership.md): the shared allocation, ownership, partial
  initialization, `String`, and `Vec<T>` foundation
- [Standard Library](standard-library.md): released runtime baseline and completed v0.3.0 runtime
  integrations
- [LSP](lsp.md): released compiler-backed capabilities and completed v0.3.0 editor integrations
- [Maintenance](maintenance.md): update ownership, verification, and commit policy
- [TODO](../TODO.md): internal short-term handoff state

## Information Ownership

| Information | Owner |
|---|---|
| Public language rules | `spec/` |
| v0.5.0 milestone status and acceptance | `v0.5.0.md` |
| v0.4.0 release status, scope, and qualification | `v0.4.0.md` |
| Package/compiler responsibility boundary | `packages.md` |
| Package-wide editor state and invalidation | `lsp-snapshots.md` |
| v0.3.0 release status, scope, and qualification | `v0.3.0.md` |
| Previous v0.2.0 completion record | `v0.2.0.md` |
| Compiler responsibility boundaries | `architecture.md` |
| Region, provenance, and allocation-context implementation design | `region-provenance.md` |
| Typed literal and ephemeral element-pack implementation design | `typed-literals.md` |
| Explicit readonly/owned iteration and collection access design | `iteration.md` |
| Owned interpolation and formatting implementation design | `interpolation.md` |
| Public provenance contracts and generic interface-bound dispatch | `provenance-contracts.md` |
| Nested outcome lowering and executable process context | `outcomes-process-context.md` |
| First-class stored outcome values | `outcome-values.md` |
| Protocol-driven collection iteration | `iteration-protocol.md` |
| Composable iterators and collection builders | `iterator-composition.md` |
| Callable values, interface default methods, and iterator chains | `callable-default-methods.md` |
| Construction declarations and type-owned creation APIs | `construction-surfaces.md` |
| Allocator, ownership, and drop design | `allocator-ownership.md` |
| Distributed `std` implementation state | `standard-library.md` |
| LSP capabilities and analysis boundary | `lsp.md` |
| Next concrete internal task | `../TODO.md` |

Do not copy chronological completion lists or commit history into design documents. Git owns the
history.
