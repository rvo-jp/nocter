# Nocter Development Documents

This directory contains implementation design and completion criteria. The public language
[specification](../../spec/README.md) is the sole authority for language semantics; do not duplicate
those rules here.

The recorded milestone is **v0.2.0**. Do not use `v0` as shorthand for a release name or work scope.

## Documents

- [v0.2.0 Development Contract](v0.2.0.md): completion criteria, non-goals, implementation order,
  and release gates; the single entry point for milestone status
- [Architecture](architecture.md): compiler phase responsibilities and boundaries
- [Allocator and Ownership](allocator-ownership.md): the shared allocation, ownership, partial
  initialization, `String`, and `Vec<T>` foundation
- [Standard Library](standard-library.md): distributed standard-library behavior and v0.2.0
  runtime acceptance criteria
- [LSP](lsp.md): compiler-backed LSP design and v0.2.0 acceptance criteria
- [Maintenance](maintenance.md): update ownership, verification, and commit policy
- [TODO](../TODO.md): internal short-term handoff state

## Information Ownership

| Information | Owner |
|---|---|
| Public language rules | `spec/` |
| v0.2.0 completion criteria, scope, and priority | `v0.2.0.md` |
| Compiler responsibility boundaries | `architecture.md` |
| Allocator, ownership, and drop design | `allocator-ownership.md` |
| Distributed `std` implementation state | `standard-library.md` |
| LSP capabilities and analysis boundary | `lsp.md` |
| Next concrete internal task | `../TODO.md` |

Do not copy chronological completion lists or commit history into design documents. Git owns the
history.
