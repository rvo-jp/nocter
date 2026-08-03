# Nocter Development

This directory contains the Rust bootstrap compiler, the distributed standard library, release
packaging inputs, and implementation documentation. See the [repository README](../README.md) for
the public overview and the [specification](../spec/README.md) for language rules.

The released baseline is **Nocter v0.2.0**. **v0.3.0 Phase 0: Region and Allocation Context**,
**Phase 1: Typed Literal Core**, **Phase 2: Explicit Iteration and Collection Access**, and
**Phase 3: Owned String Interpolation and Formatting**, **Phase 4: Public Provenance Contracts
and Generic Interface Bounds**, **Phase 5: Nested Outcomes and Executable Process Context**,
**Phase 6: First-Class Outcome Values**, **Phase 7: Protocol-Driven Collection Iteration**, and
**Phase 8: Explicit Sequence Spread and Composable Element Packs**, and **Phase 9: Composable
Iterators and Collection Builders** are complete on `develop`. **Phase 10: Callable Values and
Contract-Derived Extensions** is active. The completion records and dependency order are defined in the
[v0.3.0 Development Contract](docs/v0.3.0.md). The completed v0.2.0 criteria remain available in the
[v0.2.0 release record](docs/v0.2.0.md).

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

The canonical standard-library source is tracked in `development/std/`, and release metadata lives
in `development/packaging/`. The packaging script creates a local installation image at
`dist/.nocter/` and an archive named `dist/nocter-v<version>-arm64-darwin.tar.gz`.

Rust and Cargo are required only for development. The release archive runs from a single
`.nocter/` home containing the compiler and `std/`; users do not need LLVM, `clang`, `as`, `ld`, an
external runtime library, or the Xcode Command Line Tools.

## Documents

- [Documentation Index](docs/README.md)
- [v0.3.0 Development Contract](docs/v0.3.0.md)
- [v0.2.0 Release Record](docs/v0.2.0.md)
- [Compiler Architecture](docs/architecture.md)
- [Region, Provenance, and Allocation Context](docs/region-provenance.md)
- [Typed Literal Core](docs/typed-literals.md)
- [Explicit Iteration and Collection Access](docs/iteration.md)
- [Owned String Interpolation and Formatting](docs/interpolation.md)
- [Public Provenance Contracts and Generic Interface Bounds](docs/provenance-contracts.md)
- [Composable Iterators and Collection Builders](docs/iterator-composition.md)
- [Callable Values and Contract-Derived Extensions](docs/callable-extensions.md)
- [Nested Outcomes and Executable Process Context](docs/outcomes-process-context.md)
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
