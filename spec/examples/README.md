# Nocter Example Corpus

This directory contains small Nocter examples for humans, tests, editor tooling, and AI-assisted code generation.

The examples are not a replacement for the specification. They show canonical style and common mistakes in compact form.

Layout:

```text
spec/examples/
    valid/
        *.nct
        imports/
            app.nct
            config.nct
    packages/
        hello/
            nocter.nct
    invalid/
        *.nct
```

Rules:

- `valid/` examples should be formatter-ready Nocter code.
- `invalid/` examples should focus on one intended mistake per file.
- Invalid examples should explain the intended mistake in comments.
- Examples represent user project modules and must not write source-level `use std/prelude`; the standard prelude is compiler-managed.
- Compiler integration tests check that `valid/` examples pass `nocter check` and `invalid/` examples fail it.
- The same examples are checked through `nocter check --format json` to keep the diagnostics envelope stable for future editor and LSP tooling.
- `valid/` means source-valid for `check`. Examples may use implemented development-version
  surfaces that are newer than the released v0.2.0 runtime.
- Companion files under a valid example subdirectory are imported by the checked root example and are not necessarily checked as standalone executable roots.
- `packages/` contains complete package roots checked through package mode rather than `--file`.
- Do not use examples to introduce syntax that is not specified in `spec/README.md`.

## Package Example

The `hello` package keeps its manifest and root-module code in the same `nocter.nct`:

```sh
cd spec/examples/packages/hello
nocter check
nocter run
```

Its `#executable` omits `entry`, so package execution uses the `main` declared in `nocter.nct`.

## Front-End Coverage

The corpus is an external stability suite for released v0.3.0 language behavior and the adopted
v0.4.0 package contract. It covers lexer, parser, compile-unit loading, import resolution, type
checking, entry selection, human diagnostics, and JSON diagnostics through the real `nocter`
command.

Current valid coverage:

- default `main` entry
- doc comments for future editor tooling
- non-relative standard-library import
- relative source import
- fallible `T!`, postfix `?`, postfix `!`, and `catch`
- optional `T?`, `otherwise`, and enum `if is`
- executable fallible optional success `T?!` through `std/process.env`
- enum construction and `match`
- range-only `for`
- package metadata and ordinary root-module code in one `nocter.nct`

Current invalid coverage:

- missing default entry
- invalid entry signature
- removed `module` declaration syntax
- return type mismatch
- fallible propagation outside a fallible return layer
- optional propagation outside an optional return layer
- `catch` on `T?`
- postfix `!` on a plain value
- mismatched `otherwise` fallback type
- non-integer range bounds
- `match` on a non-enum value
