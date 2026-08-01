# Nocter Development

This directory contains the Rust bootstrap compiler, the distributed standard library, release
packaging inputs, and implementation documentation. See the [repository README](../README.md) for
the public overview and the [specification](../spec/README.md) for language rules.

The completed development milestone is **Nocter v0.2.0**. Its completion criteria are recorded in
the [v0.2.0 Development Contract](docs/v0.2.0.md).

## Quick Start

Run the complete verification suite from the repository root:

```sh
./development/compiler/scripts/verify.sh
```

To verify only the compiler:

```sh
cargo test --manifest-path development/compiler/Cargo.toml
```

To build and run a repository-local distribution:

```sh
./development/compiler/scripts/package-local-release.sh
./dist/.nocter/nocter example.nct
```

Rust and Cargo are required only for development. The release archive runs from a single
`.nocter/` home containing the compiler and `std/`; users do not need LLVM, `clang`, `as`, `ld`, an
external runtime library, or the Xcode Command Line Tools.

## Documents

- [Documentation Index](docs/README.md)
- [v0.2.0 Development Contract](docs/v0.2.0.md)
- [Compiler Architecture](docs/architecture.md)
- [Allocator and Ownership](docs/allocator-ownership.md)
- [Standard Library Runtime](docs/standard-library.md)
- [Language Server](docs/lsp.md)
- [Maintenance](docs/maintenance.md)
- [Internal Handoff](TODO.md)

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
- `compiler/tests`: CLI, runtime, distributed-home, LSP, and corpus integration tests
- `std`: canonical source for the packaged standard library
- `packaging`: release metadata copied into generated homes
- `docs`: current design and acceptance documents; Git retains history
