# Overview

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Status

- Language name: Nocter
- Source extension: `.nct`
- Current target: `arm64-darwin`
- Current output: ARM64 Mach-O executable
- Cross compilation: disabled, but host and target are modeled separately
- Runtime GC: none
- Default entry function: `main`
- User Nocter home: `~/.nocter/`
- Archive root: `.nocter/`
- Normal command exposure: `nocter` symlink in an existing `PATH` directory
- Release metadata: `VERSION` and `MANIFEST.json`
- Compiler command: `nocter`

## Core Principles

Nocter is a statically typed, value-centered, module-oriented, low-dependency systems language.
Its durable design pillars are simplicity, encapsulation, and foolproof design;
the full rationale is defined in [Design Principles](00-design-principles.md).

The language avoids giving intrinsic language semantics to most ordinary identifier names. Names
such as `self`, `this`, and `init` are not magic. An executable declaration selects a module whose
top-level function named `main` is the entry point; the function name is fixed to remove
command-line ambiguity.

Nocter prioritizes:

- direct compilation from `.nct` to native executable output
- direct output for `arm64-darwin`: ARM64 Mach-O
- no dependency on `clang`, `as`, `ld`, Xcode Command Line Tools, or external runtime libraries
- simple and readable high-level syntax
- AI-readable and AI-writable source through one canonical style, stable examples, and machine-readable diagnostics
- value-centered program structure using `struct`, `enum`, `func`, `instance`, `conform`, and modules
- memory management without GC
- standard-library implementation in Nocter, with limited typed `primitive` declarations for low-level boundaries
- no user-facing `unsafe` mode; low-level trusted code is restricted to the active Nocter home

AI support must not fragment the language surface. Nocter should prefer `nocter fmt`, `nocter check --format json`, `nocter tokens --format json`, `nocter ast --format json`, and curated examples over alternate syntax forms for the same concept.

## Executable Entry

Nocter uses a normal top-level function as the executable entry point.

The executable entry name is fixed to `main`. The CLI does not provide `--entry`. Package-root
`index.nct` selects the containing module through `#executable`.

```nct
func main(): i32! {
    run()?
    return 0
}
```

`main` is not a keyword, reserved word, built-in function, or implicitly imported symbol. It can be called like any other function. Its entry behavior follows the executable-entry rules below.

The compiler generates the real Mach-O entry code and connects it to the selected entry function. The generated low-level entry code is an implementation detail.

### Allowed Forms

Allowed forms:

```nct
func main(): i32! {
    ...
}
```

```nct
func main(): void {
    ...
}
```

```nct
func main(): void! {
    run()?
    return
}
```

```nct
func main(): i32 {
    return 0
}
```

Rules:

- A selected executable module must define exactly one top-level function named `main`.
- The CLI does not accept `--entry`.
- Entry lookup considers only the selected executable module's top-level functions.
- Imported modules may define ordinary functions named `main`; they are not selected as the executable entry point.
- Duplicate declarations for the selected entry name in one visible scope are normal duplicate function-name errors.
- `func main(): i32!` is the standard executable entry form.
- If `func main(): i32!` or `func main(): usize!` succeeds, the returned value is used as the process exit status.
- If `func main(): void!` succeeds, the process exits with status code `0`.
- If `func main(): i32!`, `func main(): usize!`, or `func main(): void!` fails, the compiler-generated entry wrapper writes the built-in `error` payload to stderr and exits with status code `1`.
- The generated failure report includes the root classification code and the outer-to-inner
  context and leaf messages exposed by the built-in error handle.
- If stderr reporting itself fails, the wrapper ignores that reporting failure and still exits with status code `1`.
- The compiler-generated entry wrapper must not require allocation or call fallible standard-library APIs.
- `func main(): void` exits with status code `0`.
- `func main(): i32` and `func main(): usize` use the returned value as the process exit status.
- `func main(): void`, `func main(): void!`, `func main(): i32`, `func main(): i32!`, `func main(): usize`, and `func main(): usize!` are accepted entry return forms, but `func main(): i32!` is the preferred form for applications that need a numeric success code.
- Entry functions cannot declare type parameters or value parameters.
- Command-line arguments and environment variables are accessed through `std/process`, not through special entry function parameters.
- Package-root `#executable` metadata selects the module, not a different function name.

Process entry context:

- The compiler-generated low-level entry code receives the platform process entry information, such as `argc`, `argv`, and environment data when the target provides them.
- User code does not see the platform entry ABI.
- The generated entry code makes process entry information available to the standard library's process context.
- `std/process` exposes process information through ordinary functions such as `args()` and `env(...)`.
- Names such as `args`, `env`, `cwd`, `exit`, and `abort` are standard-library names, not compiler-special identifiers.

Rationale:

- keeps the entry point familiar without reserving a new source construct
- avoids requiring a general attribute system before the language needs one
- lets `main` remain an ordinary function name outside the root-file `main` entry rule
- leaves room for future explicit entry configuration
- keeps process arguments in the standard library instead of expanding entry syntax

## Attributes

Nocter has no attribute syntax.

Nocter does not use attributes for entry points, layout control, target selection, optimization hints, testing, deprecation, primitive declarations, or trusted-code boundaries.

Invalid syntax:

```nct
@inline
@repr(...)
@target(...)
@test
@deprecated
```

Rules:

- The `@` character is reserved for possible future attribute-like syntax and is invalid outside string literals, byte literals, and comments.
- Layout is governed by the internal Nocter ABI, not by a `repr` attribute.
- Target-specific std declarations are selected by `#target: "..."` directives inside `~/.nocter/std/`, not by `@target` attributes or target overlay directories.
- Low-level compiler boundaries are expressed by typed `primitive` declarations inside the active Nocter home, not by attributes.
- Visibility is expressed by `pub`, ancestor scopes such as `pub(../)`, and package scope
  `pub(/)`, not by attributes.
- Test, inline, deprecation, documentation, export-name, and link-name attributes are not supported.
