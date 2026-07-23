# Nocter AI Guide

This file is a compact guide for AI tools that read, write, review, or repair Nocter code.
The normative language specification starts at [../README.md](../README.md). When this guide conflicts with the specification, the specification wins.

## Goal

Nocter should be readable and writable by humans first, and predictable for AI tools second.
The language should not add alternate syntax only to satisfy AI tools. Instead, AI support comes from one canonical style, clear examples, machine-readable diagnostics, source formatting, and compiler-owned structure dumps.

## Canonical Style

Use the formatter's output as the only canonical source style.

Important spellings:

```nct
use std/io.print

func main(): i32! {
    print("Hello")?
    return 0
}
```

Rules for generated code:

- Use 4 spaces for indentation.
- Do not write semicolons.
- Prefer `func main(): i32!` for executable roots.
- Treat `main` as an ordinary function name that v0 fixes as the executable entry point, not as a keyword or built-in.
- Write fallible types as `T!`.
- Write fallible optional success values as `T?!`.
- Use `let` for immutable bindings and `var` for mutable bindings.
- Use `&T` for readonly borrow and `&+T` for readwrite borrow.
- Use postfix `expr?` to propagate fallible failure or optional absence.
- Use postfix `expr!` only for unrecoverable assumptions, tests, and prototypes.
- Use `expr catch error { ... }` for local handling of `T!` failure.
- Use `if let value = optional { ... } else { ... }` for optional branching.
- Use `let value = optional else { ... }` for optional early-exit extraction.
- Use `??` for optional default values.
- Use `match` for enum pattern handling.
- Do not use `match` to unwrap `T!` or `T?`.

## Imports

Nocter does not use a `module` declaration. A file's module identity comes from its path.

```nct
use std/io.print
use ./config.Config
```

Rules:

- User project modules receive a synthetic `use std/prelude`.
- Do not write `use std/prelude` in ordinary generated user code unless preserving existing source style.
- Files inside `.nocter/std/` and target standard-library overlays do not receive the synthetic prelude.
- `use path` imports all public exported names from a module.
- `use path.Name` imports selected public names.
- `use path as name` imports a namespace alias.
- Paths starting with `./` or `../` are resolved relative to the current file.
- Paths such as `std/io` are resolved from the active Nocter home.
- Do not invent wildcard imports, textual includes, explicit `.nct` import suffixes, or `module` declarations.

## Errors And Optionals

Fallible values use `T!`. The failure payload is always the built-in `error` type.

```nct
use std/io.print

func announce(text: &str): void! {
    print(text)?
    return
}
```

Handle a failure locally with `catch`.

```nct
run() catch error {
    return Error.new("app.run_failed", error.message)
}
```

Rules:

- `expr?` on `T!` extracts `T` on success and propagates the same `error` on failure.
- `expr catch error { ... }` extracts `T` on success and runs the `catch` block on failure.
- The `catch` binding name is ordinary. `error` is conventional, but `err` is valid.
- A `catch` block must not fall through in v0.
- In a function returning `T!`, `return expr` is a failure return when `expr` has type `error`.
- `try`, `throw`, `Result<T, E>`, `ok`, and failure patterns are not part of fallible handling.

Optional values use `T?`.

```nct
func lookup(name: &str): &str? {
    if missing {
        return none
    }

    return value
}
```

Use optional values through explicit forms:

```nct
if let home = lookup("HOME") {
    use(home)
} else {
    use("fallback")
}
```

```nct
let home = lookup("HOME") else {
    return none
}
```

```nct
let user = lookup("USER") ?? "unknown"
```

Fallible optional success values use `T?!`.

```nct
use std/process.env

func user_name(): &str! {
    return env("USER")? ?? "unknown"
}
```

`env("USER")?` unwraps the fallible layer and leaves `&str?`; `??` chooses a fallback when the optional success is `none`.

## Enums And Match

Use `match`, `if expr is Pattern`, and `?{}` for enum values.

```nct
match error {
    AppError.missing_path {
        report_missing_path()
    }
    AppError.open_failed(path) {
        report_open_failed(path)
    }
    else {
        report_unknown(error)
    }
}
```

Use `?{}` when enum pattern handling must produce a value.

```nct
return error ?{
    AppError.missing_path : missing_code()
    AppError.open_failed(path) : code_for(path)
    : unknown_code()
}
```

Rules:

- Use qualified variants such as `AppError.open_failed(path)`.
- Use `else` as the fallback arm.
- Use `?{}` instead of `return match ...` when the branches produce values.
- Write the `?{}` fallback arm as `: expression`.
- Do not write `switch`; enum pattern handling uses `match` in current Nocter.
- Do not use enum pattern syntax for `T!` or `T?`.

## Documentation Comments

Use doc comments when generated APIs should be useful in future hover, LSP, and generated documentation.

```nct
//! File-level documentation.

/// Opens a file.
func open(path: &str): File! {
    ...
}
```

Rules:

- `///` and `/** ... */` document the next documentable declaration, member, field, variant, or local binding.
- `//!` and `/*! ... */` document the source file/module.
- Empty lines break attachment between a doc comment and the following construct.
- `//` and `/* ... */` are normal implementation comments and must not be treated as hover text.
- `nocter ast app.nct --format json` may expose attached doc text through AST node `documentation` fields.

## Common Mistakes

Avoid these obsolete or invalid patterns:

```nct
// invalid: `try` is not Nocter fallible propagation
let file = try File.open(path)

// invalid: fallible types do not write a custom error type
func read(): String!IOError

// invalid: spaced fallible type syntax is obsolete
func read(): String ! IOError

// invalid: Nocter does not use a module declaration
module app/main

// invalid: enum pattern handling uses match, not match
match error {
    AppError.missing_path {
        return 1
    }
}

// invalid: do not treat print as a compiler builtin
print("Hello")
```

Prefer:

```nct
use std/io.print

func main(): i32! {
    print("Hello")?
    return 0
}
```

## Machine-Readable Tooling

AI tools should interact with the compiler instead of reimplementing Nocter semantics.

Reserved and planned commands:

```sh
nocter check app.nct --format json
nocter tokens app.nct --format json
nocter ast app.nct --format json
nocter fmt app.nct
```

Expected AI loop:

```text
write Nocter code
run nocter fmt
run nocter check --format json
use diagnostics spans and fix hints
repeat
```

Rules:

- `fmt` is the source of canonical formatting.
- `check --format json` is the source of semantic diagnostics.
- `tokens --format json` is the source of lexer output.
- `ast --format json` is the source of parser structure and attached documentation text.
- AI tools must not maintain a separate interpretation of import resolution, type checking, ownership, borrowing, optional handling, fallible handling, or the fixed root `main` entry rule.

## Example Corpus

Use examples as training and repair references:

```text
spec/examples/valid/
spec/examples/invalid/
```

Valid examples should be formatter-ready Nocter code.
Invalid examples should contain one intended error pattern and a short comment explaining it.
