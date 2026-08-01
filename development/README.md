# Nocter Development

このディレクトリは Rust bootstrap compiler、配布標準ライブラリ、release packaging input、
実装者向け文書を持つ。公開説明は [repository README](../README.md)、言語規則は
[spec](../spec/README.md) に置く。

現行の開発対象は **Nocter v0.2.0** だけである。終了条件は
[v0.2.0 Development Contract](docs/v0.2.0.md) を参照する。

## Quick Start

repository root から全検証を実行する。

```sh
./development/compiler/scripts/verify.sh
```

compiler だけを検証する場合:

```sh
cargo test --manifest-path development/compiler/Cargo.toml
```

repository-local distribution を生成して実行する場合:

```sh
./development/compiler/scripts/package-local-release.sh
./dist/.nocter/nocter example.nct
```

Rust/Cargo は開発時だけ必要である。配布 archive は compiler と `std/` を含む一つの
`.nocter/` home で動作し、LLVM、`clang`、`as`、`ld`、外部 runtime library を user に
要求しない。

## Documents

- [Documentation Index](docs/README.md)
- [v0.2.0 Development Contract](docs/v0.2.0.md)
- [Compiler Architecture](docs/architecture.md)
- [Allocator and Ownership](docs/allocator-ownership.md)
- [Standard Library Runtime](docs/standard-library.md)
- [Language Server](docs/lsp.md)
- [Maintenance](docs/maintenance.md)
- [Current Handoff](TODO.md)

## Layout

```text
development/
├── AGENTS.md
├── README.md
├── TODO.md
├── compiler/
│   ├── Cargo.toml
│   ├── scripts/
│   ├── src/
│   └── tests/
├── docs/
├── packaging/
└── std/
```

- `compiler/src`: compiler implementation
- `compiler/tests`: CLI、runtime、distributed-home、LSP、corpus integration tests
- `std`: packaged standard-library source of truth
- `packaging`: release metadata copied into generated homes
- `docs`: current design and acceptance documents; Git history is not duplicated here
