# Nocter Compiler Development Notes

This document is for developers working on the Nocter compiler implementation. User-facing project information belongs in the repository root `README.md`. The language specification belongs in `SPEC.md`.

## Goal

The compiler must compile `.nct` source files directly to ARM64 macOS Mach-O executables.

It must not depend on:

- `clang`
- `as`
- `ld`
- Xcode Command Line Tools
- external runtime libraries

The distributable product is:

```text
.nocter/
    nocter
    std/
```

`src/` contains the implementation used to build the `nocter` compiler. `.nocter/` contains the user-facing compiler binary and standard library.

## Compiler Responsibilities

The compiler owns the whole pipeline:

1. lexing
2. parsing
3. AST construction
4. name resolution
5. type checking
6. ownership, borrow, move, and drop checking
7. optional IR generation
8. ARM64 instruction encoding
9. Mach-O executable generation
10. minimal link-style layout work when needed

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
    arm64/
    macho/
    driver/
    diagnostics/
```

Responsibilities:

- `lexer/`: tokenization for `.nct` source files.
- `parser/`: syntax parsing and parser diagnostics.
- `ast/`: source-level syntax tree definitions.
- `resolve/`: imports, path-derived modules, visibility, and name lookup.
- `typecheck/`: type rules, generics, traits, fallible types, optional types, ownership checks.
- `ir/`: optional lower-level compiler representation if direct AST lowering becomes too tangled.
- `arm64/`: ARM64 instruction selection and binary instruction encoding.
- `macho/`: Mach-O headers, segments, sections, symbols, relocations if needed, and executable layout.
- `driver/`: command-line flow, source loading, import root discovery, `.nocter/std` lookup.
- `diagnostics/`: structured errors with source spans.

## Implementation Order

The implementation should keep the self-contained goal intact from the beginning.

1. lexer
2. parser
3. AST
4. diagnostics with source spans
5. path-derived module loading
6. name resolution and visibility
7. basic type checking
8. `program` entry validation
9. ARM64 instruction encoder
10. minimal Mach-O writer
11. compile a `program(): i32` returning a constant
12. string literal placement
13. basic call lowering
14. statement control flow: `if`, `match`, `while`, `loop`, `break`, `continue`
15. fallible and optional control flow
16. ownership, move, borrow, and drop checks
17. restricted standard-library `asm`
18. imports from `.nocter/std`
19. standard-library growth

## Design Constraints

Nocter should prefer language and standard-library mechanisms over compiler magic. The compiler must not special-case ordinary names such as `print`, `exit`, `File`, `String`, `Option`, or `Result`.

Exceptions are syntax and core type forms adopted by the language, such as:

- `program`
- `T?`
- `T!E`
- `return none`
- `fail error`
- `match ... is ok(...)`
- `match ... is fail(...)`

The standard library may use restricted ARM64 `asm` as the low-level escape hatch. General user code should not rely on `asm` in the initial language design.

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
