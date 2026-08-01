# Nocter Development Documents

このディレクトリは実装者向けの設計と進行条件だけを扱う。言語の公開仕様は
[spec](../../spec/README.md) が唯一の規範であり、ここへ複製しない。

現行の開発マイルストーンは **v0.2.0 のみ**である。本文中で `v0` を
リリース名や作業範囲の略称として使わない。

## Documents

- [v0.2.0 Development Contract](v0.2.0.md): 終了条件、非目標、実装順序、
  リリース判定。進捗判断の唯一の入口。
- [Architecture](architecture.md): コンパイラのフェーズ責務と境界。
- [Allocator and Ownership](allocator-ownership.md): メモリ確保、所有値、部分初期化、
  `String` と `Vec<T>` を成立させる共通基盤。
- [Standard Library](standard-library.md): 配布標準ライブラリの現状と v0.2.0 の
  実行時受入条件。
- [LSP](lsp.md): コンパイラ解析を再利用する LSP の設計と v0.2.0 の受入条件。
- [Maintenance](maintenance.md): 更新責務、検証、コミット規約。
- [TODO](../TODO.md): 次の作業に必要な短期引き継ぎ。

## Information Ownership

| 情報 | 更新先 |
|---|---|
| 公開言語規則 | `spec/` |
| v0.2.0 の終了条件・範囲・優先順位 | `v0.2.0.md` |
| コンパイラの責務境界 | `architecture.md` |
| Allocator・所有権・drop の設計 | `allocator-ownership.md` |
| 配布 `std` の実装状態 | `standard-library.md` |
| LSP の能力と解析境界 | `lsp.md` |
| 次に行う一つの具体的作業 | `../TODO.md` |

コミット履歴や完了項目の時系列一覧は文書へ転記しない。必要な履歴は Git が持つ。
