# Contributor Documentation

This directory is the entry point for compiler contributors. It contains the Rust bootstrap
compiler, distributed standard-library source, release packaging inputs, implementation design,
milestone records, and release qualification evidence. See the [repository README](../README.md)
for the product overview and the [specification](../spec/README.md) for public language behavior.

The [current handoff](TODO.md) owns the next concrete task. The
[milestone index](milestones/README.md) identifies an active candidate when one exists. Do not copy
either state into this entry point.

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
./dist/.nocter/nocter check --root examples/hello
./dist/.nocter/nocter run --root examples/hello
```

The canonical standard-library source is tracked in `development/std/`, and release metadata lives
in `development/packaging/`. The packaging script creates a local installation image at
`dist/.nocter/` and an archive named `dist/nocter-v<version>-arm64-darwin.tar.gz`.

Rust and Cargo are required only for development. The release archive runs from a single
`.nocter/` home containing the compiler and `std/`; users do not need LLVM, `clang`, `as`, `ld`, an
external runtime library, or the Xcode Command Line Tools.

## Documents

- [Implementation Documentation](docs/README.md)
- [Active Milestones](milestones/README.md)
- [Release Qualification Records](releases/README.md)
- [Packages, Dependencies, and Locks](docs/packages.md)
- [Immutable LSP Snapshots](docs/lsp-snapshots.md)
- [Compiler Architecture](docs/architecture.md)
- [Region, Provenance, and Allocation Context](docs/region-provenance.md)
- [Typed Literal Core](docs/typed-literals.md)
- [Explicit Iteration and Collection Access](docs/iteration.md)
- [Owned String Interpolation and Formatting](docs/interpolation.md)
- [Public Provenance Contracts and Compiler-Owned Result Storage](docs/provenance-contracts.md)
- [Composable Iterators and Collection Builders](docs/iterator-composition.md)
- [Callable Values and Interface Default Methods](docs/callable-default-methods.md)
- [Nested Outcomes and Executable Process Context](docs/outcomes-process-context.md)
- [Allocator and Ownership](docs/allocator-ownership.md)
- [Standard Library Runtime](docs/standard-library.md)
- [Language Server](docs/lsp.md)
- [Documentation Site Generation](docs/site-generation.md)
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
├── milestones/
├── packaging/
├── releases/
└── std/
```

- `compiler/src`: compiler implementation
- `compiler/tests`: CLI, runtime, distributed-home, LSP, and corpus integration tests
- `std`: canonical source for the packaged standard library
- `packaging`: release metadata copied into generated homes
- `docs`: compiler and standard-library implementation design
- `milestones`: active candidate scope, completion criteria, and qualification
- `releases`: frozen compiler-developer qualification records
