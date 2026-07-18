# Overview

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Status

- Language name: Nocter
- Source extension: `.nct`
- Initial target: `arm64-darwin`
- Initial output: ARM64 Mach-O executable
- Initial cross compilation: disabled, but host and target are modeled separately
- Runtime GC: none
- Default entry function: `main`
- User Nocter home: `~/.nocter/`
- Initial archive root: `.nocter/`
- Release metadata: `VERSION` and `MANIFEST.json`
- Compiler command: `nocter`

## Core Principles

Nocter is a statically typed, value-centered, module-oriented, low-dependency systems language.

The language avoids giving intrinsic language semantics to ordinary identifier names. Names such as `self`, `this`, and `init` are not magic. The executable entry point is selected by the compiler's entry setting; in v0 that setting defaults to the ordinary top-level function name `main`.

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

## Executable Entry

Adopted: Nocter uses a normal top-level function as the executable entry point.

In v0, the compiler's default entry setting is `main`. The CLI can override it with `--entry <name>` for build, run, and check commands.

```nct
func main(): i32! {
    run()?
    return 0
}
```

`main` is not a keyword, reserved word, built-in function, or implicitly imported symbol. It can be called like any other function. Its entry behavior comes from the compiler's default entry setting, not from the identifier having intrinsic language meaning.

The compiler generates the real Mach-O entry code and connects it to the selected entry function. The generated low-level entry code is an implementation detail.

### Allowed Forms

Initial allowed forms:

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
func main(): i32 {
    return 0
}
```

Rules:

- An executable root file must define exactly one top-level function named by the active entry setting.
- v0 defaults the active entry setting to `main`.
- `nocter build app.nct --entry start`, `nocter run app.nct --entry start`, `nocter app.nct --entry start`, and `nocter check app.nct --entry start` select `func start()` instead.
- The `--entry` value must be a valid Nocter identifier and must not be a keyword.
- Entry lookup considers only the root file's top-level functions.
- Imported modules may define ordinary functions named `main`; they are not selected as the executable entry point.
- Duplicate declarations for the selected entry name in one visible scope are normal duplicate function-name errors.
- `func main(): i32!` is the standard executable entry form.
- If `func main(): i32!` or `func main(): usize!` succeeds, the returned value is used as the process exit status.
- If `func main(): i32!` or `func main(): usize!` fails, the compiler-generated entry wrapper writes the built-in `error` payload to stderr and exits with status code `1`.
- The generated failure report should include `error.code` and `error.message` when both fields are available.
- If stderr reporting itself fails, the wrapper ignores that reporting failure and still exits with status code `1`.
- The compiler-generated entry wrapper must not require allocation or call fallible standard-library APIs.
- `func main(): void` exits with status code `0`.
- `func main(): i32` and `func main(): usize` use the returned value as the process exit status.
- `func main(): void`, `func main(): i32`, and `func main(): usize` are accepted for simple infallible entry points in v0, but `func main(): i32!` is the preferred form for applications.
- Entry function parameters are not part of v0.
- Entry functions with parameters, such as `func main(args: Vec<&str>): i32!`, are not part of v0.
- Command-line arguments and environment variables are accessed through `std/process`, not through special entry function parameters.
- Future manifest configuration may allow setting the entry point without repeating `--entry` on every command.

Process entry context:

- The compiler-generated low-level entry code receives the platform process entry information, such as `argc`, `argv`, and environment data when the target provides them.
- User code does not see the platform entry ABI.
- The generated entry code makes process entry information available to the standard library's process context.
- `std/process` exposes process information through ordinary functions such as `args()` and `env(...)`.
- Names such as `args`, `env`, `cwd`, `exit`, and `abort` are standard-library names, not compiler-special identifiers.

Rationale:

- keeps the entry point familiar without reserving a new source construct
- avoids requiring a general attribute system before the language needs one
- lets `main` remain an ordinary function name outside root-file entry selection
- leaves room for future explicit entry configuration
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
- Target-specific std declarations are selected by `#target("...")` directives inside `~/.nocter/std/`, not by `@target` attributes or target overlay directories.
- Low-level compiler boundaries are expressed by typed `primitive` declarations inside the active Nocter home, not by attributes.
- Visibility is expressed by `pub` and `pub(nocter)`, not by attributes.
- Test, inline, deprecation, documentation, export-name, and link-name attributes are not part of v0.
