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

共有 `typecheck/provenance` model を callable summary の正式な semantic fact に昇格させ、
borrow-like call result を ownership/NLL に接続する。`ValueProvenance`、`StorageOrigin`、
`CallableId`、`InputId` と宣言 identity ベースの binding environment は導入済みで、既存の
return 検査が共有 model を consume している。

次の checkpoint では result provenance と allocation effect を保持する
`CallableProvenanceSummary` を導入し、return 専用 alias を除去する。その query API を
ownership が利用できるまで、region parser、ambient allocator、literal 構文を追加しない。

Phase 0 gate:

```sh
./development/compiler/scripts/verify.sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
git diff --check
```

- v0.2.0 release gate: passed at tag `v0.2.0`
- v0.3.0 Phase 0 implementation: shared provenance identity foundation in progress
- current documentation contract: defined
