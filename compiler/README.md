# Nocter Compiler

This directory contains the Rust bootstrap compiler for Nocter.

User-facing project context belongs in the repository root
[README.md](../README.md). The language specification belongs in
[../spec](../spec/README.md). This directory documents compiler implementation
work only.

## Quick Start

From the repository root:

```sh
./compiler/scripts/verify.sh
```

From this directory:

```sh
cargo test
```

Run a local Nocter program through the repository-local distribution image:

```sh
NOCTER_HOME="$PWD/../.nocter" cargo run -- run ../example.nct
```

Rust and Cargo are development-time dependencies. A released Nocter archive is
intended to contain a runnable `nocter` binary plus `.nocter/std`, without
requiring Rust, LLVM, `clang`, `as`, `ld`, or an external runtime from users.

## Developer Documents

- [Compiler Docs Index](docs/README.md): where each compiler document belongs.
- [Architecture](docs/architecture.md): compiler pipeline, module ownership, and
  phase boundaries.
- [Implementation Status](docs/implementation-status.md): current parsed,
  checked, buildable, runtime, CLI, std, and LSP capability.
- [v0 Closure Definition](docs/v0-closure.md): fixed completion gates for v0.
- [Backend V0](docs/backend-v0.md): native ARM64 Darwin backend and ABI lowering
  design.
- [Std Runtime Status](docs/std-runtime-status.md): implementation status of the
  distributed `.nocter/std` tree relative to the public spec.
- [Interpolation Lowering](docs/interpolation-lowering.md): deferred lowering
  plan for bare string interpolation.
- [Roadmap](docs/roadmap.md): recommended implementation order.
- [Maintenance Policy](docs/maintenance.md): long-running development rules.
- [TODO](TODO.md): short-term handoff state for the next compiler session.

## Main Layout

```text
compiler/
├── AGENTS.md
├── README.md
├── TODO.md
├── Cargo.toml
├── docs/
├── scripts/
├── src/
└── tests/
```

- `src/` contains the compiler implementation.
- `tests/` contains user-visible CLI, runtime, distributed-home, LSP, and corpus
  integration tests.
- `docs/` contains implementation-facing design and status documents.
- `scripts/` contains developer verification and local-install helpers.
