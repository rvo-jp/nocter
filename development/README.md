# Nocter Development

This directory contains the Rust bootstrap compiler, the distributed standard library, release
packaging inputs, and implementation documentation. See the [repository README](../README.md) for
the public overview and the [specification](../spec/README.md) for language rules.

The current release is **Nocter v0.4.0**. It adds source-native package roots, deterministic exact
dependency graphs, and immutable package-wide LSP snapshots to the v0.3.0 language foundation.
The completed Phase 0 through Phase 2 contracts, stabilization audit, qualification, and non-goals
are recorded in the [v0.4.0 Release Record](docs/v0.4.0.md), with compiler boundaries in
[Packages, Dependencies, and Locks](docs/packages.md) and
[Immutable LSP Snapshots](docs/lsp-snapshots.md).

The previous language milestone remains available in the [v0.3.0 release record](docs/v0.3.0.md),
and the v0.2.0 criteria remain in the [v0.2.0 release record](docs/v0.2.0.md).

Active development is **v0.5.0 Phase 4: Practical Standard Library**. The published-artifact
audit, explicit package test targets, native test declarations, and package-wide editor
refactoring foundation are complete. The adopted phase boundaries are recorded in the
[v0.5.0 Development Plan](docs/v0.5.0.md).

## Quick Start

Run the complete verification suite from the repository root:

```sh
./development/compiler/scripts/verify.sh
```

To verify only the compiler:

```sh
cargo test --manifest-path development/compiler/Cargo.toml
```

The development and test profiles retain source line tables but disable incremental compilation
and macOS split debug objects. This is intentional: the compiler's large test crate otherwise
leaves hundreds of thousands of unpacked object files and can grow `development/compiler/target/`
by tens of gigabytes. To discard local compiler artifacts manually, run:

```sh
cargo clean --manifest-path development/compiler/Cargo.toml -p nocter
```

To build and run a repository-local distribution:

```sh
./development/compiler/scripts/package-local-release.sh
./dist/.nocter/nocter run example.nct
```

The canonical standard-library source is tracked in `development/std/`, and release metadata lives
in `development/packaging/`. The packaging script creates a local installation image at
`dist/.nocter/` and an archive named `dist/nocter-v<version>-arm64-darwin.tar.gz`.

Rust and Cargo are required only for development. The release archive runs from a single
`.nocter/` home containing the compiler and `std/`; users do not need LLVM, `clang`, `as`, `ld`, an
external runtime library, or the Xcode Command Line Tools.

## Documents

- [Documentation Index](docs/README.md)
- [v0.5.0 Development Plan](docs/v0.5.0.md)
- [v0.4.0 Release Record](docs/v0.4.0.md)
- [Packages, Dependencies, and Locks](docs/packages.md)
- [Immutable LSP Snapshots](docs/lsp-snapshots.md)
- [v0.3.0 Release Record](docs/v0.3.0.md)
- [v0.2.0 Release Record](docs/v0.2.0.md)
- [Compiler Architecture](docs/architecture.md)
- [Region, Provenance, and Allocation Context](docs/region-provenance.md)
- [Typed Literal Core](docs/typed-literals.md)
- [Explicit Iteration and Collection Access](docs/iteration.md)
- [Owned String Interpolation and Formatting](docs/interpolation.md)
- [Public Provenance Contracts and Generic Interface Bounds](docs/provenance-contracts.md)
- [Composable Iterators and Collection Builders](docs/iterator-composition.md)
- [Callable Values and Interface Default Methods](docs/callable-default-methods.md)
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
