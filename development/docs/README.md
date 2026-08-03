# Nocter Development Documents

This directory contains implementation design and completion criteria. The public language
[specification](../../spec/README.md) is the sole authority for language semantics; do not duplicate
those rules here.

The released baseline is **v0.2.0**. v0.3.0 Phase 0, **Phase 1: Typed Literal Core**,
**Phase 2: Explicit Iteration and Collection Access**, **Phase 3: Owned String Interpolation and
Formatting**, **Phase 4: Public Provenance Contracts and Generic Interface Bounds**, and **Phase 5:
Nested Outcomes and Executable Process Context**, **Phase 6: First-Class Outcome Values**, and
**Phase 7: Protocol-Driven Collection Iteration**, **Phase 8: Explicit Sequence Spread and
Composable Element Packs**, and **Phase 9: Composable Iterators and Collection Builders** are
complete on `develop`. **Phase 10: Callable Values and Interface Default Methods** is active. Do
not use `v0` as shorthand for a release name or work scope.

## Documents

- [v0.3.0 Development Contract](v0.3.0.md): completed Phase 0 through Phase 9 records and the single
  entry point for current milestone status
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
| v0.3.0 phase status, scope, and priority | `v0.3.0.md` |
| Released v0.2.0 completion record | `v0.2.0.md` |
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
| Allocator, ownership, and drop design | `allocator-ownership.md` |
| Distributed `std` implementation state | `standard-library.md` |
| LSP capabilities and analysis boundary | `lsp.md` |
| Next concrete internal task | `../TODO.md` |

Do not copy chronological completion lists or commit history into design documents. Git owns the
history.
