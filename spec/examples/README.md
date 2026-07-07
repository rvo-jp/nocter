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
    invalid/
        *.nct
```

Rules:

- `valid/` examples should be formatter-ready Nocter code.
- `invalid/` examples should focus on one intended mistake per file.
- Invalid examples should explain the intended mistake in comments.
- Examples represent user project modules and normally omit redundant `use std/prelude`.
- Compiler integration tests check that `valid/` examples pass `nocter check` and `invalid/` examples fail it.
- The same examples are checked through `nocter check --format json` to keep the diagnostics envelope stable for future editor and LSP tooling.
- Valid examples may opt into a non-default entry function in the compiler integration test. This is used only to show `--entry`; the source name itself remains ordinary.
- Companion files under a valid example subdirectory are imported by the checked root example and are not necessarily checked as standalone executable roots.
- Do not use examples to introduce syntax that is not specified in `SPEC.md`.

## v0 Front-End Coverage

The corpus is the external stability suite for Nocter v0 front-end behavior. It should cover lexer, parser, compile-unit loading, import resolution, type checking, entry selection, human diagnostics, and JSON diagnostics through the real `nocter` command.

Current valid coverage:

- default `main` entry
- explicit `--entry`
- doc comments for future editor tooling
- non-relative standard-library import
- relative source import
- fallible `T!`, postfix `?`, postfix `!`, and `catch`
- optional `T?`, `??`, `if let`, and `let ... else`
- enum construction and `switch`
- range-only `for`

Current invalid coverage:

- missing default entry
- invalid entry signature
- removed `module` declaration syntax
- return type mismatch
- fallible propagation outside a fallible return layer
- optional propagation outside an optional return layer
- `catch` on `T?`
- postfix `!` on a plain value
- fallthrough in `let ... else`
- non-integer range bounds
- `switch` on a non-enum value
