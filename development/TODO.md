# Nocter v0.3.0 Phase 0 Completion Handoff

終了条件と全体順序は [v0.3.0 Development Contract](docs/v0.3.0.md) にある。
region/provenance の実装境界は [Region, Provenance, and Allocation Context](docs/region-provenance.md)
にある。実装履歴は Git にあり、この file に session log は残さない。

## Current Baseline

- branch: `develop`
- released baseline: `0.2.0`
- completed milestone gate: `0.3.0 Phase 0`
- target: `arm64-darwin`
- compiler responsibilities have been split into focused parser, AST JSON, import resolution, IR
  lowering, buildability, ownership, backend, and analysis modules
- recursive drop obligations cover nested struct fields, fixed-array completed prefixes and the
  partially constructed current element, and payload enum fields
- tracked std provides checked `Layout`, provenance-carrying `RawBuffer`, failure-atomic allocator
  growth, practical `String`/`Vec<T>`, and file/process/fmt support
- LSP provides diagnostics, semantic tokens, hover, definition, references, document symbols,
  resolved generic signature help, semantic completion contexts, and incomplete-edit recovery

## Completed Gate

Phase 0 の必須責務は compiler、distributed standard library、native runtime、analysis/LSP に
接続済み。共有 provenance は scope、input、lexical region、current allocation context を追跡し、
owned container、borrow、aggregate、optional、fallible channel、helper call、move を通過しても
origin を保持する。return、外側 binding への代入、間接 aggregate escape は同じ environment で
検査され、region-independent な copy value だけが lexical region を離れられる。

`region name using allocator { ... }` は専用 AST、parser recovery、resolver identity、typecheck、
IR lowering、ARM64 runtime を持つ。すべての exiting edge は live value を drop してから region を
release する。runtime test は region 内で確保した mapping が body 内では有効で、region 終了後に
解放されることを OS から観測する。

trusted declaration registry は capability、current-context allocation、region runtime、recoverable
I/O error の意味を declaration identity と primitive shape で検証する。公開名の綴りを semantic
magic として扱わない。共有 fallible core、aborting `Allocator`、`TryAllocator`、normal/`try_*`
collection surface は実装済み。

LSP は compiler analysis から region identity、parent/current context、allocation effect、provenance
を取得する。incomplete region header の recovery は元 source の cursor offset を保持し、hover、
completion、definition、semantic token、diagnostic が recovery 専用の第二意味モデルを持たない。

Phase 0 gate:

```sh
./development/compiler/scripts/verify.sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
git diff --check
```

- v0.2.0 release gate: passed at tag `v0.2.0`
- v0.3.0 Phase 0 implementation: complete on `develop`
- focused and complete verification gates: passed
- required Phase 0 TODO items: none

Phase 1 は未開始。typed literals、per-literal `using`、spread、iteration を実装する前に、Phase 0 と
同じ形式で completion definition、非目標、受け入れ matrix をレビューして新しい active gate を
作る。
