# Nocter v0.2.0 Completion Record

終了条件と全体順序は [v0.2.0 Development Contract](docs/v0.2.0.md) にある。
実装履歴は Git にあり、この file に session log は残さない。

## Current Baseline

- branch: `develop`
- version: `0.2.0`
- target: `arm64-darwin`
- compiler responsibilities have been split into focused parser, AST JSON, import resolution, IR
  lowering, buildability, ownership, backend, and analysis modules
- recursive drop obligations cover nested struct fields, fixed-array completed prefixes and the
  partially constructed current element, and payload enum fields
- tracked std provides checked `Layout`, provenance-carrying `RawBuffer`, failure-atomic allocator
  growth, practical `String`/`Vec<T>`, and file/process/fmt support
- LSP provides diagnostics, semantic tokens, hover, definition, references, document symbols,
  resolved generic signature help, semantic completion contexts, and incomplete-edit recovery

## Milestone State

v0.2.0 の必須実装と acceptance scenarios は完了した。新しい開発を始める場合は、
この終了定義を再利用せず、次の version の contract と non-goals を先に作る。

最終 release gate:

```sh
./development/compiler/scripts/verify.sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
git diff --check
```

- compiler/unit/integration/native/distributed-home tests: passed
- formatter and clippy with warnings denied: passed
- local package, doctor, installed-home resolution, packaged std execution: passed
