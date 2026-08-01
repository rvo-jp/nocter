# Development Maintenance

この文書は長期運用規約を持つ。短期引き継ぎは [TODO](../TODO.md)、公開言語規則は
[spec](../../spec/README.md) に置く。

## Design Rules

- diff の小ささより、責務の一貫性と次の変更の容易さを優先する。
- line count ではなく responsibility と abstraction layer で分割する。
- caller が内部 map や mutable state を探索する API より、目的を表す owned result を返す。
- compiler phase、protocol transport、presentation を一つの file に混ぜない。
- AST traversal、lookup、type formatting、drop logic の複製が必要になった時点で共通責務を
  抽出する。
- removed repository location や未公開 behavior の compatibility shim を追加しない。

新しい責務は新しい module/file に作る。既存 file への追加で責務が自然に説明できる場合だけ
同居させる。

## Sources of Truth

| Information | Owner |
|---|---|
| language and public std semantics | `spec/` |
| current release completion and priorities | `docs/v0.2.0.md` |
| compiler phase boundaries | `docs/architecture.md` |
| allocator, ownership, drop invariants | `docs/allocator-ownership.md` |
| distributed standard-library implementation | `docs/standard-library.md` |
| LSP capability and analysis design | `docs/lsp.md` |
| next task and handoff facts | `TODO.md` |
| historical sequence | Git history |

同じ status table を複数文書に置かない。v0.2.0 の checklist は `v0.2.0.md` だけに置き、
個別文書は設計と具体的な acceptance behavior を持つ。

## Update Triggers

- release gate、non-goal、work order を変えた: `v0.2.0.md`
- compiler module ownership や phase data flow を変えた: `architecture.md`
- allocation/drop/collection invariant を変えた: `allocator-ownership.md`
- tracked `development/std` の runtime behavior を変えた: `standard-library.md`
- editor-facing capabilityまたは analysis API を変えた: `lsp.md`
- 次の具体的 task、blocker、uncommitted state が変わった: `TODO.md`

文書へ command log、commit list、完了項目の年代記を追記しない。現在の判断に必要な fact だけを
置き換える。

## Verification

共有 compiler behavior を変更した commit の標準検証は repository root から行う。

```sh
./development/compiler/scripts/verify.sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
git diff --check
```

変更に応じて narrow test を先に実行し、最後に full verification を行う。標準ライブラリの
runtime promotion には distributed-home または CLI run test、LSP behavior には analysis unit
test と JSON-RPC integration test を含める。

docs-only change では link/path search、Markdown structure、`git diff --check` を最低条件とする。

## Commit Checkpoints

- behavior change と test/doc update を一つの coherent commit にする。
- pure refactor は behavior promotion と分離する。
- unrelated user changes を stage、revert、format しない。
- coherent chunk が検証済みになったら、長い session の終了を待たず commit する。
- verification を実行できない場合は理由を final response と必要なら `TODO.md` に残す。

commit message は変更の結果を述べる。時系列メモや「続き」は使わない。
