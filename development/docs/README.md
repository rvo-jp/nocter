# Nocter Development Documents

This directory contains implementation design and completion criteria. The public language
[specification](../../spec/README.md) is the sole authority for language semantics; do not duplicate
those rules here.

The released baseline is **v0.2.0**. v0.3.0 Phase 0, **Phase 1: Typed Literal Core**, and
**Phase 2: Explicit Iteration and Collection Access** are complete on `develop`;
**Phase 3: Owned String Interpolation and Formatting** is active. Do not use `v0` as shorthand for
a release name or work scope.

## Documents

- [v0.3.0 Development Contract](v0.3.0.md): completed Phase 0 through Phase 2 records and the active
  Phase 3 gate; the single entry point for current milestone status
- [v0.2.0 Release Record](v0.2.0.md): immutable completion criteria for the released baseline
- [Architecture](architecture.md): compiler phase responsibilities and boundaries
- [Region, Provenance, and Allocation Context](region-provenance.md): shared storage-origin,
  allocation-effect, lexical-region, and lowering design
- [Typed Literal Core](typed-literals.md): literal shapes, definitions, element packs, context
  selection, and lowering boundaries
- [Explicit Iteration and Collection Access](iteration.md): readonly and owned iterator invariants,
  optional access, shifting, cleanup, and LSP boundaries
- [Owned String Interpolation and Formatting](interpolation.md): runtime declarations, semantic
  plans, formatting policy, lowering, cleanup, and LSP boundaries
- [Allocator and Ownership](allocator-ownership.md): the shared allocation, ownership, partial
  initialization, `String`, and `Vec<T>` foundation
- [Standard Library](standard-library.md): released runtime baseline and completed Phase 0
  allocator-policy integration
- [LSP](lsp.md): released compiler-backed capabilities and completed Phase 0 region integration
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
| Allocator, ownership, and drop design | `allocator-ownership.md` |
| Distributed `std` implementation state | `standard-library.md` |
| LSP capabilities and analysis boundary | `lsp.md` |
| Next concrete internal task | `../TODO.md` |

Do not copy chronological completion lists or commit history into design documents. Git owns the
history.
