# Nocter Implementation Status

This document summarizes the current compiler implementation. It is not the
language specification. Normative source-language rules live in
[../../spec](../../spec/README.md). The fixed implementation completion gates
live in [v0-closure.md](v0-closure.md).

## Status Terms

- `specified`: described in `spec/`
- `parsed`: represented in the AST
- `checked`: resolved, typechecked, ownership-checked, or diagnosed before
  lowering
- `buildable`: lowerable through IR and the native backend
- `runtime`: has meaningful native behavior on `arm64-darwin`
- `check-only`: intentionally present for type/API shape, but not runtime-shipped

## Current Summary

Nocter is currently a small native compiler, not just a frontend prototype.
It can load a v0 compile unit, typecheck a meaningful language subset, lower the
runtime subset to IR, emit ARM64 Darwin machine code, write a Mach-O executable,
and run that executable.

The buildable subset remains narrower than the checked subset. Code outside the
runtime subset should fail through buildability diagnostics before IR or backend
emission.

## Implemented Capability

| Area | Current state |
|---|---|
| Entry and CLI root | `build`, `run`, and `check` use `main.nct` when no file is provided. The root-file `func main` is the executable entry. `fmt` still requires an explicit file. `--entry` is removed. |
| Modules and `use` | File-start bare, selected, grouped, aliased, relative, absolute, source-root, Nocter-home, and `pub use` forms are implemented. Block-start non-`pub` `use` is implemented as lexical, compile-time-only scope. Legacy `import` / `from` forms are removed syntax. |
| Target and distribution | The active Nocter home comes from `NOCTER_HOME`, otherwise the compiler executable's parent. Target-dependent std declarations use `#target("arm64-darwin")` inside stable std files. |
| Scalars and strings | `i32`, `usize`, `u8`, `bool`, `void`, `never`, `&str`, string literals, selected arithmetic, comparisons, bool operators, shifts, and runtime trap checks are implemented in the buildable subset. |
| Blocks and control expressions | Blocks can produce a value from the final expression. `if` and `match` can be value expressions in the supported subset. Loops remain statement-oriented in v0. |
| Errors and optionals | `T!`, `T?`, postfix `?`, postfix `!`, `catch`, `none`, and `otherwise` are parsed and checked. Scalar/view and supported aggregate paths build and run. Nested fallible/optional return shapes remain limited. |
| Struct aggregates | Struct literals, fields, copies, explicit moves, direct and indirect aggregate parameters and returns, call-result slots, selected assignments, replacement drops, and cleanup paths are implemented for the current subset. |
| Enum values | Payloadless enum tag equality, `if is`, and `match` are runtime-shipped. Payload enum construction and checking exist in the frontend; broad payload pattern lowering is still not runtime-shipped. |
| Ownership and drop | The typechecker rejects common use-after-move, double move/drop, invalid drop, borrow conflicts, escaping local borrows, and implicit non-copy aggregate copies. Lowering inserts drop glue for the documented aggregate/control-flow subset. |
| Methods and `self` | Inherent associated functions, `method &self`, `method &+self`, consuming receiver syntax, `drop &+self`, and method lookup are implemented for the current call subset. |
| Interfaces | Contract-only `interface` declarations and explicit structural `impl Interface for Type` checks are frontend-shipped. Interface values, dispatch, generic bounds, and code reuse are not part of v0. |
| Generics | Generic structs, functions, impl methods, associated functions, enum checks, aliases, and concrete specializations are implemented for the current scalar/view/aggregate subset. Unspecialized reachable generic calls are rejected before backend emission. |
| Slices and vectors | Scalar, `&str`, and current copy-aggregate slice indexing and assignment paths are supported. `Vec<T>` supports scalar, `&str`, and promoted copy-aggregate element storage paths. |
| Standard library | `.nocter/std` contains `error`, `string`, `fmt`, `mem`, `io`, `process`, `vec`, `ptr`, `os`, and `prelude`. See [Std Runtime Status](std-runtime-status.md). |
| CLI diagnostics | Text and JSON diagnostics are source-backed where possible. Command-line, filesystem, target, Nocter-home, and formatting diagnostics have stable user-facing messages. |
| LSP | Basic LSP supports initialize, shutdown, full document sync, diagnostics, semantic tokens, hover, definition, references, document symbols, and position-aware basic completion using compiler facts. Block-scope `use` visibility is reflected in completion, references, and semantic tokens. |

## Runtime-Shipped Standard Library

The current distributed std supports:

- `std/error`: built-in `error` construction through ordinary public names
- `std/string`: owned `String`, explicit allocation, views, metadata, reserve,
  clear, append support, bytes view, and drop
- `std/fmt`: append helpers for `&str`, `String`, `i32`, `usize`, and `bool`
- `std/mem`: page allocator, raw buffers, and byte-slice views
- `std/io`: `File`, open/read/write/write_text, stdout/stderr, and `print`
- `std/process`: `exit`, `abort`, `cwd`, and `args`
- `std/vec`: construction, capacity, push, from-slice, views, mutation through
  views, clear, reserve, and storage release for shipped element kinds
- `std/ptr`: narrow pointer address and std-internal raw storage boundaries

`std/process.env` is check-only. It reserves the future `&str?!` shape but is
not runtime-shipped.

## Known Runtime Gaps

- targets other than `arm64-darwin`
- broad control-flow lowering outside the documented subset
- payload-carrying enum `if is` and `match` runtime lowering
- array literal storage and general array runtime behavior
- broad pointer dereference and user memory mutation APIs
- broad view iteration
- bare string interpolation lowering without an explicit allocator source
- non-copy aggregate `Vec<T>` element storage and per-element drop glue
- insertion/removal/iterator collection APIs
- interface dispatch, interface-bound generics, embedding, and trait-style code
  reuse
- package management, separate compilation, incremental compilation, debug info,
  optimization, dynamic linking, and stable public binary ABI

## Verification

The full v0 closure suite is listed in [v0-closure.md](v0-closure.md). For a
broad compiler change, run the relevant subset of:

```sh
cargo fmt --manifest-path compiler/Cargo.toml --check
cargo test --manifest-path compiler/Cargo.toml --lib
cargo test --manifest-path compiler/Cargo.toml --test cli_build
cargo test --manifest-path compiler/Cargo.toml --test cli_run
cargo test --manifest-path compiler/Cargo.toml --test distributed_home
cargo test --manifest-path compiler/Cargo.toml --test cli_fmt
cargo test --manifest-path compiler/Cargo.toml --test cli_lsp
cargo test --manifest-path compiler/Cargo.toml --test example_corpus
```

Documentation-only changes usually need only formatting verification unless
they alter examples, CLI contracts, or generated outputs.
