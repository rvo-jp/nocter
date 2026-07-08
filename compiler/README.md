# Nocter Compiler

This directory contains the Rust bootstrap compiler for Nocter.
User-facing project information lives in the repository root `README.md`.
The language specification lives in `../spec/`.

## Quick Start

Run the standard verification from the repository root:

```sh
./compiler/scripts/verify.sh
```

Or run the main test suite from this directory:

```sh
cargo test --quiet
```

Rust and Cargo are development-time dependencies only.
Nocter users should be able to install a released `.nocter/` archive without installing Rust, LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or an external runtime.

## Developer Documents

- [Architecture](docs/architecture.md): compiler pipeline, module responsibilities, source and JSON span model, and v0 phase boundaries.
- [Implementation Status](docs/implementation-status.md): what is specified, parsed, checked, buildable, and backed by runtime behavior.
- [Backend V0](docs/backend-v0.md): current ARM64 Darwin backend boundary and register-only convention.
- [Roadmap](docs/roadmap.md): implementation order and near-term constraints.
- [TODO](TODO.md): short-term handoff notes for the next compiler work session.

## Main Layout

```text
compiler/
    README.md
    TODO.md
    Cargo.toml
    Cargo.lock
    rust-toolchain.toml
    docs/
    scripts/
    src/
    tests/
```

`src/` contains the compiler implementation.
`tests/` contains user-visible CLI and corpus integration tests.
`docs/` contains compiler-only design and status documents.
