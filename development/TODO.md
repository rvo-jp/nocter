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
- tracked std provides checked `Layout`, provenance-carrying `RawBuffer`, failure-atomic allocator
  growth, initial `String`/`Vec<T>`, and file/process/fmt support
- LSP provides diagnostics, semantic tokens, hover, definition, references, document symbols,
  resolved generic signature help, and several completion contexts

## Next Concrete Area

Complete the v0.2.0 LSP contract on top of compiler-owned semantic facts.

1. Add scope-correct local and parameter completion without recreating name resolution.
2. Present callable/member/field types, receiver capability, documentation, and insert text in
   completion items.
3. Recover signature help and completion for incomplete call/member/import edits, then fix the
   multi-file JSON-RPC acceptance sequence.

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
