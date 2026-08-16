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
- Treat `main` as the ordinary function name selected by an executable target, not as a keyword or
  built-in.
- Write fallible types as `T!`.
- Write fallible optional success values as `T?!`.
- Use `void!` for a recoverable operation with no success value. Do not generate `void?`,
  `void?!`, or `(void!)?`; model an observable absence/completion distinction with an enum.
- Use `never` only for terminating control flow. Do not wrap it in `?` or `!`; use `void!` for an
  operation that can recoverably fail without returning success data.
- Write `never` only as a complete callable result type. Do not use it in parameters, aggregates,
  pointers, borrows, generic arguments, associated types, arrays, or other data positions. A
  terminating `never` expression does not infer a generic type argument.
- Treat `void` as completion without a value. Use it as a callable result, as `void!`, or as the
  opaque pointer spelling `*void`; do not use it for bindings, parameters, borrows, aggregates,
  arrays, generic arguments, or associated types. Use an empty struct for a storable unit value.
- A `void` expression may feed an expected `void` completion such as `return log()`, or construct
  payloadless success at a concrete expected `void!` boundary. It does not infer a generic type.
- Inside interpolation, postfix `?` performs ordinary propagation and cleanup, while postfix `!`
  traps without dropping the partial `String`. A bare fallible call never propagates implicitly.
- Use `let` for immutable bindings and `var` for mutable bindings.
- Use `&T` for readonly borrow and `&+T` for readwrite borrow.
- Use postfix `expr?` to propagate fallible failure or optional absence.
- Use postfix `expr!` only for unrecoverable assumptions, tests, and prototypes.
- Use `expr catch error { ... }` for local handling of `T!` failure.
- Use `expr otherwise { ... }` for optional fallback values and optional-side early exits.
- Existing move-only outcome places require `move` before elimination: `move value?`,
  `move value!`, `move value catch error { ... }`, or `move value otherwise { ... }`. New
  temporaries and copyable outcomes omit `move`.
- Do not concatenate expression suffixes as `??`, `!!`, `?!`, or `!?`. Use an intermediate binding
  or parentheses such as `(move result?)?` for a second outcome layer. Type syntax `T?!` remains
  valid and has a different grammar.
- A failed or absent postfix `!` immediately traps without Nocter-provided output or cleanup. Use
  `catch` or `otherwise` when a stable error message, exit status, or cleanup is required.
- Return values are injected through declared outcome layers from outermost to innermost. For
  `T?!`, `error` is outer failure and `none` is success absence. For `(T!)?`, `none` is outer
  absence and `error` is a present inner failure. A value already having the complete declared
  result type is returned unchanged.
- Postfix `?` propagates through the matching declared layer without reordering outcomes. A
  fallible failure in `(T!)?` becomes a present inner failure; optional absence in `T?!` remains
  outer success with inner absence.
- Expected-type boundaries use the same recursive injection for optional and fallible values.
  Annotated initializers, assignments, arguments, aggregate payloads, fallbacks, and returns may
  accept a payload and construct the expected outcome. Do not use `none` without an expected type.
- A known generic shape such as `T?` may infer `T` from a payload before injection. `none` and a
  failure `error` do not infer an unknown payload type.
- Signed integers use fixed-width two's-complement ranges. Do not assume wrapping arithmetic;
  ordinary overflow remains an always-on trap.
- Signed minimum literals are written normally, such as `let minimum: i8 = -128`. The checker
  validates unary `-` and its grouped integer literal as one mathematical value for range checking.
- Shifts are fixed-width bit operations. Left shift discards outgoing high bits, unsigned right
  shift zero-fills, and signed right shift sign-fills. Negative or out-of-width counts trap.
- Signed division truncates toward zero, and signed remainder is zero or has the dividend's sign.
  Both `/` and `%` trap for minimum signed value with divisor `-1`.
- Compound assignment evaluates the right-hand side first and the target place exactly once
  afterward. Do not reason about it as duplicated `target = target operator rhs` source.
- Use `match` for enum pattern handling.
- Declare at least one variant in every enum; zero-variant enums are invalid.
- Do not use `match` to unwrap `T!` or `T?`.

## Imports

Nocter does not use a `module` declaration. A file's module identity comes from its path.

```nct
use std/io.print
use ./config.Config
```

Rules:

- User project modules receive a compiler-managed synthetic standard prelude.
- Do not write `use std/prelude` in generated user code; source-level prelude imports are invalid.
- Files inside the active Nocter home `std/` tree do not receive the synthetic prelude.
- `use path` imports a module namespace using the path's default name.
- `use path.Name` imports selected public names.
- `use path as name` imports a namespace alias.
- Paths starting with `./` or `../` are resolved relative to the current file.
- Non-relative paths name a declared dependency or `std`; `std/io` resolves only through the
  active Nocter home.
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
run() catch failure {
    return error.new("app.run_failed", failure.message)
}
```

Rules:

- `expr?` on `T!` extracts `T` on success and propagates the same `error` on failure.
- `expr catch error { ... }` extracts `T` on success and runs the `catch` block on failure.
- The `catch` binding name is ordinary. `error` is conventional, but `err` is valid.
- A reachable `catch` block end must produce `T`; the block may instead leave the current control
  path with explicit control flow.
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
let home = lookup("HOME")?
```

```nct
let user = lookup("USER") otherwise { "unknown" }
```

Fallible optional success values use `T?!`.

```nct
use std/process.env

func user_name(): &str! {
    return env("USER")? otherwise { "unknown" }
}
```

`env("USER")?` unwraps the fallible layer and leaves `&str?`; `otherwise` chooses a fallback when the optional success is `none`.

## Enums And Match

Use `match` and `if expr is Pattern` for enum values.

```nct
match error {
    AppError.missing_path {
        report_missing_path()
    }
    AppError.open_failed(path) {
        report_open_failed(path)
    }
    _ {
        report_unknown(error)
    }
}
```

Use `match` expressions when enum pattern handling must produce a value.

```nct
return match error {
    AppError.missing_path { missing_code() }
    AppError.open_failed(path) { code_for(path) }
    _ { unknown_code() }
}
```

Rules:

- Use qualified variants such as `AppError.open_failed(path)`.
- Use `_` as the fallback arm.
- Use `return match ...` or a body-result `match` when the branches produce values.
- Write `match` value arms as `Pattern { result }`.
- Match `&value` to inspect payloads as readonly borrows, `&+value` to mutate payloads through
  readwrite borrows, and `move value` to consume an existing move-only enum and own its payload.
- Do not add borrow markers to individual payload names. Every payload binding follows the pattern
  target capability.
- Supply one positional identifier or `_` for every payload field. `_` ignores exactly one field;
  it never stands for a complete multi-field payload.
- Write each explicit variant at most once in a `match`. Payload names and `_` do not distinguish
  repeated variant arms.
- A final `_` arm may intentionally remain after all current variants are listed. It is still type
  checked and is useful as a fallback for variants introduced by a future dependency version.
- Do not write `switch`; enum pattern handling uses `match` in current Nocter.
- Do not use enum pattern syntax for `T!` or `T?`.

```nct
match &message {
    Message.text(text) { print(text as &str)? } // text: &String
    _ { ... }
}
```

```nct
match &pair {
    Pair.values(_, right) { inspect(right) }
}
```

## Loop Bodies

Loop bodies do not produce values. Their final reachable expression must be `void` or `never`; use
`let _ = expression` when an iteration intentionally discards a value.

Use `&source`, `&+source`, or `move source` to choose collection iteration ownership. A bare source
is valid only when it already is an iterator and ordinary ownership still applies. Existing
move-only iterator bindings require `move`; new iterator temporaries do not.

```nct
for item in move iterator { consume(move item) }
for item in make_iterator() { consume(move item) }
```

`move` always requires an existing move-only place, including in iteration and spread. Bind a new
collection before requesting owned expansion:

```nct
let values = make_values()
for item in move values { consume(move item) }
```

## Closure Control Flow

A closure is its own control-flow boundary. `return` and postfix `?` affect the closure result.
`break` and `continue` can target only loops inside that closure, never a loop surrounding the
closure expression.

## Condition Temporaries

Temporary values in an ordinary `if` or `while` boolean condition are dropped after the condition
is computed and before the body begins. Bind an RAII value first when it must remain live in the
body.

## Unreachable Source

Source after `return`, `break`, `continue`, or `never` still must use valid names, visible APIs, and
compatible types. It has no fictional post-terminal ownership state and does not affect executable
reachability. Remove stale unreachable source instead of relying on suppressed flow analysis.

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

// invalid: enum pattern handling uses `match`, not `switch`
switch error {
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

Compiler commands:

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
run nocter build or nocter run when runtime buildability matters
use diagnostics spans and fix hints
repeat
```

Rules:

- `fmt` is the source of canonical formatting.
- `check --format json` is the source of semantic diagnostics.
- `check`, `build`, and `run` accept the same source programs for the same selected target and
  toolchain. Use `build` to verify executable emission and `run` to verify runtime behavior.
- `tokens --format json` is the source of lexer output.
- `ast --format json` is the source of parser structure and attached documentation text.
- AI tools must not maintain a separate interpretation of import resolution, type checking, ownership, borrowing, optional handling, fallible handling, or the fixed root `main` entry rule.

## Examples

Use the runnable programs under the repository-root
[examples directory](../../examples/README.md) as generation references. They demonstrate
canonical single-file and package source that can be checked and run. Invalid compiler fixtures
are development inputs, not user examples or an alternate language contract.
