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
| `void` entry | yes | yes | yes | yes | yes | Empty body and bare `return` are buildable. |
| `bool` expressions | yes | yes | yes | partial | yes | Build supports literals, locals, `!`, `&&`, `||`, `i32` comparisons, and bool equality/inequality over literal/local operands in lowerable positions. |
| Immutable `let` bindings | yes | yes | yes | partial | yes | Build supports lowerable `i32` and `bool` initializers. |
| `var` and reassignment | yes | yes | partial | no | no | Backend has no general local storage yet. |
| Same-file function calls | yes | yes | yes | partial | partial | Build supports non-generic `i32` tail calls with up to 8 `i32` arguments, `bool` tail returns, and a narrow same-file `i32` normal-call subset. |
| Imported function calls | yes | yes | yes | no | no | Import resolution exists; backend call lowering does not cover imported calls. |
| General non-tail calls | yes | yes | yes | partial | partial | Build supports same-file non-generic `i32` normal calls in `let` initializers and simple `i32` return additions. Unsupported shapes still report `E8006`. |
| Terminal `if` / `else` | yes | yes | yes | partial | yes | Build supports terminal branches returning direct `i32` or non-entry `bool`. |
| General `if`, `while`, `loop`, `for`, `match`, `?{}` | yes | yes | partial | no | no | Several forms are checkable; backend lowering remains intentionally narrow. |
| Fallible entry success/failure | yes | yes | partial | partial | partial | Build supports simple success and `return make_error("code", "message")` failure. |
| Optional values | yes | yes | yes | no | no | Check examples cover optionals; backend and runtime layout are not implemented. |
| Structs and enums | yes | yes | partial | no | no | Type checking exists for several cases; aggregate layout/lowering is not buildable. |
| Arrays, views, and pointers | yes | yes | partial | no | no | Compiler-owned layout and provenance rules are still future work. |
| Methods, traits, and generics | yes | yes | partial | no | no | Parser and selected diagnostics exist; monomorphization/lowering do not. |
| Ownership, borrowing, move, drop | yes | partial | partial | no | no | Design exists; full semantic checking and drop glue are not implemented. |
| Standard library API names | yes | yes | partial | partial | partial | Many `.nocter/std` declarations are placeholders or primitive boundaries. |
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
- immutable local `let` bindings whose initializer is lowerable as `i32` or `bool`
- `void` entry with an empty body or bare `return`
- same-file non-generic tail calls returning `i32` or `bool`
- same-file non-generic normal calls returning `i32` in `let` initializers
- same-file non-generic normal calls returning `i32` in `i32` additions, evaluated left to right with distinct temporary locals
- same-file non-generic normal calls returning `bool` in `let` initializers
- nested `i32` normal-call arguments such as `let value = outer(inner())`
- up to 8 `i32` parameters for lowered functions and calls
- reordered parameter arguments are supported for normal calls and tail calls through argument staging
- non-entry functions returning `bool`
- `i32` addition used in lowerable `i32` expressions
- bool `!`, `&&`, `||`, bool equality/inequality over literal/local operands, and `i32` comparisons used in lowerable bool expressions
- terminal `if` / `else` statements with bool literal, bool local, bool equality/inequality over literal/local operands, or `i32` comparison conditions and direct `i32` or non-entry `bool` returns in both branches
- simple fallible entry success
- simple fallible entry failure through `return make_error("code", "message")`

Currently not buildable even when it may be checkable:

- `var`, reassignment, and general local storage
- general `if`, `while`, `loop`, range `for`, `match`, and pattern conditional `?{}`
- imported function calls
- nested tail-call arguments such as `return outer(inner())`
- normal calls returning `bool` outside `let` initializers, including calls directly inside conditions
- `str` values beyond static failure messages
- optional values
- aggregate values, arrays, views, pointers, methods, traits, generics, ownership lowering, and drop glue
- custom output path selection through `-o`
