# Nocter Development Documents

This directory contains implementation design and completion criteria. The public language
[specification](../../spec/README.md) is the sole authority for language semantics; do not duplicate
those rules here.

The released baseline is **v0.2.0**. The v0.3.0 Phase 0 region and allocation-context gate is
complete on `develop`. No later phase is active until a reviewed gate replaces it. Do not use `v0`
as shorthand for a release name or work scope.

## Documents

- [v0.3.0 Development Contract](v0.3.0.md): completed Phase 0 outcome, non-goals, implementation
  order, and acceptance gate; the single entry point for current milestone status
- [v0.2.0 Release Record](v0.2.0.md): immutable completion criteria for the released baseline
- [Architecture](architecture.md): compiler phase responsibilities and boundaries
- [Region, Provenance, and Allocation Context](region-provenance.md): shared storage-origin,
  allocation-effect, lexical-region, and lowering design
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
| Active v0.3.0 phase, scope, and priority | `v0.3.0.md` |
| Released v0.2.0 completion record | `v0.2.0.md` |
| Compiler responsibility boundaries | `architecture.md` |
| Region, provenance, and allocation-context implementation design | `region-provenance.md` |
| Allocator, ownership, and drop design | `allocator-ownership.md` |
| Distributed `std` implementation state | `standard-library.md` |
| LSP capabilities and analysis boundary | `lsp.md` |
| Next concrete internal task | `../TODO.md` |

Do not copy chronological completion lists or commit history into design documents. Git owns the
history.
