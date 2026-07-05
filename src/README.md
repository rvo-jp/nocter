# Nocter Compiler Development Notes

This document is for developers working on the Nocter compiler implementation. User-facing project information belongs in the repository root `README.md`. The language specification belongs in `SPEC.md`.

## Goal

The compiler must first compile `.nct` source files directly to `arm64-macos` Mach-O executables.

The implementation should still keep target-specific code isolated so future targets can be added without rewriting the front end, type checker, ownership checker, or high-level standard-library model.

The initial implementation does not support cross compilation beyond `arm64-macos`, but it should still model host and target separately. The current development host package is `.nocter-arm64-macos/`, which contains the compiler binary for ARM64 macOS, common standard-library sources, and the `targets/arm64-macos/` overlay for the `arm64-macos` target.

It must not depend on:

- `clang`
- `as`
- `ld`
- Xcode Command Line Tools
- external runtime libraries

The distributable archive payload for the initial host is:

```text
.nocter-arm64-macos/
    nocter
    std/
        prelude.nct
        io.nct
        mem.nct
        os.nct
        ptr.nct
        string.nct
        view.nct
    targets/
        arm64-macos/
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

Users normally install that payload as `~/.nocter/`. `src/` contains the implementation used to build the `nocter` compiler. `.nocter-arm64-macos/` contains the current development host package for the user-facing compiler binary and standard library. It is generated output and is not committed to git.

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
src/
    README.md
    main.*
    lexer/
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

- `lexer/`: UTF-8 source decoding, CRLF-to-LF normalization, comments, ASCII identifiers, integer/string/byte literals, tokenization for `.nct` source files, and v0 diagnostics for reserved-but-invalid punctuation such as `@`.
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
- `driver/`: command-line flow for `build`, `run`, `check`, and future `lsp`, root `.nct` file loading, recursive import graph loading, canonical-path module de-duplication, target registry lookup, active target selection, active target overlay lookup, common Nocter home `std` lookup, temporary executable handling for `run`, and stdout/stderr discipline.
- `diagnostics/`: structured errors with source spans, display paths, canonical absolute paths, human rendering, and the `nocter.diagnostics` JSON envelope for `--format json`.
- `lsp/`: future language-server entry point that reuses the compiler front end, resolver, type checker, ownership checker, and diagnostics instead of reimplementing language semantics for editors.

The v0 driver should not implement package manifests, project-root discovery, package registries, workspaces, separate compilation, or incremental module artifacts. `nocter build app.nct`, `nocter run app.nct`, and `nocter check app.nct` each receive one root file and operate on the whole reachable compile unit.

Editor tooling should treat the compiler as the source of semantic truth. A VS Code TextMate grammar may provide syntax highlighting, but import resolution, type checking, borrow checking, and diagnostics belong in `nocter check --format json` first and later in `nocter lsp`.

The first standard-library source files are `.nocter-arm64-macos/std/prelude.nct`, `.nocter-arm64-macos/std/string.nct`, `.nocter-arm64-macos/std/view.nct`, `.nocter-arm64-macos/std/mem.nct`, `.nocter-arm64-macos/std/ptr.nct`, `.nocter-arm64-macos/std/os.nct`, `.nocter-arm64-macos/std/io.nct`, `.nocter-arm64-macos/targets/arm64-macos/std/process.nct`, and `.nocter-arm64-macos/targets/arm64-macos/std/os/macos.nct`. They define the explicit prelude, string and view types, initial memory API, core pointer primitive boundary, common OS error model, user-facing I/O errors, macOS process API implementation, and macOS primitive boundary. Future target support should add target overlays without changing ordinary user-facing APIs.

The target-independent core pointer primitive set is:

```text
std/ptr.addr
std/ptr.from_ref
std/ptr.from_ref_mut
std/ptr.from_addr
```

`std/ptr.from_addr` is restricted to modules inside the active Nocter home. Project modules must receive a diagnostic if they call it.

The initial `arm64-macos` primitive set is deliberately small:

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

The compiler should validate primitives by module path, name, and exact signature. `print`, `exit`, file APIs, allocator APIs, `String`, and `Buffer` are standard-library APIs, not compiler primitives.

Future typed wrappers such as raw file, process, allocation, or memory-map helpers should be ordinary Nocter APIs in common `std/` or the active target overlay. The normal implementation path is to grow the standard library on top of the closed primitive set, not to add compiler primitives for each OS operation. User project modules remain outside the primitive declaration boundary.

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
.nocter-arm64-macos/targets/x64-linux/std/
.nocter-arm64-macos/targets/arm64-linux/std/
.nocter-arm64-macos/targets/x64-windows/std/
.nocter-arm64-macos/targets/arm64-windows/std/
```

These directories are placeholders. The driver must not treat them as implemented targets until the target registry marks the backend, executable writer, primitive set, and target standard-library overlay as implemented. A request such as `--target x64-linux` should fail with a not-implemented diagnostic while the name remains recognized.

Standard-library resolution should search the active target overlay before the common standard library:

```text
<NOCTER_HOME>/targets/<active-target>/std/
<NOCTER_HOME>/std/
```

Both roots map to the `std/...` import path namespace.

## Implementation Order

The implementation should keep the self-contained goal intact from the beginning.

1. lexer
2. parser
3. AST
4. diagnostics with source spans
5. target registry with `arm64-macos` implemented and future targets reserved
6. active target value, defaulting to host target `arm64-macos`
7. path-derived module loading
8. name resolution and visibility
9. basic type checking
10. Nocter ABI v0 data layout for primitives, pointers, borrows, structs, enums, `T?`, and `T!E`
11. `program` entry validation
12. ARM64 instruction encoder
13. minimal Mach-O writer
14. compile a `program(): i32` returning a constant
15. string literal placement
16. Nocter ABI v0 function call and return lowering
17. statement control flow: `if`, `match`, `while`, `loop`, range `for`, `break`, `continue`
18. fallible and optional control flow
19. `never` and unreachable-code diagnostics
20. ownership, move, borrow, and drop checks
21. drop glue generation using Nocter ABI v0
22. region scopes and escape diagnostics
23. initial `.nocter-arm64-macos/std/prelude.nct`
24. initial `.nocter-arm64-macos/std/string.nct`
25. initial `.nocter-arm64-macos/std/view.nct`
26. initial `.nocter-arm64-macos/std/mem.nct`
27. initial `.nocter-arm64-macos/std/ptr.nct`
28. initial `.nocter-arm64-macos/std/os.nct`
29. initial `.nocter-arm64-macos/std/io.nct`
30. initial `.nocter-arm64-macos/targets/arm64-macos/std/process.nct`
31. initial `.nocter-arm64-macos/targets/arm64-macos/std/os/macos.nct`
32. core pointer primitive validation and lowering for `std/ptr`
33. closed target primitive set validation for `std/os/macos.syscall0..6`, `trap`, and `unreachable`
34. primitive lowering for the active target
35. imports from the active target overlay and common Nocter home `std`
36. standard-library growth
37. `nocter run app.nct` using a temporary Mach-O executable and the same code path as `build`
38. `nocter check --format json` using compiler-owned diagnostics
39. `nocter lsp` reusing the compiler front end and semantic checks

## Design Constraints

Nocter should prefer language and standard-library mechanisms over compiler magic. The compiler must not special-case ordinary names such as `print`, `exit`, `File`, `String`, `Option`, or `Result`.

Exceptions are syntax and core type forms adopted by the language, such as:

- `program`
- `T?`
- `T!E`
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
