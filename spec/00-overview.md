# Overview

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Status

- Language name: Nocter
- Source extension: `.nct`
- Initial target: `arm64-darwin`
- Initial output: ARM64 Mach-O executable
- Initial cross compilation: disabled, but host and target are modeled separately
- Runtime GC: none
- Entry syntax: `program`
- User Nocter home: `~/.nocter/`
- Initial archive root: `.nocter/`
- Release metadata: `VERSION` and `MANIFEST.json`
- Compiler command: `nocter`

## Core Principles

Nocter is a statically typed, value-centered, module-oriented, low-dependency systems language.

The language avoids giving special meaning to ordinary identifier names. Names such as `self`, `this`, `init`, and `main` are not magic. Special behavior must be represented by syntax, types, or explicit declarations.

Nocter prioritizes:

- direct compilation from `.nct` to native executable output
- initial direct output for `arm64-darwin`: ARM64 Mach-O
- no dependency on `clang`, `as`, `ld`, Xcode Command Line Tools, or external runtime libraries
- simple and readable high-level syntax
- AI-readable and AI-writable source through one canonical style, stable examples, and machine-readable diagnostics
- value-centered program structure using `struct`, `enum`, `func`, `impl`, and modules
- memory management without GC
- standard-library implementation in Nocter, with limited typed `primitive` declarations for low-level boundaries
- no user-facing `unsafe` mode in v0; low-level trusted code is restricted to the active Nocter home

AI support must not fragment the language surface. Nocter should prefer `nocter fmt`, `nocter check --format json`, `nocter tokens --format json`, `nocter ast --format json`, and curated examples over alternate syntax forms for the same concept.

## Program Entry

Adopted: Nocter uses a dedicated top-level `program` construct for executable entry points.

```nct
program(): i32 {
    return 0
}
```

`program` is not a function name. It is a reserved top-level construct that defines the source-level entry point for an executable.

The compiler generates the real Mach-O entry code and connects it to the `program` body. The generated low-level entry code is an implementation detail.

### Allowed Forms

Initial allowed forms:

```nct
program(): void {
    ...
}
```

```nct
program(): i32 {
    return 0
}
```

Rules:

- An executable must contain exactly one `program` construct.
- Library modules must not define `program`.
- `program` is not imported or exported as a normal function.
- `program(): void` exits with status code `0`.
- `program(): i32` uses the returned value as the process exit status.
- `program` parameters are not part of v0.
- `program(args: [str])` is not part of v0.
- Command-line arguments and environment variables are accessed through `std/process`, not through special `program` parameters.
- `func main()` has no special meaning. `main` is an ordinary identifier if used.

Process entry context:

- The compiler-generated low-level entry code receives the platform process entry information, such as `argc`, `argv`, and environment data when the target provides them.
- User code does not see the platform entry ABI.
- The generated entry code makes process entry information available to the standard library's process context.
- `std/process` exposes process information through ordinary functions such as `args()` and `env(...)`.
- Names such as `args`, `env`, `cwd`, `exit`, and `abort` are standard-library names, not compiler-special identifiers.

Rationale:

- avoids making the identifier `main` magical
- avoids requiring a general attribute system before the language needs one
- makes executable source files visually clear
- keeps the entry point explicit without adding project configuration
- keeps process arguments in the standard library instead of expanding entry syntax

## Attributes

Adopted: v0 has no attribute syntax.

Nocter does not use attributes for entry points, layout control, target selection, optimization hints, testing, deprecation, primitive declarations, or trusted-code boundaries in v0.

Not adopted in v0:

```nct
@inline
@repr(...)
@target(...)
@test
@deprecated
```

Rules:

- The `@` character is reserved for possible future attribute-like syntax and is invalid in v0 source outside string literals, byte literals, and comments.
- Layout is governed by Nocter ABI v0, not by a `repr` attribute.
- Target-specific code is selected by target overlays under `~/.nocter/targets/<target>/std/`, not by per-item target attributes.
- Low-level compiler boundaries are expressed by typed `primitive` declarations inside the active Nocter home, not by attributes.
- Visibility is expressed by `pub` and `pub(nocter)`, not by attributes.
- Test, inline, deprecation, documentation, export-name, and link-name attributes are not part of v0.
