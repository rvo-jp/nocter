# Nocter Compiler Development Notes

This document is for developers working on the Nocter compiler implementation. User-facing project information belongs in the repository root `README.md`. The language specification belongs in the repository root `SPEC.md`.

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
        io.nct
        mem.nct
        os.nct
        ptr.nct
        string.nct
        view.nct
    targets/
        arm64-darwin/
            std/
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
- `lexer/`: comments, ASCII identifiers, integer/string/byte literals, tokenization for normalized `.nct` source text, and v0 diagnostics for reserved-but-invalid punctuation such as `@`.
- `parser/`: syntax parsing and parser diagnostics.
- `ast/`: source-level syntax tree definitions.
- `resolve/`: imports, canonical absolute source-file identity, path-derived modules, visibility, and name lookup.
- `typecheck/`: type rules, generics, traits, fallible types, optional types, `never` reachability, ownership checks, non-lexical borrow live ranges, field-sensitive borrow checks, provenance checks, and region escape checks.
- `ir/`: optional lower-level compiler representation if direct AST lowering becomes too tangled.
- `abi/`: Nocter ABI v0 classification, data layout, aggregate layout, call lowering rules, return lowering rules, and drop glue rules.
- `target/`: target-specific lowering and output.
- `target/arm64/`: ARM64 instruction selection and binary instruction encoding.
- `target/macho/`: Mach-O headers, segments, sections, symbols, relocations if needed, and executable layout.
- `target/primitive/`: lowering and validation for target-independent core primitives and the closed primitive set of the active target.
- `driver/`: command-line flow for `--version`, `doctor`, `build`, `run`, `check`, `fmt`, `tokens`, `ast`, and future `lsp`, Nocter home resolution, `VERSION` and `MANIFEST.json` validation, root `.nct` file loading, recursive import graph loading, canonical-path module de-duplication, target registry lookup, active target selection, active target overlay lookup, common Nocter home `std` lookup, temporary executable handling for `run`, and stdout/stderr discipline.
- `diagnostics/`: structured errors with source spans, display paths, canonical absolute paths, human rendering, and the `nocter.diagnostics` JSON envelope for `--format json`.
- `lsp/`: future language-server entry point that reuses the compiler front end, resolver, type checker, ownership checker, and diagnostics instead of reimplementing language semantics for editors.

The v0 driver should not implement package manifests, project-root discovery, package registries, workspaces, separate compilation, or incremental module artifacts. `nocter build app.nct`, `nocter run app.nct`, and `nocter check app.nct` each receive one root file and operate on the whole reachable compile unit. `nocter fmt app.nct` receives one source file and formats only that file without following imports. `nocter doctor` validates the active Nocter home and must not execute user code.

Editor and AI tooling should treat the compiler as the source of semantic truth. A VS Code TextMate grammar may provide syntax highlighting, and AI tools may generate code, but formatting, import resolution, type checking, borrow checking, tokenization, AST shape, and diagnostics belong in the Nocter toolchain: `nocter fmt`, `nocter check --format json`, `nocter tokens --format json`, `nocter ast --format json`, and later `nocter lsp`.

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

Output:

- `Option<AstFile>`
- `Vec<Diagnostic>`
- `AstFile::to_json` conversion for `nocter ast app.nct --format json`

Initial grammar coverage:

- `use std/prelude`
- `from std/io import print`
- `program(): i32 { ... }`
- `func name(...): Type { ... }`
- parameter lists
- `T?` and `T ! E` type syntax
- blocks
- `return`
- `let` and `var` bindings with initializers
- `let name = optional else { ... }` and `var name = optional else { ... }`
- `try expr`
- `try expr catch name { ... }`
- call expressions
- member expressions
- grouped expressions
- `??` optional-default expressions
- identifier, integer, string, and `none` expressions

Parser v0 deliberately does not resolve imports, validate the `program` signature, type-check expressions, check ownership, or follow imported files. It may stop after the first syntax error. When lexing fails, `nocter ast` returns a `nocter.ast` envelope with `ast: null` and the lexer diagnostics.

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

- executable root has a `program` entry
- `func main` without `program` receives a dedicated diagnostic because `main` is an ordinary function name
- executable root has no duplicate `program` entries
- `program` return type is exactly `i32` or `void`
- relative imports starting with `./` or `../` are loaded recursively and lexed/parsed before root semantic checks run
- `from ./path import name` resolves imported top-level `func` declarations and uses their signatures for direct call checking
- missing imported function names from relative imports are diagnosed
- non-relative imports are loaded recursively from the active Nocter home when needed
- `std/...` import paths search `targets/<active-target>/std/` before common `std/`
- `from std/path import name` resolves imported top-level `func` declarations and uses their signatures for direct call checking
- `use std/prelude` loads the imported file in v0, but does not introduce prelude names yet
- same-file top-level `func` declarations are collected into a resolver-owned symbol table
- duplicate visible names among same-file functions, explicit imported names, parameters, locals, and catch bindings are diagnosed
- direct calls to same-file functions are resolved and checked for argument count
- direct calls to same-file functions check argument types when both expected and actual types are known
- primitive return type checking for `i32`, `void`, `never`, `StringView`, `T?`, and the success side of `T ! E`
- local binding types are tracked inside a callable when they come from literals, annotations, parameters, known direct calls, `try`, `try ... catch`, `??`, or optional `let ... else`
- integer literals have type `i32` in v0 checking
- string literals have type `StringView` in v0 checking
- same-file function call expressions use the callee return type
- `try` and `try ... catch` expressions unwrap the success side of known `T ! E` expressions
- `let ... else` and `var ... else` require a known `T?` initializer when the initializer type is known
- `let ... else` and `var ... else` expose the contained `T` type on the continuing path
- `let ... else` and `var ... else` require an `else` block that terminates; parser/check v0 currently recognizes `return` as the terminating form
- `try` without `catch` is diagnosed when used in a non-fallible current callable or with a mismatched known error type
- `return` expression type must match the declared success return type when both sides are known
- bare `return` is valid only for `void` success returns
- non-`void` functions and `program(): i32` must not fall through without an explicit return

The current `check` implementation does not introduce prelude names, resolve imported types or non-function declarations in other files, validate method or associated function calls, perform full block control-flow analysis beyond last-statement `return`, check ownership, select a target beyond the default active target, or lower code. Unknown expression types are not diagnosed until import resolution and full type checking exist.

`nocter check app.nct --format json` runs:

```text
SourceMap::load_file
    -> lexer
    -> parser
    -> typed AST
    -> recursive relative and non-relative import loading and parsing
    -> same-file, relative imported, and Nocter-home imported function resolver
    -> entry validation
    -> call validation for known same-file functions
    -> basic return checking
    -> DiagnosticsEnvelope
    -> JSON stdout
```

If source loading, lexing, or parsing fails, `check` returns a `nocter.diagnostics` envelope with those diagnostics and does not run semantic checks.

The first standard-library source files are `.nocter/std/prelude.nct`, `.nocter/std/string.nct`, `.nocter/std/view.nct`, `.nocter/std/mem.nct`, `.nocter/std/ptr.nct`, `.nocter/std/os.nct`, `.nocter/std/io.nct`, `.nocter/targets/arm64-darwin/std/process.nct`, and `.nocter/targets/arm64-darwin/std/os/macos.nct`. They define the explicit prelude, string and view types, initial memory API, core pointer primitive boundary, common OS error model, user-facing I/O errors, `File`, `stdout`, `stderr`, `print`, byte read/write, text write, macOS process API implementation, and macOS primitive boundary. Future target support should add target overlays without changing ordinary user-facing APIs.

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

Initial `std/io.nct` should expose `IOError`, `File`, `File.open`, `File.read`, `File.write`, `File.write_text`, `stdout`, `stderr`, and `print`. `File.open` creates an owned handle whose drop closes it. `stdout` and `stderr` return `File` values for borrowed process standard streams, and their drop must not close the underlying standard stream.

OS error flow belongs in the standard library:

```text
std/os/macos.SyscallResult
std/os/macos.Errno
std/os.OSError
std/io.IOError
```

The compiler should not special-case any of those names.

Reserved target overlay directories may exist before implementation:

```text
.nocter/targets/x64-linux/std/
.nocter/targets/arm64-linux/std/
.nocter/targets/x64-windows/std/
.nocter/targets/arm64-windows/std/
```

These directories are placeholders. The driver must not treat them as implemented targets until the target registry marks the backend, executable writer, primitive set, and target standard-library overlay as implemented. A request such as `--target x64-linux` should fail with a not-implemented diagnostic while the name remains recognized.

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
- Milestone 2: parser, AST, parsing `program(): i32`, and `nocter ast app.nct --format json`.
- Milestone 3: basic type checking and `program` entry validation.
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
12. Nocter ABI v0 data layout for primitives, pointers, borrows, structs, enums, `T?`, and `T ! E`
13. `program` entry validation, allowing only `program(): void` and `program(): i32`
14. ARM64 instruction encoder
15. minimal Mach-O writer
16. compile a `program(): i32` returning a constant
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
27. initial `.nocter/std/view.nct`
28. initial `.nocter/std/mem.nct`
29. initial `.nocter/std/ptr.nct`
30. initial `.nocter/std/os.nct`
31. initial `.nocter/std/io.nct`
32. initial `.nocter/targets/arm64-darwin/std/process.nct` with process context APIs and termination APIs
33. initial `.nocter/targets/arm64-darwin/std/os/macos.nct`
34. core pointer primitive validation and lowering for `std/ptr`
35. closed target primitive set validation for `std/os/macos.syscall0..6`, `trap`, and `unreachable`
36. primitive lowering for the active target
37. imports from the active target overlay and common Nocter home `std`
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

- `program`
- `T?`
- `T ! E`
- `return none`
- `fail error`
- `try ... catch`
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
