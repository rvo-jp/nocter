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
| `usize` scalar values | yes | yes | yes | partial | partial | Build supports annotated `usize` locals, non-entry `usize` literal/local/call/arithmetic/shift returns, same-file and loaded imported scalar calls with `usize` parameters/arguments, `usize` arithmetic and shifts in lowerable expressions, and `usize` comparisons in lowerable bool positions. |
| `void` entry | yes | yes | yes | yes | yes | Empty body and bare `return` are buildable. |
| `bool` expressions | yes | yes | yes | partial | yes | Build supports literals, locals, parameters, `!`, `&&`, `||`, `i32` and `usize` comparisons, bool equality/inequality over literal/local operands, and bool call arguments in lowerable positions. |
| String literals and interpolation | yes | yes | partial | partial | partial | Single-line and multi-line string literals are tokenized, parsed, typed as `&str`, buildable as `&str` call arguments, buildable as direct non-entry `&str` returns, and buildable through annotated `&str` locals plus narrow `&str` normal-call result staging. Interpolation is parsed as an explicit expression and checked as `String!` with limited supported part types. The explicit `std/mem.page_allocator` + `std/string.with_capacity` + `std/fmt.append_str` + `return move out` shape builds through current stub standard-library bodies, but bare interpolation lowering and allocation-backed string runtime behavior are still disabled. |
| Immutable `let` bindings | yes | yes | yes | partial | yes | Build supports lowerable `i32`, annotated `u8`, annotated `usize`, `bool`, annotated `&str`, annotated slice initializers, narrow aggregate call result bindings used by a same-function aggregate return, direct aggregate normal call-result slots, and aggregate struct-literal locals with word-sized fields. |
| `var` and reassignment | yes | yes | partial | partial | partial | Build supports stack-backed scalar/view `var` initializers and simple whole-binding `=` assignment in the current straight-line leading-statement subset. ABI-indirect aggregate call-result slots, fallible direct aggregate call-result slots, aggregate struct-literal `var` slots, and direct aggregate normal call-result `var` slots are buildable for aggregate borrow arguments and same-function return-by-name. Simple aggregate slot reassignment is buildable from supported struct literals, normal or propagated fallible aggregate call results, and matching copy struct local aggregate slots. Field/index assignment, compound assignment, reinitialization, move-only aggregate slot moves, and drop-aware replacement are not buildable. |
| Same-file function calls | yes | yes | yes | partial | partial | Build supports non-generic tail calls with scalar `i32`/`usize`/`bool` arguments plus `&str` slice arguments when the total register ABI footprint is at most 8 words, `bool` tail returns, and a narrow scalar/view normal-call subset. |
| Imported function calls | yes | yes | yes | partial | partial | Build supports loaded imported non-generic calls through the same narrow call subset as same-file calls. Unloaded imported placeholders still diagnose before backend lowering. |
| General non-tail calls | yes | yes | yes | partial | partial | Build supports non-generic scalar/view normal calls in selected expression positions, including `&str` results staged into annotated `&str` locals or `&str` call arguments. ABI-indirect aggregate normal and propagated fallible calls, plus direct aggregate normal calls, can be staged into reserved aggregate slots for narrow `let`/`var` binding paths, and aggregate slot borrows can be passed as `&T`/`&+T` arguments. Unsupported shapes still report IR lowering diagnostics. |
| Terminal `if` / `else` | yes | yes | yes | partial | yes | Build supports terminal branches returning direct `i32` or non-entry `bool`. |
| `never` termination | yes | yes | partial | partial | yes | Type checking accepts terminal expression statements whose type is `never` and rejects `return` in `never` functions. Build supports lowerable calls returning `never`, including `std/os/macos.trap` and `unreachable` as ARM64 traps. |
| General `if`, `while`, `loop`, `for`, `match`, `?{}` | yes | yes | partial | no | no | Several forms are checkable; backend lowering remains intentionally narrow. |
| Fallible entry success/failure | yes | yes | partial | partial | partial | Build supports success, static `error` constructor failure returns, propagated/caught failures through the current scalar/view/void call subset, propagated ABI-indirect aggregate call-result staging, and propagated direct aggregate call-result staging, reporting `code: message` on stderr. |
| Optional values | yes | yes | yes | no | no | Check examples cover optionals; backend and runtime layout are not implemented. |
| Structs and enums | yes | yes | partial | partial | partial | Type checking exists for several cases. Build supports ABI-indirect and direct struct return signatures, non-entry struct literal returns with word-sized fields, aggregate struct-literal local slots, direct and indirect aggregate return calls, narrow aggregate call result slot returns, aggregate slot borrow arguments, copy struct local slot assignment, and reserved aggregate slot `return move name`; general aggregate values remain unsupported. |
| Arrays, views, and pointers | yes | yes | partial | no | no | Compiler-owned layout and provenance rules are still future work. |
| Methods, traits, and generics | yes | yes | partial | no | no | Parser and selected diagnostics exist; monomorphization/lowering do not. |
| Ownership, borrowing, move, drop | yes | partial | partial | partial | partial | Explicit scalar and aggregate borrow expressions are parsed and checked, `&+` requires a writable `var` binding, `move name` parses and checks the v0 binding-name operand shape, and local scalar plus aggregate slot borrow arguments are buildable for normal calls. Whole-binding assignment from another binding rejects implicit copies of non-copy structs and allows copy structs. Reserved aggregate slot `return move name` lowers through the aggregate return copy path. Full semantic checking, dereference, escapes, use-after-move, general move-only/copy enforcement, moved-value drop suppression, and drop glue are not implemented. |
| Standard library API names | yes | yes | partial | partial | partial | `.nocter/std` includes initial prelude, error, memory, owning string with private `ptr`/`len`/`capacity` fields, formatting, pointer, OS, and I/O declarations with the initial `File` method surface and private close-on-drop state; the active target overlay provides `std/io_impl` raw file-descriptor helpers and `std/process`. Many bodies are placeholders or primitive boundaries. |
| `check --format json` | yes | n/a | yes | n/a | yes | JSON diagnostics are used by corpus tests. |
| `tokens --format json` and `ast --format json` | yes | yes | n/a | n/a | yes | Tooling aids, not stable language compatibility promises. |
| `fmt` | yes | yes | n/a | n/a | partial | v0 rejects files with comments instead of rewriting them. |
| `lsp` | partial | n/a | partial | n/a | partial | JSON-RPC server supports initialize, workspace root recording, shutdown, exit, full document sync, stale version rejection, open-document import reuse, stale diagnostic clearing, UTF-16 diagnostic positions, publishDiagnostics, semantic tokens, Markdown hover for documented declarations, local/top-level/imported references, and loaded import module paths, definition, document symbols, and basic completions. |

## Buildable Subset

The current buildable language subset is intentionally smaller than the checkable subset.
The front end can parse and type-check more Nocter syntax than the backend can lower.

Currently buildable:

- root-file `main` or `--entry <name>`
- custom executable output paths through `build -o <path>`
- explicit `--target arm64-darwin` selection for `build`, `run`, and `check`; reserved future targets are recognized but rejected as unimplemented
- entry return types `i32`, `i32!`, and `void`
- literal `i32` returns
- local `let` and `var` bindings whose initializer is lowerable as `i32`, annotated `u8`, annotated `usize`, `bool`, annotated `&str`, or annotated `&[u8]`/`&+[u8]`
- simple whole-binding `=` assignment to stack-backed scalar/view local bindings in leading statement position
- `void` entry with an empty body or bare `return`
- same-file and loaded imported non-generic tail calls returning `i32` or `bool`
- same-file and loaded imported calls returning `never` in terminal return or expression-statement position
- same-file and loaded imported non-generic normal calls returning `i32` in `let` initializers
- same-file and loaded imported non-generic normal calls returning `i32` in `i32` arithmetic and shift expressions using `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`, evaluated left to right with distinct temporary locals
- same-file and loaded imported non-generic normal calls returning `i32` as `i32` comparison operands such as `if answer() == 42`, `let matched = left() <= right()`, and `return left() < right()`
- same-file and loaded imported non-generic normal calls returning `usize` in annotated `let` initializers and `usize` arithmetic or shift expressions, including calls with scalar arguments
- non-entry functions returning `usize` literal/local/call/arithmetic/shift values in lowerable positions
- `usize` comparisons over literals, locals, lowerable arithmetic or shift expressions, and same-file or loaded imported normal calls in lowerable bool expressions and terminal `if` conditions
- same-file and loaded imported non-generic normal calls returning `bool` in `let` initializers
- same-file and loaded imported non-generic normal calls returning `bool` under unary `!` in `let` initializers and bool return expressions
- same-file and loaded imported non-generic normal calls returning `bool` as atomic bool equality/inequality operands such as `ready() == true` and `left() != right()`
- same-file and loaded imported non-generic normal calls returning `bool` in short-circuit bool value expressions such as `let value = ready() && other()` and `return ready() || other()`
- same-file and loaded imported non-generic normal calls returning `bool` directly in terminal `if` conditions, including unary `!`
- same-file and loaded imported non-generic normal calls returning `bool` in terminal `if` short-circuit conditions such as `ready() && other()` and `ready() || other()`
- short-circuit bool expressions that combine `i32` call comparisons with bool calls, such as `if answer() == 42 && ready()` and `let matched = answer() == 42 && ready()`
- nested scalar normal-call arguments such as `let value = outer(inner())`, for `i32`, `usize`, and `bool` parameter positions
- nested scalar tail-call arguments such as `return outer(inner())`, for `i32`, `usize`, and `bool` parameter positions
- explicit local scalar borrow arguments such as `let result = choose(&value, 42)` and `touch(&+value)`, for `i32`, `u8`, `usize`, and `bool` normal-call parameter positions
- static string literals and `&str` parameters as call arguments, passed as `ptr,len` ABI word pairs
- same-file and loaded imported non-generic normal calls returning `&str` in annotated `&str` `let` initializers and as `&str` call or tail-call arguments, with results staged into two local ABI words
- up to 8 ABI argument words across scalar `i32`/`usize`/`bool`, local scalar borrow, and `&str` parameters/call arguments for lowered functions and calls
- reordered parameter arguments are supported for normal calls and tail calls through argument staging
- non-entry functions returning `bool`, `usize`, or direct `&str` literal/parameter/local/tail-call values
- `i32` arithmetic with `+`, `-`, `*`, `/`, and `%` used in lowerable `i32` expressions; addition, subtraction, and multiplication emit signed-overflow trap checks, and division and remainder emit zero-divisor plus signed-overflow trap checks
- `i32` shifts with `<<` and `>>` used in lowerable `i32` expressions; shift counts trap when negative or greater than or equal to 32
- `usize` arithmetic with `+`, `-`, `*`, `/`, and `%` used in lowerable `usize` expressions; addition traps on carry, subtraction traps on borrow, multiplication traps when the high product word is non-zero, and division and remainder trap on zero divisors
- `usize` shifts with `<<` and `>>` used in lowerable `usize` expressions; shift counts trap when greater than or equal to 64
- bool `!`, `&&`, `||`, bool equality/inequality over literal/local operands, and `i32` or `usize` comparisons used in lowerable bool expressions
- terminal `if` / `else` statements with bool literal, bool local, bool equality/inequality over literal/local operands, or `i32`/`usize` comparison conditions and direct `i32` or non-entry `bool` returns in both branches
- non-entry `never` functions that end with a lowerable call returning `never`
- the `std/os/macos.trap` and `std/os/macos.unreachable` target primitives as ARM64 `brk #0`
- simple fallible entry success
- simple fallible entry failure through a loaded static `error` constructor call with string code and message literals, where the message may be single-line or multi-line
- fallible `?`, `!`, and `catch` lowering for the current scalar/view/void normal-call subset: `i32`, `u8`, `usize`, `bool`, `&str`, slices, and `void`
- `catch` blocks that contain leading scalar/view `let` bindings or void call statements followed by a terminating `return`, including `error.code` and `error.message` payload access
- direct aggregate value parameters, call arguments, and returns for supported non-generic structs up to 16 bytes, including partial final ABI words and shifted or boundary argument-register placement inside the 8-word limit
- indirect aggregate parameters, call arguments, and returns for supported non-generic structs larger than 16 bytes, passed by slot address or returned through caller-provided `x8` storage
- aggregate struct-literal returns and local slots, aggregate call-result slot bindings and assignments, aggregate slot borrow arguments, matching copy-struct slot assignment, and scalar field reads from supported aggregate slots, parameters, and call results
- distributed `std/io.print` execution through the target-overlay `std/io_impl.write_text_raw` bootstrap primitive

Currently not buildable even when it may be checkable:

- compound assignment, field/index assignment, reinitialization after move/drop, drop-aware assignment replacement, aggregate mutable storage, and general local storage beyond the current scalar/view subset
- general `if`, `while`, `loop`, range `for`, `match`, and pattern conditional `?{}`
- unloaded imported function placeholders
- tail calls with borrow arguments, borrow arguments from parameters or non-local places, and dereferencing scalar borrow parameters
- compound bool equality operands with calls such as `(ready() && other()) == true`
- `usize` entry return values
- `&str` member operations and view/byte iteration
- interpolated string construction
- optional values
- general aggregate value expressions outside the supported struct-literal, call-result, slot-copy, and borrow paths; arrays, views, pointers, methods, traits, generics, ownership lowering, and drop glue
