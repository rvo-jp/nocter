# Nocter Development

This directory is the development root for Nocter. It contains the Rust
bootstrap compiler, the tracked standard-library source, release packaging
inputs, and implementation-facing documentation.

The user-facing language introduction and getting-started flow belong in the
repository root [README.md](../README.md). The language specification belongs
in [../spec](../spec/README.md). This directory documents development work only.

## Quick Start

From the repository root:

```sh
./development/compiler/scripts/verify.sh
```

From this directory:

```sh
cargo test --manifest-path compiler/Cargo.toml
```

From `development/compiler/`:

```sh
cargo test
```

From the repository root, run a local Nocter program through the
repository-local distribution image:

```sh
./development/compiler/scripts/package-local-release.sh
./dist/.nocter/nocter example.nct
```

Rust and Cargo are development-time dependencies. A released Nocter archive is
intended to contain one `.nocter/` home with a runnable `nocter` binary plus
`std/`, without
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
  tracked standard library and distributed `std/` tree relative to the public
  spec.
- [Interpolation Lowering](docs/interpolation-lowering.md): deferred lowering
  plan for bare string interpolation.
- [Roadmap](docs/roadmap.md): recommended implementation order.
- [Maintenance Policy](docs/maintenance.md): long-running development rules.
- [TODO](TODO.md): short-term handoff state for the next compiler session.

## Main Layout

```text
.
├── README.md
├── AGENTS.md
├── TODO.md
├── compiler/
│   ├── Cargo.toml
│   ├── scripts/
│   ├── src/
│   └── tests/
├── std/
├── packaging/
└── docs/
```

- `compiler/src/` contains the compiler implementation.
- `compiler/tests/` contains user-visible CLI, runtime, distributed-home, LSP,
  and corpus integration tests.
- `docs/` contains implementation-facing design and status documents.
- `compiler/scripts/` contains developer verification and local release packaging
  helpers.
- `std/` contains the canonical standard-library source.
- `packaging/` contains release metadata copied into generated Nocter homes.
