# Language Server

Nocter LSP は compiler facts の protocol view である。エディタ向けに別の resolver や
型システムを作らない。

## Architecture

```text
open documents + filesystem
  -> compile-unit frontend
  -> resolver/typecheck facts
  -> feature-specific analysis result
  -> LSP protocol conversion
```

`driver/lsp` は JSON-RPC、document state、URI/range conversion、capability routing を所有する。
`analysis` は hover/completion/definition/references に必要な compiler-owned result types を
提供する。visibility、type normalization、generic specialization、ownership capability は
resolver/typechecker が決める。

## Current Baseline

現在は document sync と diagnostics publish、semantic tokens、hover、definition、references、
document symbols、global/member/enum-pattern/struct-field completion、signature help を持つ。
call-site analysis は resolved target、generic specialization、active parameter、documentation を
一つの compiler result に統合し、hover と signature help が共有する。completion は lexical
scope と shadowing、generic member specialization、receiver capability、signature detail、
documentation、insert text を compiler facts から返す。import path は frontend の module layout と
workspace/source root を共有し、import symbol は resolved import identity と visibility から返す。
call argument は型検査器の assignability で候補を順位付けする。編集中の call・member・import は
authoritative document と分離した一時 overlay で compile unit を回復する。

## v0.2.0 Capabilities

### Hover

hover result は presentation 用文字列を AST から再構成するのではなく、型検査済み facts
から組み立てる。

| Target | Required contents |
|---|---|
| local / parameter | mutability、borrow capability、resolved type |
| function / method | full signature、generic parameters/specialization、fallibility |
| struct / enum / interface | declaration kind、type parameters、documentation |
| field / variant | owner type、field/payload type、documentation |
| imported symbol | resolved declaration、module path、visibility |
| expression | normalized result type when a declaration target is not enough |

応答には source-backed range を含める。情報が確定できない編集途中の source では虚偽の型を
作らず、確定した宣言情報だけを返すか `null` にする。

### Completion

completion request は cursor context を分類してから候補を収集する。

- expression / statement: visible locals、parameters、functions、types、keywords
- import: current module から到達可能な modules と public symbols
- member: receiver type に対する fields と methods。borrow capability を満たさない候補を除外
- enum pattern: 対象 enum の variants と payload fields
- struct literal: 未指定 fields のみ
- call argument: expected type と active parameter に適合する visible values

候補は少なくとも `label`、`kind`、型または signature の `detail`、documentation summary、
必要な `insertText` を持つ。visibility と shadowing を尊重し、同一 semantic symbol の重複を
除く。順位は exact prefix、locality、expected-type compatibility の順を基本とし、並び順を
テストで固定する。

### Signature Help

call cursor から resolved call target と argument index を取得し、次を返す。

- full callable signature
- parameters と各 parameter documentation
- active parameter
- return type と fallibility
- generic specialization が確定していれば concrete types

overload-like 候補を文字列一致で推測しない。resolver/typechecker が target を確定できない
場合だけ、回復解析で得た候補を明示的な incomplete result として扱う。

## Reliability Requirements

- didOpen/didChange/didClose 後に stale diagnostics を残さない。
- UTF-16 LSP position と UTF-8 source byte spans の変換を一箇所に集約する。
- malformed / incomplete source、unknown imports、missing receiver で panic しない。
- open document overlay を imports からも参照し、disk text と混在した identity を作らない。
- hover、completion、signature help が同じ cursor で矛盾する型を返さない。
- protocol tests は response JSON だけでなく compiler analysis result の unit tests を持つ。

## Acceptance Tests

v0.2.0 では少なくとも次を integration test に固定する。

1. imported generic function の hover と specialized signature help
2. `Vec<String>` の method completion と receiver borrow capability
3. payload enum pattern と struct literal の未指定 field completion
4. documentation comment を持つ std symbol の hover/completion detail
5. 一文字ずつ壊れた call、member access、import を編集する連続 didChange
6. multi-file open-document overlay での definition/references/diagnostics consistency

## Deferred Features

rename、code action、formatting request、workspace-wide package index、inlay hints は
v0.2.0 の終了条件に含めない。hover/completion/signature help の semantic facts と recovery
API を安定させた後に追加する。
