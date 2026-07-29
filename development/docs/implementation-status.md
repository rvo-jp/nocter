# Nocter Implementation Status

This document summarizes the current compiler implementation. It is not the
language specification. Normative source-language rules live in
[../../spec](../../spec/README.md). The fixed implementation completion gates
live in [v0-closure.md](v0-closure.md).

## Status Terms

- `specified`: described in `spec/`
- `parsed`: represented in the AST
- `checked`: resolved, typechecked, ownership-checked, or diagnosed before
  lowering
- `buildable`: lowerable through IR and the native backend
- `runtime`: has meaningful native behavior on `arm64-darwin`
- `check-only`: intentionally present for type/API shape, but not runtime-shipped

## Current Summary

Nocter is currently a small native compiler, not just a frontend prototype.
It can load a v0 compile unit, typecheck a meaningful language subset, lower the
runtime subset to IR, emit ARM64 Darwin machine code, write a Mach-O executable,
and run that executable.

The buildable subset remains narrower than the checked subset. Reachable code
outside the runtime subset should fail through buildability diagnostics before
IR or backend emission. Reachable callable signatures, local bindings, and field
member value uses that mention storage-only scalar types such as `u16` or `u32`
are rejected at this boundary instead of surfacing as IR lowering errors.
Bare string interpolation, unsupported fixed-array forms, unsupported explicit
aggregate moves in control-flow conditions, and unsupported outer aggregate
moves/drops inside non-terminal control-flow branches/bodies are also rejected
at the same boundary. Fixed array literal local bindings, including
zero-length literal bindings, aggregate-field fixed array literals, fixed array
local copy bindings including zero-length copies, matching fixed array
call-result bindings including zero-length results, fallible-call `catch`
bindings/assignments including aggregate-field assignments, optional-call
`otherwise` fixed array bindings, value arguments, aggregate-field initializers,
whole-local assignments, aggregate-field assignments, and returns,
aggregate-field fixed array value copies in supported binding,
argument, return, and aggregate-field initializer positions, fixed array value
parameters including zero-length values, direct fixed array literal value
arguments including zero-length literal arguments, direct
literal/local/call-result/field fixed array returns including
zero-length returns, whole-local fixed array assignment including zero-length
literal/copy/call-result/optional-call-otherwise assignment, and
constant and variable index reads and simple writes over local or
aggregate-field fixed arrays are buildable for the current scalar/view element
subset, including fixed array fields inside concrete generic structs, with
constant and variable numeric index compound assignment for `i32`, `u8`, and
`usize`.
Unreachable body tails after proven terminal statements are ignored by return/reachability
checking, buildability, and IR lowering. Buildability and IR lowering consume
structured typechecker facts for binding types and method receiver kinds, and
buildability resolves imported type aliases by their declaring source when
deciding whether value-producing control expressions or discardable expression
statements are in scalar/view runtime positions, and when it gates read-write
borrow call arguments, local binding and field-member value types, explicit
aggregate moves in control-flow positions, and call-result slice element
mutation/borrow positions.
Non-copy aggregate slice indexing and slice index assignment now reject at the
same buildability boundary instead of leaking into IR diagnostics. IR type
normalization and drop glue resolution also handle source-qualified imported
type names when lowering imported call signatures. Hover labels remain editor
presentation data.
Parser diagnostics reserve the `_` and `Self` spellings across v0
name-introducing syntax, including declarations, parameters, local bindings,
payload bindings, import introduced names, and import aliases.
Typechecking accepts `Self` type syntax only in inherent member, qualified
associated-function, and interface method signature contexts; other type
positions receive a source-backed diagnostic instead of normal name lookup.
Resolver diagnostics reject built-in type spellings as introduced value names,
and reject reserved type-position spellings such as `i32`, `str`, and
contextual `error` as type declaration or generic parameter names.
Typechecking rejects fallible return success types that are `error` directly or
through optional success layers such as `error?!`, including alias-expanded
forms.
Copyability treats the built-in `error` type as copyable and rejects implicit
copies of move-only owned values across bindings, assignments, arguments, and
returns, including non-copy structs, fixed arrays with non-copy elements,
optionals, fallibles, and payload-carrying enums where those values are existing
bindings or member paths. Generic `copy struct` instantiations substitute their
concrete type arguments before copyability is decided, so `Box<i32>` can be copy
while `Box<Text>` remains move-only when the field stores `T` by value. The same
substituted copyability feeds backend copy-aggregate classification and
`Vec<T>` element-storage buildability.
Return provenance also treats `error` as borrow-like, so an `error` value
derived from a local borrow cannot escape through a return while parameter-borrow
derived errors can be forwarded.

## Implemented Capability

| Area | Current state |
|---|---|
| Entry and CLI root | `build`, `run`, and `check` use `main.nct` when no file is provided. The root-file `func main` is the executable entry. `fmt` still requires an explicit file. `--entry` is removed. |
| Modules and `use` | File-start bare, selected, grouped, aliased, relative, absolute, source-root, Nocter-home, and `pub use` forms are implemented. User source imports can intentionally shadow Nocter-home paths, while synthetic prelude loading and active-Nocter-home imports are resolved from the active home boundary. Synthetic prelude name collisions are diagnosed as prelude collisions across top-level names, parameters, locals, and block imports. Active-home-only `pub(nocter)` declaration placement is checked for top-level definitions, struct fields, and impl methods. Block-start non-`pub` `use` is implemented as lexical, compile-time-only scope. Legacy `import` / `from` forms are removed syntax. |
| Target and distribution | The active Nocter home comes from `NOCTER_HOME`, otherwise the resolved real compiler executable's parent, so a `PATH` symlink to `.nocter/nocter` works as the normal install shape. Target-dependent std declarations use `#target("arm64-darwin")` inside stable std files. |
| Scalars and strings | `i32`, `usize`, raw pointer address values, `u8`, `bool`, `void`, `never`, `&str`, string literals, byte literals, checked `i32` unary negation, checked `i32`/`usize`/`u8` arithmetic and shifts, numeric whole-binding compound assignment, comparisons, bool operators, and runtime trap checks are implemented in the buildable subset. Narrower/wider integer ABI types such as `u16` and `u32` are currently storage-only inside supported aggregates, not standalone runtime scalar values. |
| Blocks and control expressions | Blocks can produce a value from the final expression. `if`, payloadless enum `if is`, and payloadless enum `match` can be value expressions in the supported subset, including branch-local leading bindings, assignments, and buildable expression statements before the final value. Loops remain statement-oriented in v0. |
| Errors and optionals | `T!`, `T?`, postfix `?`, postfix `!`, `catch`, `none`, and `otherwise` are parsed and checked. `error` is copyable but participates in borrow-like return provenance through its borrowed fields. `catch` blocks that can fall through are rejected by typechecking. Direct `(&str, &str) -> error` constructor calls such as `Error.new(...)`, input-free static `error` helper wrappers, and `error` parameters build and run in the supported fallible subset. Ordinary `error` success-return helper calls with runtime inputs, including function parameters or method receivers, remain outside the runtime ABI subset and reject before IR lowering. Scalar/view and supported aggregate paths build and run when `catch` failure blocks use the current direct-return/effect-only terminal subset and `otherwise` is applied directly to optional-returning calls in supported scalar/view value, binding, assignment, or return positions, and supported aggregate/fixed-array member-root projection, binding, argument, aggregate-field initializer, whole-local or aggregate-field assignment, or return positions. Broader catch/otherwise control-flow endings and nested/general `otherwise` expression positions reject before IR lowering. Nested fallible/optional return shapes remain limited. |
| Struct aggregates | Struct literals, fields including optional-call `otherwise` member roots, copies, explicit moves, direct and indirect aggregate parameters and returns, call-result slots, selected assignments, numeric field compound assignment, replacement drops, and cleanup paths are implemented for the current subset. |
| Enum values | Payloadless enum tag equality, `if is`, and `match` are runtime-shipped. Payload enum construction and checking exist in the frontend; broad payload pattern lowering is still not runtime-shipped. |
| Ownership and drop | The typechecker rejects common use-after-move, double move/drop, invalid drop, borrow conflicts, escaping local borrows, and implicit copies of move-only owned values such as non-copy structs, non-copy concrete generic `copy struct` instantiations, fixed arrays with non-copy elements, optionals, fallibles, and payload-carrying enums. Lowering inserts drop glue for the documented aggregate/control-flow subset. Buildability rejects unsupported explicit aggregate moves in control-flow conditions and outer aggregate moves/drops inside non-terminal control-flow branches/bodies unless that path is one of the current immediate-function-exit forms. |
| Methods and `self` | Inherent associated functions, `method &self`, `method &+self`, consuming receiver syntax, `drop &+self`, and method lookup are implemented for the current call subset. Method receiver kind is recorded in compiler facts for downstream buildability and tooling behavior. |
| Interfaces | Contract-only `interface` declarations and explicit structural `impl Interface for Type` checks are frontend-shipped. Interface values, dispatch, generic bounds, and code reuse are not part of v0. |
| Generics | Generic structs, functions, impl methods, associated functions, enum checks, aliases, and concrete specializations are implemented for the current scalar/view/aggregate subset. Unspecialized reachable generic calls are rejected before backend emission. |
| Slices, fixed arrays, and vectors | Scalar, `&str`, and current copy-aggregate slice indexing and assignment paths are supported, including numeric scalar slice element compound assignment. Fixed array literal local bindings, including zero-length literal bindings, aggregate-field fixed array literals and value copies, local copy bindings including zero-length copies, matching call-result bindings including zero-length results, fallible-call `catch` bindings/assignments including aggregate-field assignments, optional-call `otherwise` fixed array bindings, value arguments, aggregate-field initializers, whole-local and aggregate-field assignments, and returns, value parameters including zero-length values, direct fixed array literal value arguments including zero-length literal arguments, direct literal/local/call-result/field returns including zero-length returns, whole-local assignment including zero-length literal/copy/call-result/optional-call-otherwise assignment, aggregate-field assignment from fixed array literals, locals, call results, fallible call results, optional-call `otherwise` results, and aggregate fields, constant and variable index reads and simple writes over local or aggregate-field fixed arrays for `i32`, `u8`, `usize`, `bool`, and `&str`, including fixed array fields inside concrete generic structs, and constant and variable numeric index compound assignment for `i32`, `u8`, and `usize` build and run. `Vec<T>` supports scalar, `&str`, and promoted copy-aggregate element storage paths, with concrete generic `copy struct` element instantiations accepted only when substituted fields are copyable. |
| Standard library | Tracked `development/std/` contains `error`, `string`, `fmt`, `mem`, `io`, `process`, `vec`, `ptr`, `os`, and `prelude`; local release packaging places it under `dist/.nocter/std`. See [Std Runtime Status](std-runtime-status.md). |
| CLI diagnostics | Text and JSON diagnostics are source-backed where possible. Command-line, filesystem, target, Nocter-home, and formatting diagnostics have stable user-facing messages. |
| LSP | Basic LSP supports initialize, shutdown, full document sync, diagnostics, semantic tokens, hover, definition, references, document symbols, and position-aware basic completion using compiler facts. Block-scope `use` visibility is reflected in completion, references, and semantic tokens. |

## Runtime-Shipped Standard Library

The current distributed std supports:

- `std/error`: built-in `error` construction through ordinary public names
- `std/string`: owned `String`, explicit allocation, views, metadata, reserve,
  clear, append support, bytes view, and drop
- `std/fmt`: append helpers for `&str`, `String`, `i32`, `usize`, and `bool`
- `std/mem`: page allocator, opaque raw buffers with std-internal
  representation fields, and byte-slice views
- `std/io`: `File`, open/read/write/write_text, stdout/stderr, and `print`
- `std/process`: `exit`, `abort`, `cwd`, and `args`
- `std/vec`: construction, capacity, push, from-slice, views, mutation through
  views, clear, reserve, and storage release for shipped element kinds
- `std/ptr`: public `addr`/`from_ref`/`from_ref_mut` address conversions and
  std-internal raw storage boundaries

`std/process.env` is check-only. It reserves the future `&str?!` shape but is
not runtime-shipped.
`std/ptr.from_addr(...)` with a statically zero address is rejected by
buildability because v0 raw pointers are non-null; trusted std code uses a
non-zero sentinel when it needs an empty view that will not be dereferenced.

## Known Runtime Gaps

- targets other than `arm64-darwin`
- broad control-flow lowering outside the documented subset
- payload-carrying enum `if is` and `match` runtime lowering
- ordinary input-dependent `error` success-return helper ABI outside direct
  `(&str, &str) -> error` constructors or input-free static failure-payload
  wrappers
- broader fixed-array behavior: move-only element arrays and general array
  runtime behavior outside the supported binding, assignment, argument, and
  return positions
- broad pointer dereference and user memory mutation APIs
- broad view iteration
- bare string interpolation lowering without an explicit allocator source
- non-copy aggregate `Vec<T>` element storage and per-element drop glue
- insertion/removal/iterator collection APIs
- interface dispatch, interface-bound generics, embedding, and trait-style code
  reuse
- package management, separate compilation, incremental compilation, debug info,
  optimization, dynamic linking, and stable public binary ABI

## Verification

The full v0 closure suite is listed in [v0-closure.md](v0-closure.md). For a
broad compiler change from the repository root, prefer:

```sh
./development/compiler/scripts/verify.sh
```

Use narrower `cargo fmt --manifest-path development/compiler/Cargo.toml --check`
and `cargo test --manifest-path development/compiler/Cargo.toml --lib` runs only
when the change is clearly local.

Documentation-only changes usually need only formatting verification unless
they alter examples, CLI contracts, or generated outputs.
