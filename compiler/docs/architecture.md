# Nocter Compiler Architecture

This document is for developers working on the Nocter compiler implementation.
User-facing project information belongs in the repository root `README.md`.
The language specification belongs in `../../spec/`.

## Goal

The compiler must first compile `.nct` source files directly to `arm64-darwin` Mach-O executables.

The implementation should still keep target-specific code isolated so future targets can be added without rewriting the front end, type checker, ownership checker, or high-level standard-library model.

The initial implementation does not support cross compilation beyond `arm64-darwin`, but it should still model host and target separately. The current development host package is `.nocter/`, which contains the compiler binary for ARM64 macOS, common standard-library sources, and the `targets/arm64-darwin/` overlay for the `arm64-darwin` target.

It must not depend on:

- LLVM
- `clang`
- `as`
- `ld`
- Xcode Command Line Tools
- external runtime libraries

The distributable archive root for the initial host is:

```text
.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
        prelude.nct
        fmt.nct
        io.nct
        mem.nct
        os.nct
        ptr.nct
        string.nct
    targets/
        arm64-darwin/
            std/
                io_impl.nct
                process.nct
                os/
                    macos.nct
        x64-linux/
            std/
        arm64-linux/
            std/
        x64-windows/
            std/
        arm64-windows/
            std/
```

Users normally install that archive root as `~/.nocter/`. `compiler/src/` contains the implementation used to build the `nocter` compiler. `.nocter/` contains the current development host package for the user-facing compiler binary and standard library. It is generated output and is not committed to git.

The Nocter home root must contain `VERSION` and `MANIFEST.json`. `VERSION` is the single-line release version. `MANIFEST.json` records the manifest schema, release, host, default target, implemented targets, compiler path, standard-library path, and archive name. Manifest v1 does not include checksum metadata.

## Implementation Language

The v0 compiler implementation language is Rust.

Rust and Cargo are development-time dependencies only. A Nocter user must be able to download the host archive, install `.nocter/`, and run `.nocter/nocter` without installing Rust, Cargo, LLVM, `clang`, `as`, `ld`, or Xcode Command Line Tools.

The Rust implementation is a bootstrap implementation, not a change to Nocter's self-contained design. The compiler must implement the Nocter front end, Nocter ABI lowering, ARM64 instruction encoding, Mach-O writing, and minimal link-style layout itself. It must not invoke LLVM, use `clang` as a backend, emit `.s` for an external assembler, or delegate executable generation to a system linker.

## Developer Rust Environment

Rust is required only for developers who build or test the Nocter compiler from `compiler/`. It is not required for Nocter users who install a released `.nocter/` archive.

Recommended setup:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install stable
rustup default stable
rustc --version
cargo --version
./compiler/scripts/verify.sh
```

Nightly Rust is not part of the initial plan. `compiler/rust-toolchain.toml` pins the compiler implementation to the stable toolchain and installs `rustfmt` and `clippy` for local quality checks.

If Rust is not installed, language design, specification editing, and `.nct` standard-library design can continue. Rust compilation, `cargo test`, `compiler/Cargo.lock` generation, and `.nocter/nocter` binary generation require Rust and Cargo.

Standard verification from the repository root:

```sh
./compiler/scripts/verify.sh
```

The script runs `cargo check`, `cargo test`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` inside `compiler/`.

Crate policy:

- Prefer Rust `std` and small local modules.
- Allow crates only when they do not own Nocter language semantics or output generation.
- CLI argument parsing, JSON emission for diagnostics and manifest handling, and test utilities may use small crates if they do not add runtime prerequisites to the distributed `nocter` binary.
- Do not depend on parser generators, LLVM bindings, assembler libraries, linker libraries, or code-generation frameworks for the v0 compiler pipeline.
- Keep error and diagnostic data structures owned by the compiler so `nocter check --format json` and future `nocter lsp` can share them.

Initial Cargo shape:

- `Cargo.toml` lives at `compiler/Cargo.toml`.
- The package name is `nocter`.
- The Rust edition is `2024`.
- The initial implementation is a single crate, not a workspace.
- `src/main.rs` stays thin and only calls into `src/lib.rs`.
- `src/lib.rs` owns the compiler core modules.
- Initial external dependencies are limited to `serde` and `serde_json` for `MANIFEST.json` parsing and future diagnostics JSON.
- `compiler/Cargo.lock` should be committed, but it is still a development artifact and is not part of the user-facing `.nocter/` archive.

Long-term direction: self-hosting. The Rust compiler is the implementation used to reach a correct Nocter compiler and standard library. Once Nocter is mature enough, a Nocter-written compiler can become the primary implementation.

## Compiler Responsibilities

The compiler owns the whole pipeline:

1. lexing
2. parsing
3. AST construction
4. name resolution
5. type checking
6. ownership, borrow, move, and drop checking
7. optional IR generation
8. active target selection, with reserved target names and not-implemented diagnostics
9. Nocter ABI v0 data layout and call lowering
10. target instruction encoding, initially ARM64
11. target executable generation, initially Mach-O
12. minimal link-style layout work when needed

External assemblers and linkers are not part of the design.

## Build V0 Boundary

`nocter build` already runs the native backend path end to end:

```text
SourceMap
    -> lexer/parser
    -> import resolution
    -> type checking
    -> IR lowering
    -> ARM64 Darwin machine code
    -> Mach-O executable image
    -> executable file
```

The current buildable language subset is intentionally smaller than the checkable language subset.
The front end can parse and type-check more Nocter syntax than the backend can lower.

Currently buildable:

- root-file `main` or `--entry <name>`
- entry return types `i32`, `i32!`, and `void`
- literal `i32` returns
- immutable local `let` bindings whose initializer is lowerable as `i32`, annotated `usize`, `bool`, or annotated `&str`
- `void` entry with an empty body or bare `return`
- same-file non-generic tail calls returning `i32` or `bool`
- same-file and loaded imported calls returning `never` in terminal return or expression-statement position
- same-file non-generic normal calls returning `i32` in `let` initializers, `i32` arithmetic and shift expressions using `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`, `i32` comparison operands, and nested scalar call arguments
- same-file and loaded imported non-generic normal calls returning `usize` in annotated `let` initializers, including scalar call arguments
- custom executable output paths through `build -o <path>`
- explicit `--target arm64-darwin` selection for `build`, `run`, and `check`; reserved future targets are recognized but rejected as unimplemented
- same-file non-generic normal calls returning `bool` in `let` initializers, unary-not expressions, bool equality/inequality operands, short-circuit bool value expressions, and terminal `if` conditions
- short-circuit bool expressions can combine `i32` call comparisons with bool calls, such as `if answer() == 42 && ready()` and `let matched = answer() == 42 && ready()`
- `usize` comparisons over literals, locals, and same-file or loaded imported normal calls in lowerable bool expressions and terminal `if` conditions
- nested scalar normal-call arguments such as `let value = outer(inner())`, for `i32`, `usize`, and `bool` parameter positions
- nested scalar tail-call arguments such as `return outer(inner())`, for `i32`, `usize`, and `bool` parameter positions
- static string literals and `&str` parameters as call arguments, passed as `ptr,len` ABI word pairs
- same-file and loaded imported non-generic normal calls returning `&str` in annotated `&str` `let` initializers and as `&str` call or tail-call arguments, with results staged into two local ABI words
- up to 8 ABI argument words across scalar `i32`/`usize`/`bool` and `&str` parameters/call arguments for lowered functions and calls
- reordered parameter arguments are supported for normal calls and tail calls through argument staging
- non-entry functions returning `bool`, `usize`, or direct `&str` literal/parameter/local/tail-call values
- `i32` arithmetic with `+`, `-`, `*`, `/`, and `%` used in lowerable `i32` expressions; addition, subtraction, and multiplication trap on signed overflow, and division and remainder trap on zero divisors and signed division overflow
- `i32` shifts with `<<` and `>>` used in lowerable `i32` expressions; shift counts trap when negative or greater than or equal to 32
- bool `!`, `&&`, `||`, bool equality/inequality over literal/local operands, and `i32` or `usize` comparisons used in lowerable bool expressions
- terminal `if` / `else` statements with bool literal, bool local, bool equality/inequality over literal/local operands, or `i32`/`usize` comparison conditions and direct `i32` or non-entry `bool` returns in both branches
- non-entry `never` functions that end with a lowerable call returning `never`
- the `std/os/macos.trap` and `std/os/macos.unreachable` target primitives as ARM64 `brk #0`
- simple fallible entry success
- simple fallible entry failure through a loaded static `error` constructor call with string code and message literals, where the message may be single-line or multi-line

Currently not buildable even when it may be checkable:

- `var`, reassignment, and general local storage
- general `if`, `while`, `loop`, range `for`, and `match`
- unloaded imported function placeholders
- `usize` arithmetic and `usize` entry return values
- `&str` member operations and view/byte iteration
- interpolated string construction
- optional values
- aggregate values, arrays, views, pointers, methods, traits, generics, ownership lowering, and drop glue

### Backend V0 Register Convention

The `arm64-darwin` backend v0 uses a deliberately small register-only convention while the IR has no stack frame, spill slots, or ABI-complete call lowering.

- scalar `i32` and `bool` values are represented in 32-bit ARM64 `w` registers
- scalar `usize` values are represented in 64-bit ARM64 `x` registers
- `&str` values are represented as two 64-bit ABI words, `ptr` then byte `len`
- `bool` is encoded as `0` for false and `1` for true
- lowered `i32` and `bool` function arguments are passed in `w0` through `w7`, lowered `usize` function arguments are passed in `x0` through `x7`, and lowered `&str` arguments consume two consecutive `x` argument registers at the same ABI word indexes
- lowered function return values are produced in `w0` for `i32`/`bool`, `x0` for `usize`, and `x0,x1` for `&str`
- scalar local bindings use `w9` through `w15` for `i32`/`bool` and `x9` through `x15` for `usize`; framed functions spill scalar locals through 8-byte stack slots
- `w16`/`w17` and `x16`/`x17` are backend scratch registers and may be clobbered by code generation

Tail calls are lowered by loading the callee arguments into `w0` through `w7` or `x0` through `x7` according to each scalar argument type, then branching directly to the target function.
The source-level scalar/view call subset lowers same-file and loaded imported non-generic `i32` calls in `let` initializers, `i32` arithmetic and shift expressions using `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`, `i32` comparison operands, nested normal-call arguments, and nested tail-call arguments, evaluating staged calls left to right into distinct temporary locals.
It also lowers same-file and loaded imported non-generic calls returning `usize` in annotated `let` initializers and `usize` comparison operands, including calls whose parameter list contains `usize`.
Calls in the current buildable subset can receive static string literals, existing `&str` parameters or locals, or staged `&str` normal-call results as `&str` arguments, with each `&str` occupying two ABI argument words.
Non-entry functions can directly return static string literals, `&str` parameters, `&str` locals, or tail calls to `&str` functions, with `&str` returns occupying `x0,x1`.
The frame, spill/reload, and normal-call implementation order is tracked in `backend-v0.md`.
`backend/frame.rs` owns the fixed v0 frame layout planner: frame size, saved `x30` offset, scalar spill-slot offsets, stack-backed argument staging offsets, and reserved aggregate stack slots for the next aggregate lowering step.
Frame-only IR `ReserveAggregateSlot` markers carry ABI `ValueLayout` requests into the planner; the planner collects them from nested instruction lists and catch handlers, deduplicates matching slot indexes, and codegen treats the marker as a no-op after prologue allocation.
IR function signatures preserve ABI-indirect aggregate returns as `Type::Aggregate { layout }`, so lowering can distinguish owned aggregate return shapes before aggregate value construction is buildable.
IR `CallAggregate` lowers normal calls that return indirect aggregate values by passing the reserved destination slot address in ABI register `x8`; source lowering does not emit it yet.
IR `StoreAggregateUsize` writes 8-byte fields either into the current indirect return storage behind `x8` or into a reserved aggregate stack slot, which is the first low-level field-store primitive for `ptr`/`len`/`capacity` style aggregates.
Codegen emits framed prologue/epilogue sequences and normal calls with conservative scalar spill/reload plus stack-backed argument staging.
Backend call patching and function offset registration use an internal `FunctionSymbol` key rather than raw function-name strings, so same-file calls and imported calls use distinct symbol identities before Mach-O branch patching resolves offsets.
Addition and subtraction emission uses ARM64 flag-setting arithmetic and traps on signed overflow.
Multiplication emission computes a signed 64-bit product and traps unless that product exactly fits in `i32`.
Division and remainder emission inserts zero-divisor and signed-overflow trap checks before ARM64 `sdiv`.
Shift emission checks the runtime count before ARM64 variable shift instructions and traps when the count is negative or greater than or equal to the shifted value width.
Unloaded imported placeholders, aggregate values, ownership/drop lowering, and general control-flow call placement remain outside the buildable subset.

Use integration tests for user-visible CLI behavior and backend/unit tests for lower-level encoding and Mach-O layout. When extending build support, first add a small `nocter build` regression case, then expand IR lowering, code generation, and documentation together.

## Suggested Source Layout

The exact implementation layout can evolve, but the first structure should keep each compiler phase separate.

```text
README.md
Cargo.toml
Cargo.lock
rust-toolchain.toml
src/
    main.rs
    lib.rs
    lexer/
    source/
    parser/
    ast/
    resolve/
    typecheck/
    frontend/
    home/
    analysis/
    ir/
    abi/
    target/
        arm64/
        macho/
        primitive/
    driver/
    diagnostics/
    lsp/
```

Responsibilities:

- `source/`: source file loading, UTF-8 decoding, CRLF-to-LF normalization, source maps, canonical absolute paths, and byte-based span conversion for diagnostics and JSON tools.
- `lexer/`: comments, ASCII identifiers, integer/string/byte literals, interpolation-aware string source forms, tokenization for normalized `.nct` source text, and v0 diagnostics for reserved-but-invalid punctuation such as `@`.
- `parser/`: syntax parsing and parser diagnostics.
- `ast/`: source-level syntax tree definitions.
- `resolve/`: imports, canonical absolute source-file identity, path-derived modules, visibility, and name lookup.
- `typecheck/`: type rules, generics, traits, fallible types, optional types, `never` reachability, ownership checks, non-lexical borrow live ranges, field-sensitive borrow checks, provenance checks, and region escape checks.
- `frontend/`: root `.nct` loading, lexing/parsing for semantic checks, recursive import graph loading, canonical-path module de-duplication, synthetic standard prelude insertion, active target overlay lookup, and common Nocter home `std` lookup.
- `home/`: installed Nocter home resolution, `VERSION` reading, `MANIFEST.json` parsing, manifest schema validation, release/host/default-target validation, archive metadata validation, and standard-library directory shape validation.
- `analysis/`: whole-compile-unit semantic analysis that combines per-file resolve and typecheck output into reusable `CompileUnitAnalysis` and `FileAnalysis` records for CLI diagnostics and future LSP features.
- `ir/`: optional lower-level compiler representation if direct AST lowering becomes too tangled. IR call instructions, function definitions, and lowering-time function signature lookup carry a backend-independent `CallTarget` so same-file targets and imported targets do not share raw strings. Lowering indexes root functions as `CallTarget::SameFile` and imported file functions as `CallTarget::Imported { source, name }`, using declaration source identity plus function name. `LoweringContext` can resolve call expressions through resolver output, so direct expression lowering can emit imported call targets. `ir/lower/reachability.rs` owns reachable `CallTarget` collection from lowered instructions. `ir/lower/imported_calls.rs` owns imported call target collection and the current diagnostic for unresolved imported placeholders. Function parameter lowering uses the ABI helper to count ABI words and enforce the eight-register argument window. Function signature indexing also uses ABI classification to retain indirect aggregate returns as `Type::Aggregate { layout }`, while source aggregate value lowering remains disabled. The model also has frame-only aggregate slot reservation, aggregate indirect-return call, and aggregate `usize` field-store instructions carrying ABI layout/offset information; these exist to bridge aggregate local/return lowering into backend frame planning, call emission, and initial field stores.
- `abi/`: Nocter ABI v0 classification, data layout, aggregate layout, call lowering rules, return lowering rules, and drop glue rules. The current helper layer maps resolved source type expressions for primitives, raw pointers, borrows, `&str`, slices, aliases, and non-generic structs into ABI types, computes struct field offsets, applies the 16-byte direct/indirect value classification, classifies function signatures into ABI parameters plus `void`/`never`/value returns, counts parameter ABI words including indirect-argument pointer words, and exposes indirect-return detection. Later lowering should reuse it instead of duplicating field offset or signature classification logic in IR or backend code.
- `target/`: target-specific lowering and output.
- `target/arm64/`: ARM64 instruction selection and binary instruction encoding.
- `target/macho/`: Mach-O headers, segments, sections, symbols, relocations if needed, and executable layout.
- `target/primitive/`: lowering and validation for target-independent core primitives and the closed primitive set of the active target.
- `driver/`: command-line flow for `--version`, `doctor`, `build`, `run`, `check`, `fmt`, `tokens`, `ast`, and `lsp`, target registry lookup, active target selection, temporary executable handling for `run`, and stdout/stderr discipline.
- `diagnostics/`: structured errors with source spans, display paths, canonical absolute paths, human rendering, and the `nocter.diagnostics` JSON envelope for `--format json`.
- `driver/lsp/`: language-server entry point and protocol support. It reuses the compiler front end, resolver, type checker, analysis data, and diagnostics instead of reimplementing language semantics for editors. `mod.rs` owns the server loop, request routing, notification handling, and feature orchestration. `protocol.rs` owns JSON-RPC framing and LSP position/range helpers, `documents.rs` owns open document state and URI/path handling, `analysis.rs` owns open-document compile-unit analysis setup and uses the same Nocter home resolution as the CLI, `diagnostics.rs` owns publishDiagnostics conversion and stale clearing payloads, `semantic.rs` owns semantic token classification and encoding, `hover.rs` owns hover contents, hover symbol collection, documentation attachment, and resolved-reference hover labels, `definition.rs` owns definition response construction and LSP Location conversion, `completion.rs` owns keyword and resolved symbol completion item construction, and `symbols.rs` owns document symbol construction. v0 supports initialize/shutdown/exit, workspace root recording from `workspaceFolders` or `rootUri`, full-document sync, stale version rejection for older `didChange` notifications, open-document import reuse, stale diagnostic clearing, UTF-16 diagnostic positions, publishDiagnostics, semantic tokens, hover, definition, document symbols, and basic completions. Rename, references, formatting integration, and richer type-aware hovers are later features.

The v0 driver should not implement package manifests, project-root discovery, package registries, workspaces, separate compilation, or incremental module artifacts. `nocter build app.nct`, `nocter run app.nct`, and `nocter check app.nct` each receive one root file and operate on the whole reachable compile unit. `nocter fmt app.nct` receives one source file and formats only that file without following imports. `nocter doctor` validates the active Nocter home and must not execute user code.

Editor and AI tooling should treat the compiler as the source of semantic truth. A VS Code TextMate grammar may provide syntax highlighting, and AI tools may generate code, but formatting, import resolution, type checking, borrow checking, tokenization, AST shape, and diagnostics belong in the Nocter toolchain: `nocter fmt`, `nocter check --format json`, `nocter tokens --format json`, `nocter ast --format json`, and `nocter lsp`.

## Source And JSON Span Model

Internal source identity:

```text
SourceId
SourceMap
ByteSpan { source, start, end }
```

Rules:

- `SourceId` is an internal integer ID and must not appear in public JSON.
- `SourceMap` owns source text, display paths, canonical absolute paths when known, and line-start offsets.
- `ByteSpan.start` and `ByteSpan.end` are UTF-8 byte offsets after CRLF-to-LF normalization.
- `ByteSpan.end` is exclusive.
- Internal compiler phases should pass `ByteSpan`, not line/column pairs.
- JSON output derives line and column information from `SourceMap` at the boundary.

Public JSON spans use this shape:

```json
{
  "file": "app.nct",
  "absolute_path": "/Users/me/project/app.nct",
  "start_byte": 0,
  "end_byte": 7,
  "start_line": 1,
  "start_column_byte": 1,
  "end_line": 1,
  "end_column_byte": 8
}
```

`start_column_byte` and `end_column_byte` are 1-based UTF-8 byte columns, not UTF-16 LSP columns. `nocter lsp` is responsible for converting compiler spans to the client position encoding.

Source loading:

- `SourceMap::load_file(display_path)` is the shared source-loading API for `check`, `tokens`, `ast`, `build`, and `run`.
- The display path is the path passed by the CLI or import resolver.
- `absolute_path` is filled from `canonicalize()` when loading succeeds.
- Files are read as bytes and decoded as UTF-8 before lexing.
- CRLF is normalized to LF before the source is registered.
- A bare `\r` is rejected during source loading.
- Lexer and parser phases receive only normalized UTF-8 source text.

## Lexer V0 Boundary

The v0 lexer receives a `SourceId` and normalized UTF-8 source text from `SourceMap`.

Output:

- `Vec<Token>`
- `Vec<Diagnostic>`
- `TokensEnvelope` conversion for `nocter tokens app.nct --format json`

Token stream rules:

- Reserved keywords are emitted as keyword tokens.
- Newlines are emitted as newline tokens.
- Exactly one EOF token is emitted.
- Comments are discarded and not emitted as tokens.
- LF bytes inside block comments still emit newline tokens to preserve statement separation.
- Token spans are `ByteSpan` values.
- Token JSON uses `kind`, `lexeme`, and the public JSON span shape.
- Literal tokens keep source text; final literal value interpretation belongs to parser, type checking, or later lowering unless the issue is lexical validity.
- Invalid lexical constructs produce diagnostics. The v0 lexer may stop after the first unrecoverable lexical error.

`nocter tokens app.nct --format json` should run:

```text
SourceMap::load_file
    -> lexer
    -> TokensEnvelope
    -> JSON stdout
```

When source loading fails, the command still writes a `nocter.tokens` JSON envelope with an empty `tokens` array and one diagnostic.

## Parser V0 Boundary

The v0 parser receives the lexer token stream and builds the initial typed AST used by later compiler phases. `nocter ast app.nct --format json` converts that typed AST into tooling JSON at the CLI boundary.

Fallible syntax uses `T!`, postfix `expr?`, and `expr catch error { ... }` with built-in `error`. The previous `T ! E`, `try expr`, and `try expr catch name { ... }` forms are no longer accepted by the parser.

Output:

- `Option<AstFile>`
- `Vec<Diagnostic>`
- `AstFile::to_json` conversion for `nocter ast app.nct --format json`

Initial grammar coverage:

- `use std/prelude`
- `from std/io import print`
- `pub from std/string import String`
- `from std/io import File as StdFile`
- `import std/io as io`
- `func main(): i32! { ... }`, plus infallible `func main(): i32 { ... }`, `func main(): void { ... }`, and custom `--entry <name>` functions
- `func name(...): Type { ... }`
- `trait Name { method (...)... }`
- `impl Type { func ... method ... }`
- `impl Trait for Type { method ... }`
- parameter lists
- generic parameter lists, including inline bounds such as `T: Trait`
- `str`, `&str`, `error`, `[T]`, `&[T]`, `&+[T]`, `[T; N]`, `T?`, and `T!` type syntax
- blocks
- `return`
- fallible failure through `return error_value`
- `let` and `var` bindings with initializers
- `let name = optional else { ... }` and `var name = optional else { ... }`
- `if condition { ... }` and `if condition { ... } else { ... }`
- `if value is Enum.variant { ... }` enum pattern statements, with optional single-payload pattern bindings
- `if let name = optional { ... }` and `if var name = optional { ... }`
- `else if`, `else if let`, and `else if var` chains, represented internally as nested `if` statements in synthetic else blocks
- `match value { Enum.variant { ... } else { ... } }` enum match statements, with optional single-payload arm bindings and an optional fallback arm
- `while condition { ... }`
- `while let name = optional { ... }` and `while var name = optional { ... }`
- `for name in start..<end { ... }` half-open integer range loops
- `loop { ... }`
- `break` and `continue` statements without labels or values
- postfix fallible propagation `expr?`
- fallible catch expression `expr catch name { ... }`
- call expressions
- member expressions
- index expressions
- `as` type conversion expressions
- grouped expressions
- array literal expressions
- simple named-field struct literal expressions such as `Point{ x: 1 }`
- arithmetic expressions with `+`, `-`, `*`, `/`, and `%`
- shift expressions with `<<` and `>>`
- comparison expressions with `==`, `!=`, `<`, `<=`, `>`, and `>=`
- logical expressions with `&&` and `||`
- prefix logical not expressions with `!`
- prefix numeric negation expressions with `-`
- `??` optional-default expressions
- `target ?{ Enum.variant : value : fallback }` enum pattern conditional expressions
- identifier, integer, string, bool, and `none` expressions
- interpolated string expressions with source-preserving text and expression parts

Parser v0 deliberately does not resolve imports, validate the executable entry signature, type-check expressions, check ownership, or follow imported files. It may stop after the first syntax error. When lexing fails, `nocter ast` returns a `nocter.ast` envelope with `ast: null` and the lexer diagnostics.

The internal typed AST is the compiler data model. `JsonAstNode` is only a CLI/tooling representation and must not become the semantic-analysis input.

`nocter ast app.nct --format json` runs:

```text
SourceMap::load_file
    -> lexer
    -> parser
    -> typed AST
    -> AstEnvelope
    -> JSON stdout
```

## Check V0 Boundary

`nocter check app.nct --format json` uses the same source loading, lexer, parser, and typed AST as `build` and `run`, then runs the first semantic validation pass.

Current semantic coverage:

- executable root has the active entry function, defaulting to `main`
- entry selection through `--entry <name>` validates the selected top-level function
- executable root has exactly one active entry function
- entry return type is `i32!`, `i32`, or `void`
- relative imports starting with `./` or `../` are loaded recursively and lexed/parsed before semantic checks run
- `from ./path import name` resolves imported top-level `func`, `primitive`, `type`, `struct`, and `enum` declarations
- imported `func` and `primitive` signatures are used for direct call checking
- missing imported top-level names from relative imports are diagnosed
- non-relative imports are loaded recursively from the active Nocter home when needed
- `std/...` import paths search `targets/<active-target>/std/` before common `std/`
- `from std/path import name` resolves imported top-level `func`, `primitive`, `type`, `struct`, and `enum` declarations
- imported names must be `pub`, or `pub(nocter)` when the importing file is inside the active Nocter home; private and inaccessible `pub(nocter)` imports are diagnosed
- eligible user project modules synthesize `use std/prelude`
- explicit or synthetic `use std/prelude` loads the prelude and introduces its public declarations and public `pub from` re-exports
- same-file top-level `func`, `primitive`, `type`, `struct`, and `enum` declarations are collected into a resolver-owned symbol table
- duplicate visible names among same-file declarations, explicit imported names, parameters, locals, and catch bindings are diagnosed
- direct calls to same-file functions are resolved and checked for argument count
- direct calls to same-file functions check argument types when both expected and actual types are known
- same-file and imported inherent associated function calls such as `Type.func(args...)` use the associated function signature for argument checking and return type resolution
- same-file and imported inherent method calls such as `value.method(args...)` use the concrete nominal receiver type for argument checking and return type resolution
- same-file inherent associated functions and methods are duplicate-checked per target type; v0 does not support overloads or an associated function and method with the same name
- each loaded file in the reachable compile unit is resolved and checked in its own file scope; executable entry validation runs only for the root file
- inherent method calls support `Self`, `&Self`, and `&+Self` receiver declarations in v0; `&+Self` calls require the receiver expression to be a mutable `var` binding
- inherent `impl` function bodies and method bodies are resolved and checked for calls, returns, fallible propagation, control-flow termination, and local binding types
- inside an inherent `impl`, `Self` in return types, parameter types, receiver types, struct literals, and type conversions resolves to the impl target type
- primitive return type checking for built-in primitive types, nominal struct types, `str`, `&str`, `error`, `[T]`, `&[T]`, `&+[T]`, `[T; N]`, array literals, struct literals, `void`, `never`, `T?`, and the success side of `T!`
- local binding types are tracked inside a callable when they come from literals, struct literals, annotations, parameters, known direct calls, postfix `?`, `catch`, `??`, optional `let ... else`, optional `if let` / `if var`, or optional `while let` / `while var`
- integer literals have type `i32` in v0 checking
- integer literals can be checked against an expected integer type in returns, function arguments, annotated bindings, and array literal elements
- bool literals have type `bool` in v0 checking
- string literals are modeled as built-in `&str` values in v0 checking
- interpolated string expressions are modeled as `String!` and currently accept `&str`, `String`, integer, and `bool` interpolation part types
- `Enum.variant` and `Enum.variant(args...)` construct enum values and check declared variant payload arity and payload types
- `as` type conversion expressions require lossless integer conversion and return the target type
- arithmetic expressions return the resolved integer operand type and require matching integer operands
- shift expressions return the left integer operand type and require an integer shift count
- comparison expressions return `bool`; equality is checked for supported `bool`, integer, and `&str` operands, and ordering requires matching known integer operand types
- logical expressions return `bool` and require `bool` operands
- prefix logical not expressions return `bool` and require a `bool` operand
- prefix numeric negation expressions return the operand type and require a signed integer operand
- `[T]` parses as built-in unsized array data syntax; `&[T]` and `&+[T]` parse as readonly/readwrite array slice syntax
- `[T; N]` parses as built-in fixed-size array type syntax, and `[a, b, c]` infers `[T; N]`
- index expressions check `[T; N]`, `&[T]`, `&+[T]`, and `&str` targets with integer indexes
- struct literals check the target type, required fields, duplicate fields, unknown fields, hidden fields, and field initializer types
- `if` statement conditions must have type `bool`
- `if ... else`, `if is ... else`, and `if let ... else` statements count as terminating when both branches terminate; parser/check v0 currently recognizes `return`, nested terminal `if ... else`, nested terminal `if is ... else`, nested terminal `if let ... else`, `loop`, and terminal `match ... else` as terminating forms
- `if is` targets must have a known enum type when the target type is known
- `if is` patterns must name the same enum type as the target and a variant declared by that enum
- `if is` payload bindings must match the variant payload shape; parser/check v0 supports no payload or one payload binding
- `if is` exposes the payload binding only inside the then block
- `if let` and `if var` require a known `T?` initializer when the initializer type is known
- `if let` and `if var` expose the contained `T` type only inside the then block
- `match` statement targets must have a known enum type when the target type is known
- `match` arm patterns must name the same enum type as the target and a variant declared by that enum
- `match` arm payload bindings must match the variant payload shape; parser/check v0 supports no payload or one payload binding
- `match` supports one optional `else` fallback arm, and `else` must be the last arm
- `match ... else` counts as terminating when every explicit arm and the `else` arm terminate; `match` without `else` is not treated as terminating because exhaustiveness checking is deferred
- `?{}` pattern conditional targets must have a known enum type when the target type is known
- `?{}` arms must name the same enum type as the target and a variant declared by that enum
- `?{}` arm payload bindings must match the variant payload shape; parser/check v0 supports no payload or one payload binding
- `?{}` requires a fallback `: expression` arm in v0, and that fallback must be the last arm
- `?{}` arm expressions must be assignable to the fallback arm type
- `while` statement conditions must have type `bool`
- `while let` and `while var` require a known `T?` initializer when the initializer type is known
- `while let` and `while var` expose the contained `T` type only inside the loop body
- range `for` bounds must be matching integer types, with integer literals contextually checked against the other bound type
- range `for` exposes the loop variable inside the loop body with the resolved range bound type
- `loop` bodies are checked as loop bodies and count as terminating when the body itself terminates
- `break` and `continue` are valid only inside loop bodies
- annotated bindings check their initializer type when both sides are known
- same-file function call expressions use the callee return type
- postfix `?` and `catch` expressions unwrap the success side of known `T!` expressions
- `return expr` in a function returning `T!` is a failure return when `expr` has type `error` when both sides are known
- `error!` is rejected because `return error_value` would be ambiguous between success and failure
- built-in `error.code` and `error.message` fields have type `&str`; other `error` fields are diagnosed
- direct struct field access resolves field types for known struct values and diagnoses unknown fields
- `let ... else` and `var ... else` require a known `T?` initializer when the initializer type is known
- `let ... else` and `var ... else` expose the contained `T` type on the continuing path
- `let ... else` and `var ... else` require an `else` block that terminates; parser/check v0 currently recognizes `return` as a terminating form
- postfix `?` is diagnosed when used in a non-fallible current callable
- `return` expression type must match the declared success return type when both sides are known
- bare `return` is valid only for `void` success returns
- non-`void` functions and `i32` / `i32!` entry functions must not fall through without an explicit return or failure

The current `check` implementation loads imported files, applies import aliases, registers namespace aliases as visible names, and semantic-checks each reachable file in its own file scope, but it does not resolve namespace member access, full alias-target expansion, enum payload fields, trait method calls, trait obligations, receiver move semantics, borrow lifetimes, perform full block control-flow analysis beyond last-statement `return`, check ownership, select a target beyond the default active target, or lower code. Unknown expression types are not diagnosed until import resolution and full type checking exist.

`nocter check app.nct --format json` runs:

```text
SourceMap::load_file
    -> lexer
    -> parser
    -> typed AST
    -> synthetic standard prelude insertion for eligible user project modules
    -> recursive relative and non-relative import loading and parsing
    -> per-file same-file, relative imported, and Nocter-home imported top-level symbol resolver
    -> CompileUnitAnalysis with one FileAnalysis per loaded file
    -> executable-root entry validation
    -> per-file call validation for known functions and methods
    -> per-file basic return checking
    -> stable diagnostic ordering by loaded file, primary span, diagnostic code, and message
    -> DiagnosticsEnvelope
    -> JSON stdout
```

If source loading, lexing, or parsing fails, `check` returns a `nocter.diagnostics` envelope with those diagnostics and does not run semantic checks.

`CompileUnitAnalysis` is the `analysis/` module's semantic-analysis result for the whole reachable compile unit. Each `FileAnalysis` keeps the file AST, its file-scoped `ResolveOutput`, that file's diagnostics, and whether the file is the executable root. Flattened diagnostics are sorted by loaded file order, primary span start byte, primary span end byte, diagnostic code, and message. This keeps the command-line checker aligned with future LSP features such as hover, completion, and definition lookup, where editor features need the semantic state for a specific file rather than only a flattened diagnostics list.

The first standard-library source files are `.nocter/std/prelude.nct`, `.nocter/std/string.nct`, `.nocter/std/fmt.nct`, `.nocter/std/mem.nct`, `.nocter/std/ptr.nct`, `.nocter/std/os.nct`, `.nocter/std/io.nct`, `.nocter/targets/arm64-darwin/std/io_impl.nct`, `.nocter/targets/arm64-darwin/std/process.nct`, and `.nocter/targets/arm64-darwin/std/os/macos.nct`. They currently form a Parser v0-readable skeleton for the synthetic user prelude, owning string type, explicit formatting append boundary, initial memory API, core pointer primitive boundary, common OS error model, user-facing I/O errors, `File`, `stdout`, `stderr`, `print`, macOS raw file-descriptor helpers, macOS process API placeholders, and macOS primitive boundary. `std/string.String` is an ordinary standard-library struct with private `ptr`, `len`, and `capacity` fields; `empty()` builds the zero-capacity value through the restricted `std/ptr.from_addr` primitive, while allocation-backed construction and mutation still report unsupported errors. `std/process.abort` and the placeholder `exit` terminate through the target `trap` primitive today. Future target support should add target overlays without changing ordinary user-facing APIs.

The target-independent core pointer primitive set is:

```text
std/ptr.addr
std/ptr.from_ref
std/ptr.from_ref_mut
std/ptr.from_addr
```

`std/ptr.from_addr` is restricted to modules inside the active Nocter home. Project modules must receive a diagnostic if they call it.

The initial `arm64-darwin` primitive set is deliberately small:

```text
std/os/macos.syscall0
std/os/macos.syscall1
std/os/macos.syscall2
std/os/macos.syscall3
std/os/macos.syscall4
std/os/macos.syscall5
std/os/macos.syscall6
std/os/macos.trap
std/os/macos.unreachable
```

The compiler should validate primitives by module path, name, and exact signature. `print`, `args`, `env`, `cwd`, `exit`, file APIs, allocator APIs, `String`, and `Buffer` are standard-library APIs, not compiler primitives.

Future typed wrappers such as raw file, process, allocation, or memory-map helpers should be ordinary Nocter APIs in common `std/` or the active target overlay. The normal implementation path is to grow the standard library on top of the closed primitive set, not to add compiler primitives for each OS operation. User project modules remain outside the primitive declaration boundary.

Initial `std/io.nct` should expose `File`, `File.open`, `File.read`, `File.write`, `File.write_text`, `stdout`, `stderr`, and `print`. Fallible APIs return `T!` and fail with built-in `error`; common classification names such as `Error` and `ErrorCode` belong to `std/prelude` / `std/error`, not to compiler special cases. File has a private close-on-drop state so `File.open` can create an owned handle whose drop closes it while `stdout` and `stderr` return borrowed process standard streams. Target-dependent raw file-descriptor helpers live behind `pub(nocter)` in the active target overlay, currently `.nocter/targets/arm64-darwin/std/io_impl.nct`.

OS error flow belongs in the standard library:

```text
std/os/macos.SyscallResult
std/os/macos.Errno
std/os.OSError
built-in error
```

The compiler should not special-case any of those names.

Reserved target overlay directories may exist before implementation:

```text
.nocter/targets/x64-linux/std/
.nocter/targets/arm64-linux/std/
.nocter/targets/x64-windows/std/
.nocter/targets/arm64-windows/std/
```

These directories are placeholders. The driver must not treat them as implemented targets until the target registry marks the backend, executable writer, primitive set, and target standard-library overlay as implemented. A request such as `--target x64-linux` should fail with a target-selection error while the name remains recognized.

Standard-library resolution should search the active target overlay before the common standard library:

```text
<NOCTER_HOME>/targets/<active-target>/std/
<NOCTER_HOME>/std/
```

Both roots map to the `std/...` import path namespace.

Nocter home resolution is deliberately narrow:

1. `NOCTER_HOME`, if set.
2. Otherwise the parent directory of the resolved real path of the running `nocter` executable.

The driver must not automatically search `cwd/.nocter` or `~/.nocter`. This avoids silently mixing a project-local development package with an installed release.

## Implementation Order

The implementation should keep the self-contained goal intact from the beginning.

Milestone grouping:

- Milestone 0: Cargo skeleton, `nocter --version`, `nocter doctor` skeleton, Nocter home resolution, and `VERSION` / `MANIFEST.json` validation shape.
- Milestone 1: source file loading, `SourceId`, source map, source spans, diagnostics, lexer, lexer tests, and `nocter tokens app.nct --format json`.
- Milestone 2: parser, AST, parsing `func main(): i32`, and `nocter ast app.nct --format json`.
- Milestone 3: basic type checking and executable entry validation.
- Milestone 4: ARM64 encoder, minimal Mach-O writer, and a constant-return executable.

1. lexer
2. parser
3. AST
4. diagnostics with source spans
5. Nocter home resolution from `NOCTER_HOME` or the resolved `nocter` executable path
6. `VERSION` and `MANIFEST.json` loading and validation
7. target registry with `arm64-darwin` implemented and future targets reserved
8. active target value, defaulting to host target `arm64-darwin`
9. path-derived module loading
10. name resolution and visibility
11. basic type checking
12. Nocter ABI v0 data layout for primitives, pointers, borrows, structs, enums, `T?`, and adopted `T!`
13. executable entry validation, allowing `i32!`, `i32`, and `void` entry return types
14. ARM64 instruction encoder
15. minimal Mach-O writer
16. compile a `func main(): i32!` / `func main(): i32` returning a constant
17. string literal placement
18. Nocter ABI v0 function call and return lowering
19. statement control flow: `if`, `match`, `while`, `loop`, range `for`, `break`, `continue`
20. fallible and optional control flow
21. `never` and unreachable-code diagnostics
22. ownership, move, borrow, and drop checks
23. drop glue generation using Nocter ABI v0
24. region scopes and escape diagnostics
25. initial `.nocter/std/prelude.nct`
26. initial `.nocter/std/string.nct`
27. initial `.nocter/std/mem.nct`
28. initial `.nocter/std/ptr.nct`
29. initial `.nocter/std/os.nct`
30. initial `.nocter/std/io.nct`
31. initial `.nocter/targets/arm64-darwin/std/process.nct` with process context APIs and termination APIs
32. initial `.nocter/targets/arm64-darwin/std/os/macos.nct`
33. core pointer primitive validation and lowering for `std/ptr`
34. closed target primitive set validation for `std/os/macos.syscall0..6`, `trap`, and `unreachable`
35. primitive lowering for the active target
36. imports from the active target overlay and common Nocter home `std`
38. standard-library growth
39. `nocter --version` reporting release, host, and default target
40. `nocter doctor` validating Nocter home metadata and directory structure
41. `nocter run app.nct` using a temporary Mach-O executable and the same code path as `build`
42. `nocter check --format json` using compiler-owned diagnostics
43. `nocter fmt app.nct` using the parser and official source style
44. `nocter tokens app.nct --format json` using the compiler lexer
45. `nocter ast app.nct --format json` using the compiler parser
46. `nocter lsp` reusing the compiler front end and semantic checks

## Design Constraints

Nocter should prefer language and standard-library mechanisms over compiler magic. The compiler must not special-case ordinary names such as `print`, `args`, `env`, `cwd`, `exit`, `abort`, `File`, `String`, `Option`, or `Result`.

Exceptions are syntax and core type forms adopted by the language, such as:

- active executable entry selection
- `T?`
- `T!`
- `error`
- `str`
- `[T]`
- `&str`
- `&[T]`
- `&+[T]`
- `return none`
- `return error`
- postfix `?`
- `catch`
- `never`
- `region ... using`
- `primitive`

The standard library may use typed `primitive` declarations as the low-level boundary. Arbitrary inline ARM64 `asm` is not part of the initial language design.

## Testing Direction

Compiler tests should cover observable behavior at each stage:

- lexer token streams
- parser AST shape
- resolver errors
- type checker errors
- ownership and borrow errors
- ARM64 instruction bytes
- Mach-O structural validity
- end-to-end executable generation on Apple Silicon macOS

Tests must not require `clang`, `as`, or `ld` to validate normal compiler output. Tools that inspect Mach-O files may be useful during development, but they must not become required parts of the compiler pipeline.
