# Nocter Compiler Architecture

この文書は Rust bootstrap compiler の安定した責務境界を定義する。公開言語規則は
[spec](../../spec/README.md)、現在の完了条件は [v0.2.0](v0.2.0.md) を参照する。

## Pipeline

```text
.nct source
  -> SourceMap
  -> lexer / parser
  -> module loading / resolution
  -> type checking / ownership facts
  -> buildability preflight
  -> IR lowering
  -> ABI classification
  -> ARM64 code generation
  -> Mach-O image
```

通常の user build は LLVM、`clang`、`as`、`ld`、Xcode Command Line Tools、外部 runtime
library を要求しない。v0.2.0 の native target は `arm64-darwin` である。

## Phase Ownership

| Area | Owns |
|---|---|
| `source` | canonical file identity、byte spans、line mapping |
| `lexer` | tokens と lexical diagnostics |
| `parser` | AST construction、syntax recovery、removed-syntax diagnostics |
| `ast` | syntax data、AST JSON、documentation extraction |
| `frontend` | compile-unit loading、prelude、frontend orchestration |
| `resolve` | imports、visibility、scopes、symbols、declaration identity |
| `typecheck` | types、generic specialization、places、ownership、borrows、drop semantics |
| `analysis` | compiler facts から editor/query 用の owned results を作る |
| `driver/buildability` | checked だが runtime 未対応の source を preflight rejection する |
| `ir` | typed facts から explicit lower-level operations への変換 |
| `abi` | data layout、argument/return classification |
| `backend` | IR validation、ARM64 emission、Mach-O output |
| `target` | machine encoding と target-specific output details |
| `diagnostics` | structured diagnostics と text/JSON rendering |
| `driver` | CLI、pipeline、LSP protocol orchestration |

後段は前段の facts を消費できるが、前段の判断を再実装しない。新しい責務が既存領域に
収まらない場合は、広い helper を足す前に専用 module と狭い API を作る。

## Compile-unit and Source Identity

- `SourceMap` が compiler 全体の source identity を所有する。
- import graph 内の各 file は canonical identity を一つだけ持つ。
- diagnostics は場所が分かる限り source-backed span を持つ。
- LSP の open document は disk content を overlay できるが、別 identity を作らない。
- parser recovery 後も resolver/typechecker は synthetic node と real declaration を区別する。

## Semantic Boundary

parser が受理した形は次のいずれかに到達しなければならない。

1. resolver/typechecker が compiler-owned facts を生成する。
2. parser、resolver、typechecker、buildability のいずれかが source-backed diagnostic で拒否する。

backend が raw AST から不足した言語意味を推測する第三の経路は作らない。lowering に必要な
型、symbol、ownership、variant、drop shape は resolver/typechecker output に置く。

## Buildability Boundary

checkable language と native runtime subset は同一ではない。`driver/buildability` は frontend
で正しいが IR/backend が安全に実行できない形を、machine-code error になる前に拒否する。

feature を buildable へ昇格するときは、必要な parser → resolver → typecheck → ownership →
IR → ABI → backend → CLI/std/LSP の経路を確認する。純粋な AST shape classification は共有
してよいが、symbol identity や type compatibility など phase-specific facts は混ぜない。

## IR, ABI, and Backend

- IR は ownership transfer、drop obligation、fallible exit を explicit operation として持つ。
- ABI classification は `abi` に一元化し、lowering と backend validation が共有する。
- user source の未対応形状は buildability で止め、backend validation は drifted/hand-built IR
  の防壁として使う。
- target-specific syscall と encoding は backend/target および target-gated std internals に
  閉じ込める。
- layout/ABI の公開動作を変えるときは
  [ABI and Layout](../../spec/09-abi-layout.md) も更新する。

## Allocator and Drop Boundary

Allocator は標準ライブラリの通常 API だが、compiler は所有値の runtime drop を表現できる
必要がある。型ごとの immutable drop shape、経路ごとの mutable drop obligation、allocator
provenance を分離する。詳細は [Allocator and Ownership](allocator-ownership.md) に置く。

compiler は `Allocator`、`String`、`Vec` という公開名を special-case しない。必要な primitive
は `pub(nocter)` の trust boundary と明示的 IR operation に限定する。

## LSP Boundary

`driver/lsp` は transport、document state、protocol conversion を担当する。hover、completion、
definition、references、signature help の semantic data は `analysis` が resolver/typechecker facts
から構築する。詳細は [LSP](lsp.md) に置く。

## Diagnostics

- malformed user source で panic しない。
- text diagnostics は file、line、column、snippet、primary marker、必要なら help を持つ。
- JSON/LSP diagnostics は安定した machine-readable spans を保持する。
- ordinary user source に backend implementation terminology を見せない。
- 同じ semantic error は check、build、run、LSP で同じ診断経路を通す。

## Testing Layers

| Layer | Proves |
|---|---|
| lexer/parser | syntax shape、recovery、removed syntax |
| resolver | imports、visibility、symbol identity、source loading |
| typecheck | types、generics、ownership、borrows、drop、diagnostics |
| buildability | runtime 未対応形の early rejection |
| IR | operation shape、ownership/drop transitions、ABI handoff |
| backend/target | frame/layout assumptions、instruction encoding、emission |
| CLI build/run | user-visible native behavior |
| distributed home | packaged std visibility と runtime behavior |
| analysis/LSP | compiler facts と protocol response の一致 |

user-visible promotion には最小 phase test に加え、少なくとも一つの CLI、distributed-home、
または LSP integration test を付ける。
