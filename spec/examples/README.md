# Nocter Example Corpus

This directory contains small Nocter examples for humans, tests, editor tooling, and AI-assisted code generation.

The examples are not a replacement for the specification. They show canonical style and common mistakes in compact form.

Layout:

```text
spec/examples/
    valid/
        *.nct
    invalid/
        *.nct
```

Rules:

- `valid/` examples should be formatter-ready Nocter code.
- `invalid/` examples should focus on one intended mistake per file.
- Invalid examples should explain the intended mistake in comments.
- Examples represent user project modules and normally omit redundant `use std/prelude`.
- Compiler integration tests check that `valid/` examples pass `nocter check` and `invalid/` examples fail it.
- Do not use examples to introduce syntax that is not specified in `SPEC.md`.
