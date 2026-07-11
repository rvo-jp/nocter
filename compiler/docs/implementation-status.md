# Nocter Implementation Status

This document tracks the gap between the language specification and the current compiler.
Normative language rules live in `../../spec/`.
This file describes implementation state only.

## Legend

- Specified: covered by the language specification.
- Parsed: accepted by the parser and represented in the AST.
- Checked: covered by name resolution, type checking, or control-flow diagnostics.
- Buildable: lowerable through IR and the native ARM64 Darwin backend.
- Runtime: has meaningful executable behavior today.

## Feature Matrix

| Feature | Specified | Parsed | Checked | Buildable | Runtime | Notes |
|---|---:|---:|---:|---:|---:|---|
| Root `main` / `--entry` selection | yes | yes | yes | yes | yes | Entry function must be in the root file. |
| `i32` return values | yes | yes | yes | yes | yes | Literal returns and lowerable expressions are supported. |
| `usize` scalar values | yes | yes | yes | partial | partial | Build supports annotated `usize` locals, non-entry `usize` literal returns, same-file and loaded imported normal calls returning `usize`, and `usize` comparisons in lowerable bool positions. |
| `void` entry | yes | yes | yes | yes | yes | Empty body and bare `return` are buildable. |
| `bool` expressions | yes | yes | yes | partial | yes | Build supports literals, locals, `!`, `&&`, `||`, `i32` and `usize` comparisons, and bool equality/inequality over literal/local operands in lowerable positions. |
| String literals and interpolation | yes | yes | partial | partial | partial | Single-line and multi-line string literals are tokenized, parsed, and typed as `str`; interpolation is parsed as an explicit expression and checked as `String!` with limited supported part types. The initial `std/string` and `std/fmt` API boundary exists, but build currently consumes only static string literals as `make_error` messages; interpolated string construction is not lowerable. |
| Immutable `let` bindings | yes | yes | yes | partial | yes | Build supports lowerable `i32`, annotated `usize`, and `bool` initializers. |
| `var` and reassignment | yes | yes | partial | no | no | Backend has no general local storage yet. |
| Same-file function calls | yes | yes | yes | partial | partial | Build supports non-generic `i32` tail calls with up to 8 `i32` arguments, `bool` tail returns, and a narrow scalar normal-call subset. |
| Imported function calls | yes | yes | yes | partial | partial | Build supports loaded imported non-generic scalar calls through the same narrow scalar call subset as same-file calls. Unloaded imported placeholders still diagnose before backend lowering. |
| General non-tail calls | yes | yes | yes | partial | partial | Build supports non-generic scalar normal calls in selected expression positions. Unsupported shapes still report IR lowering diagnostics. |
| Terminal `if` / `else` | yes | yes | yes | partial | yes | Build supports terminal branches returning direct `i32` or non-entry `bool`. |
| General `if`, `while`, `loop`, `for`, `match`, `?{}` | yes | yes | partial | no | no | Several forms are checkable; backend lowering remains intentionally narrow. |
| Fallible entry success/failure | yes | yes | partial | partial | partial | Build supports simple success and `return make_error("code", "message")` failure. |
| Optional values | yes | yes | yes | no | no | Check examples cover optionals; backend and runtime layout are not implemented. |
| Structs and enums | yes | yes | partial | no | no | Type checking exists for several cases; aggregate layout/lowering is not buildable. |
| Arrays, views, and pointers | yes | yes | partial | no | no | Compiler-owned layout and provenance rules are still future work. |
| Methods, traits, and generics | yes | yes | partial | no | no | Parser and selected diagnostics exist; monomorphization/lowering do not. |
| Ownership, borrowing, move, drop | yes | partial | partial | no | no | Design exists; full semantic checking and drop glue are not implemented. |
| Standard library API names | yes | yes | partial | partial | partial | `.nocter/std` includes initial prelude, error, memory, owning string, formatting, pointer, OS, and I/O declarations; many bodies are placeholders or primitive boundaries. |
| `check --format json` | yes | n/a | yes | n/a | yes | JSON diagnostics are used by corpus tests. |
| `tokens --format json` and `ast --format json` | yes | yes | n/a | n/a | yes | Tooling aids, not stable language compatibility promises. |
| `fmt` | yes | yes | n/a | n/a | partial | v0 rejects files with comments instead of rewriting them. |
| `lsp` | partial | n/a | partial | n/a | partial | JSON-RPC server supports initialize, workspace root recording, shutdown, exit, full document sync, stale version rejection, open-document import reuse, stale diagnostic clearing, UTF-16 diagnostic positions, publishDiagnostics, semantic tokens, hover, definition, document symbols, and basic completions. |

## Buildable Subset

The current buildable language subset is intentionally smaller than the checkable subset.
The front end can parse and type-check more Nocter syntax than the backend can lower.

Currently buildable:

- root-file `main` or `--entry <name>`
- entry return types `i32`, `i32!`, and `void`
- literal `i32` returns
- immutable local `let` bindings whose initializer is lowerable as `i32`, annotated `usize`, or `bool`
- `void` entry with an empty body or bare `return`
- same-file and loaded imported non-generic tail calls returning `i32` or `bool`
- same-file and loaded imported non-generic normal calls returning `i32` in `let` initializers
- same-file and loaded imported non-generic normal calls returning `i32` in `i32` arithmetic and shift expressions using `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`, evaluated left to right with distinct temporary locals
- same-file and loaded imported non-generic normal calls returning `i32` as `i32` comparison operands such as `if answer() == 42`, `let matched = left() <= right()`, and `return left() < right()`
- same-file and loaded imported non-generic normal calls returning `usize` in annotated `let` initializers
- non-entry functions returning `usize` literal/local/call values in lowerable positions
- `usize` comparisons over literals, locals, and same-file or loaded imported normal calls in lowerable bool expressions and terminal `if` conditions
- same-file and loaded imported non-generic normal calls returning `bool` in `let` initializers
- same-file and loaded imported non-generic normal calls returning `bool` under unary `!` in `let` initializers and bool return expressions
- same-file and loaded imported non-generic normal calls returning `bool` as atomic bool equality/inequality operands such as `ready() == true` and `left() != right()`
- same-file and loaded imported non-generic normal calls returning `bool` in short-circuit bool value expressions such as `let value = ready() && other()` and `return ready() || other()`
- same-file and loaded imported non-generic normal calls returning `bool` directly in terminal `if` conditions, including unary `!`
- same-file and loaded imported non-generic normal calls returning `bool` in terminal `if` short-circuit conditions such as `ready() && other()` and `ready() || other()`
- short-circuit bool expressions that combine `i32` call comparisons with bool calls, such as `if answer() == 42 && ready()` and `let matched = answer() == 42 && ready()`
- nested `i32` normal-call arguments such as `let value = outer(inner())`
- nested `i32` tail-call arguments such as `return outer(inner())`
- up to 8 `i32` parameters for lowered functions and calls
- reordered parameter arguments are supported for normal calls and tail calls through argument staging
- non-entry functions returning `bool` or `usize`
- `i32` arithmetic with `+`, `-`, `*`, `/`, and `%` used in lowerable `i32` expressions; addition, subtraction, and multiplication emit signed-overflow trap checks, and division and remainder emit zero-divisor plus signed-overflow trap checks
- `i32` shifts with `<<` and `>>` used in lowerable `i32` expressions; shift counts trap when negative or greater than or equal to 32
- bool `!`, `&&`, `||`, bool equality/inequality over literal/local operands, and `i32` or `usize` comparisons used in lowerable bool expressions
- terminal `if` / `else` statements with bool literal, bool local, bool equality/inequality over literal/local operands, or `i32`/`usize` comparison conditions and direct `i32` or non-entry `bool` returns in both branches
- simple fallible entry success
- simple fallible entry failure through `return make_error("code", <static string message>)`, where the message may be a single-line or multi-line string literal

Currently not buildable even when it may be checkable:

- `var`, reassignment, and general local storage
- general `if`, `while`, `loop`, range `for`, `match`, and pattern conditional `?{}`
- unloaded imported function placeholders
- compound bool equality operands with calls such as `(ready() && other()) == true`
- `usize` parameters, `usize` arithmetic, and `usize` entry return values
- `str` values beyond static fallible failure messages
- interpolated string construction
- optional values
- aggregate values, arrays, views, pointers, methods, traits, generics, ownership lowering, and drop glue
- custom output path selection through `-o`
