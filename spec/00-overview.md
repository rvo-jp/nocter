# Overview

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Status

- Language name: Nocter
- Source extension: `.nct`
- Initial target: `arm64-macos`
- Initial output: ARM64 Mach-O executable
- Initial cross compilation: disabled, but host and target are modeled separately
- Runtime GC: none
- Entry syntax: `program`
- Host toolchain directory: `~/.nocter-arm64-macos` for the initial host package
- Compiler command: `nocter`

## Core Principles

Nocter is a statically typed, value-centered, module-oriented, low-dependency systems language.

The language avoids giving special meaning to ordinary identifier names. Names such as `self`, `this`, `init`, and `main` are not magic. Special behavior must be represented by syntax, types, attributes, or explicit declarations.

Nocter prioritizes:

- direct compilation from `.nct` to native executable output
- initial direct output for `arm64-macos`: ARM64 Mach-O
- no dependency on `clang`, `as`, `ld`, Xcode Command Line Tools, or external runtime libraries
- simple and readable high-level syntax
- value-centered program structure using `struct`, `enum`, `func`, `impl`, and modules
- memory management without GC
- standard-library implementation in Nocter, with limited typed `primitive` declarations for low-level boundaries

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

Future candidate:

```nct
program(args: View<StringView>): i32 {
    ...
}
```

Rules:

- An executable must contain exactly one `program` construct.
- Library modules must not define `program`.
- `program` is not imported or exported as a normal function.
- `program(): void` exits with status code `0`.
- `program(): i32` uses the returned value as the process exit status.
- `func main()` has no special meaning. `main` is an ordinary identifier if used.

Rationale:

- avoids making the identifier `main` magical
- avoids requiring a general attribute system before the language needs one
- makes executable source files visually clear
- keeps the entry point explicit without adding project configuration
