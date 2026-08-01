# Nocter v0.2.0 Handoff

この file は次の実装作業に必要な短期情報だけを持つ。終了条件と全体順序は
[v0.2.0 Development Contract](docs/v0.2.0.md) を参照する。完了履歴は Git にある。

## Current Baseline

- branch: `develop`
- version: `0.2.0-dev`
- target: `arm64-darwin`
- compiler responsibilities have been split into focused parser, AST JSON, import resolution, IR
  lowering, buildability, ownership, backend, and analysis modules
- recursive drop obligations cover nested struct fields, fixed-array completed prefixes and the
  partially constructed current element, and payload enum fields
- tracked std provides initial `Allocator`, `RawBuffer`, `String`, `Vec<T>`, file/process/fmt support
- LSP provides diagnostics, semantic tokens, hover, definition, references, document symbols, and
  several completion contexts

## Next Concrete Area

Complete the common Allocator contract before promoting ownership-safe variable-length collections.

1. Make `Layout` the checked constructor for size/alignment/overflow/zero-size rules.
2. Give `RawBuffer` enough allocation provenance to free through the allocator that created it.
3. Add failure-atomic grow and runtime tests for invalid layout, overflow, allocation failure, and
   canonical empty buffers.
4. Move `String` and `Vec<T>` from direct page primitives to the common Allocator contract.
5. Reuse fixed-array current-element transitions for `Vec<T>.push`, initialized length, clear, and
   drop.

Do not implement arbitrary indexed `remove` as an exception to prefix ownership. It requires a later
sparse-live-element design. `pop` is allowed after initialized-prefix transfer is correct.

## Required Verification

For the next compiler behavior commit:

```sh
./development/compiler/scripts/verify.sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
git diff --check
```

Add a narrow IR/ownership test first and a CLI or distributed-home runtime test when collection
behavior becomes user-visible.

## Handoff Discipline

Replace this file's baseline and next area when they change. Do not append session logs, command
transcripts, commit lists, or completed checklists.
