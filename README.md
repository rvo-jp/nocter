# Nocter

Nocter is a statically typed, value-centered systems language that aims to
produce native executables directly from `.nct` source files.

The first implementation target is `arm64-darwin`. The compiler currently emits
ARM64 Mach-O executables itself; it does not route normal user builds through
LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or an external runtime
library.

Nocter is still pre-v0. The repository is useful for language design,
compiler development, and experimenting with the current buildable subset. It
is not yet a stable general-purpose release.

## Direction

Nocter is designed around:

- direct native output from one self-contained compiler
- explicit modules derived from file paths
- private-by-default declarations with `pub` for public API
- `struct`, `enum`, `func`, `impl`, `method`, and contract-only `interface`
- ownership, borrowing, and deterministic `drop`
- recoverable failure through `T!` and absence through `T?`
- a small standard library written in Nocter behind trusted primitive
  boundaries
- one canonical source style for humans, editors, and AI tools

Nocter v0 deliberately does not include a runtime GC, class inheritance, trait
code reuse, package management, external linker integration, Linux or Windows
backends, or a stable public binary ABI.

## Current Capability

The current compiler can parse, check, build, and run a meaningful v0 subset on
`arm64-darwin`.

Working areas include:

- `main.nct` as the default root file for `check`, `build`, and `run`
- root-file `func main` as the executable entry point
- `use` declarations for ordinary modules and `std/...`
- scalar values, `&str`, slices, optionals, fallible values, and selected
  aggregate values
- `if` and `match` as value-producing expressions
- `otherwise` for optional fallback and optional-side early exit
- postfix `?` propagation for both `T?` and `T!`
- ownership checks for moves, drops, borrows, and common aggregate paths
- direct and indirect aggregate ABI lowering for the current runtime subset
- a distributable `.nocter/std` tree with `Error`, `String`, `Vec`, `File`,
  allocator, formatting, process, and OS-internal modules
- CLI diagnostics, JSON diagnostics, formatting, token/AST JSON output, and a
  basic LSP server

The buildable subset is intentionally smaller than the checkable language.
Unsupported runtime forms should be rejected before machine-code emission with
source-backed diagnostics. See
[compiler/docs/implementation-status.md](compiler/docs/implementation-status.md)
for the implementation boundary.

## Quick Start From Source

Rust and Cargo are required only for compiler development.

```sh
cd compiler
cargo test
```

Run the checked-in example with the repository-local Nocter home:

```sh
cd compiler
NOCTER_HOME="$PWD/../.nocter" cargo run -- run ../example.nct
```

Run the broader compiler verification suite:

```sh
./compiler/scripts/verify.sh
```

Released Nocter archives are intended to contain a `.nocter/` root with the
compiler binary, release metadata, and standard-library source. Users of a
released archive should not need a Rust toolchain.

## Repository Layout

```text
.
├── README.md
├── example.nct
├── spec/
├── compiler/
└── .nocter/
```

- `README.md` is the public project entry point. It explains what Nocter is and
  links to the right detailed documents.
- `spec/` is the authoritative Nocter language specification for Nocter users,
  library authors, editor/tool authors, and AI assistants.
- `compiler/` contains the Rust bootstrap compiler and implementation-facing
  documentation for compiler engineers and AI coding agents.
- `.nocter/` is the repository-local distribution image used by tests and local
  development. It contains the current standard library under `.nocter/std`.
- `example.nct` is a small current-syntax example, not the full language
  specification.

## Documentation Map

- [Language Specification](spec/README.md): syntax, type system, semantics,
  standard library, CLI contract, diagnostics, and tooling behavior.
- [Compiler Documentation](compiler/README.md): architecture, implementation
  status, backend design, maintenance policy, roadmap, and handoff notes.
- [Nocter v0 Contract](spec/00-v0-contract.md): user-facing v0 language
  boundary.
- [Nocter v0 Closure](compiler/docs/v0-closure.md): implementation completion
  gates for compiler work.

The specification is the source of truth for language behavior. Compiler docs
explain how the current implementation reaches or rejects that behavior.
