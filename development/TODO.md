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

共有 provenance を scope/region の outlives constraint と escape 検査へ拡張する。
`ValueProvenance`、`StorageOrigin`、`CallableId`、`InputId`、宣言 identity ベースの binding
environment、`CallableProvenanceSummary` は導入済みで、return 検査と ownership/NLL が同じ
call result summary を consume している。helper result の loan は全input originを保持し、
result bindingの最終利用まで有効になる。`region name using allocator { ... }` は専用 AST、
parser、JSON、formatter、resolver identity を持ち、allocator operand は child binding の導入前に
parent scope で解決される。runtime lowering が完成するまでは buildability が明示的に拒否する。

return 専用 provenance alias は除去済み。共有 model は `RegionId`、region origin、aggregate
projection、lexical parent relation を持ち、`typecheck/regions` は `using` operand を確立済み
place に制限する。region handle の直接 return、owned aggregate 内の間接 return、外側 binding
への代入を同じ provenance environment で拒否し、pure copy result は許可する。

callable result summary は borrow-like result に限定せず、owned value の storage origin も
parameter identity から caller argument へ写像する。これにより region value は helper call や
helper が構築した aggregate を経由しても escape 検査を迂回できない。borrow receiver/parameter
については従来どおり参照先 storage を追跡し、by-value input とは分離している。

frontend は active Nocter home 内の検証済み declaration identity に compiler-owned semantic
role を付与する。`Allocator` capability は構造を含めて registry で検証され、`region using`
は任意の同名型や recoverable capability を受け付けない。callable summary の allocation
effect は trusted current-context operation を seed とし、function/method call graph を fixed
point まで伝播する。型検査、analysis、lowering は同じ `ResolveOutput` 上の trusted facts を
参照できる。

次の checkpoint では trusted current-context/region runtime primitive を標準ライブラリと
backend に接続し、共有 fallible allocator core、aborting adapter、`TryAllocator` を実装する。

Phase 0 gate:

```sh
./development/compiler/scripts/verify.sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
git diff --check
```

- v0.2.0 release gate: passed at tag `v0.2.0`
- v0.3.0 Phase 0 implementation: shared provenance and lexical region frontend in progress
- current documentation contract: defined
