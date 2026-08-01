# Nocter v0.3.0 Phase 0 Handoff

終了条件と全体順序は [v0.3.0 Development Contract](docs/v0.3.0.md) にある。
region/provenance の実装境界は [Region, Provenance, and Allocation Context](docs/region-provenance.md)
にある。実装履歴は Git にあり、この file に session log は残さない。

## Current Baseline

- branch: `develop`
- released baseline: `0.2.0`
- active milestone: `0.3.0 Phase 0`
- target: `arm64-darwin`
- compiler responsibilities have been split into focused parser, AST JSON, import resolution, IR
  lowering, buildability, ownership, backend, and analysis modules
- recursive drop obligations cover nested struct fields, fixed-array completed prefixes and the
  partially constructed current element, and payload enum fields
- tracked std provides checked `Layout`, provenance-carrying `RawBuffer`, failure-atomic allocator
  growth, practical `String`/`Vec<T>`, and file/process/fmt support
- LSP provides diagnostics, semantic tokens, hover, definition, references, document symbols,
  resolved generic signature help, semantic completion contexts, and incomplete-edit recovery

## Current Objective

既存の borrow-return 専用 provenance を、return 検査・NLL・region escape・allocation effect・
analysis が共有できる model へ抽出する。最初の behavior-preserving checkpoint では、
parameter 名の文字列ではなく resolver の declaration/input identity で origin を表す。

この抽出が完了するまで、region parser、ambient allocator、literal 構文を追加しない。

Phase 0 gate:

```sh
./development/compiler/scripts/verify.sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
git diff --check
```

- v0.2.0 release gate: passed at tag `v0.2.0`
- v0.3.0 Phase 0 implementation: not started
- current documentation contract: defined
