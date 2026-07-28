# Nocter v0 Closure Definition

This document fixes what "complete" means for the Nocter v0 implementation.
It is an implementation checklist for compiler work, not the public language
specification. Public source-language rules live in
[../../spec](../../spec/README.md).

Changing accepted syntax, observable typechecking, the Nocter ABI, or public
standard-library API must update the relevant spec chapter and this document in
the same commit.

## Target Scope

Nocter v0 completion is scoped to:

- the self-contained Rust bootstrap compiler in this repository
- whole-program builds through the native backend
- the initial `arm64-darwin` target
- the Nocter ABI v0 described in [ABI and Layout](../../spec/09-abi-layout.md)
- a tracked `development/std/` tree packaged into `.nocter/std` whose
  target-dependent declarations use `#target(...)` inside stable module files
- CLI commands: `check`, `build`, `run`, `fmt`, `tokens`, `ast`, `doctor`, and
  `lsp`
- basic LSP behavior that presents compiler-owned facts

Nocter v0 completion does not include:

- Linux, Windows, wasm, or other backend targets
- C ABI compatibility, dynamic linking interop, or stable public binary ABI
- package management, separate compilation, incremental compilation, debug info,
  optimization, self-hosting, or a runtime GC
- trait declarations, embedding declarations, or code reuse through interfaces
- user-defined literal declarations, typed literal construction, generalized
  `...` spread, rest capture, or variadic capture
- editor-only language semantics that differ from compiler semantics

## Status Terms

- `ship`: accepted source works through the named phase.
- `runtime ship`: accepted source builds and runs on `arm64-darwin`.
- `frontend ship`: parser, resolver, and typechecker behavior is complete; no
  backend behavior is required.
- `reject`: source outside v0 produces a stable source-backed diagnostic before
  a later phase can panic or emit an internal unsupported error.
- `defer`: intentionally not part of v0. No implementation is required except
  for parser rejection when the spelling overlaps v0 grammar.
- `narrow`: only the named subset is part of v0.

An area is complete only when every parser-accepted source form in that area is
classified as `ship`, `reject`, or `defer`, and tests lock that decision.

## Global Completion Gates

1. Malformed source must not panic the compiler.
2. Backend lowering must not invent language facts that resolver or typechecker
   did not provide.
3. Reachable user source outside the runtime subset must fail with a user-facing
   diagnostic before machine-code emission.
4. ABI changes must update `../../spec/09-abi-layout.md`, `backend-v0.md`, this
   document, and ABI tests in the same commit.
5. Public std API changes must update
   `../../spec/11-stdlib-primitives-os.md`, `std-runtime-status.md`,
   distributed-home tests, and `development/std/` declarations in the same
   commit.
6. LSP features must consume compiler analysis facts rather than duplicating
   language semantics.

The final closure check is:

```sh
./development/compiler/scripts/verify.sh
```

That script runs the closure commands from `development/compiler/`:

```sh
cargo check
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Closure Matrix

| Area | v0 decision | Complete when |
|---|---|---|
| Source loading, lexing, parser recovery | ship | All v0 tokens and item/expression forms parse or diagnose without panic. Legacy `import` / `from`, wildcard `use` forms, dotted module paths, explicit `.nct` use paths, path-like module expressions, misplaced `use`, block-scope `pub use`, source-level prelude `use` forms, textual include, top-level `trait`, top-level `literal`, embedding declarations, `if let`, `while let`, `if var`, `while var`, `let ... else`, `var ... else`, `??`, and deferred `...` spread/rest/variadic forms produce removed/deferred-syntax diagnostics where they overlap parser grammar. |
| Formatting | ship narrow | Comment-free v0 syntax formats idempotently. Comments may remain a formatter rejection boundary until comment preservation is implemented. |
| Modules, `use`, visibility, prelude | ship | File-start namespace `use path`, selected `use`, grouped `use`, aliases, `pub use`, block-start non-`pub` `use`, private-by-default visibility, `pub`, `pub(nocter)`, source-root lookup, Nocter-home lookup, compiler-managed prelude loading, shadowing rules, and distributed std loading are resolved by compiler-owned facts. |
| Entry and CLI roots | ship | `main.nct` is the default root file for `build`, `run`, and `check`; `fmt` requires a file; root-file `main` is the entry function; `--entry` is rejected. |
| Target gating | ship | Target-specific std declarations use `#target(...)`; target-specific std file names are not required for distribution; unsupported targets diagnose explicitly. |
| Resolver facts | ship | Functions, primitives, namespaces, aliases, structs, enums, interfaces, impl blocks, methods, drop members, fields, variants, and locals resolve or produce source-backed diagnostics. |
| Typechecker facts | ship | Expression types, signatures, ownership, borrowing, drop obligations, interface conformance, enum payload checks, aggregate facts, and tooling facts are available from typechecker output or rejected. |
| Scalar and view values | runtime ship narrow | `i32`, `usize`, raw pointer address values, `u8`, `bool`, `void`, `never`, `&str`, string literals, byte literals, slices, checked `i32` unary negation, checked `i32`/`usize`/`u8` arithmetic and shifts, supported comparison/bool operations, runtime traps, `.len()`, `.is_empty()`, and supported indexing build and run. Broader scalar/view forms, including storage-only integer widths used as standalone values, reject before backend emission. |
| Blocks and control expressions | runtime ship narrow | Body final-expression values, terminal and supported non-terminal `if`, value-producing `if`, payloadless enum `if is` and `match`, value-producing supported `match`, supported leading statements inside value-producing branch blocks, `while`, `loop`, `i32`/`usize` range `for`, `break`, `continue`, `return`, and `never` cleanup build and run in the documented subset. Broader effects and joins reject. |
| Fallible values | runtime ship narrow | `T!`, built-in `error`, static and dynamic error construction in the supported call subset, postfix `?`, postfix `!`, `catch`, and cleanup during propagation build and run for scalar/view/void and supported aggregate paths. `catch` blocks that can fall through reject during typechecking. Runtime-shipped `catch` failure blocks use the current direct-return/effect-only terminal subset; broader terminal control-flow endings reject before IR lowering. Nested fallible/optional return layers remain limited. |
| Optional values | runtime ship narrow | `T?`, `none`, postfix `?`, postfix `!`, and direct-call `otherwise` build and run for scalar/view and supported aggregate values in supported binding and return positions. Broader fallback terminal-control shapes and general `otherwise` expression positions reject before IR lowering. Optional-specific pattern syntax is not part of v0. |
| Aggregates and ABI | runtime ship narrow | Non-generic structs and fully concrete generic structs use correct direct/indirect ABI classification for supported parameters, arguments, returns, field loads/stores, copies, explicit moves, call-result slots, replacement assignments, and cleanup. Fixed arrays use ABI layout/classification and support the current non-empty local-literal/local-copy/call-result-binding/value-parameter/local-or-call-result-return/whole-local-assignment/constant-index-read-write plus numeric-constant-index-compound runtime subset. Payload enum runtime and broader aggregate expressions remain outside runtime closure unless promoted. |
| Field-level ownership | runtime ship narrow | Supported aggregate slot and field replacement paths stage the replacement, drop the old live value when needed, and restore drop obligations. Explicit moves out of fields and broad per-field live-state tracking remain rejected. |
| Ownership, borrowing, move, drop | ship | Frontend rejects use-after-move, double move/drop, invalid drop, borrow conflicts, escaping local borrows, and implicit non-copy aggregate copies for frontend-shipped forms. Runtime lowering inserts drops for the documented subset. Unsupported explicit aggregate moves in control-flow conditions and outer aggregate moves/drops inside non-terminal control-flow branches/bodies reject unless the path matches the current immediate-function-exit subset. Other unsupported move/drop control-flow joins reject before IR or backend emission. |
| Methods | runtime ship narrow | Associated functions, `method &self`, `method &+self`, consuming receivers, method lookup, imported methods, and `drop &+self` work in the supported scalar/view/aggregate call subset. Broader temporary readwrite receivers reject. |
| Interfaces | frontend ship | `interface` declarations, required `pub` members, and explicit structural `impl Interface for Type` conformance are checked. Interface values, dispatch, bounds, inheritance, default methods, embedding, and code reuse are deferred. |
| Generics | runtime ship narrow | Type arity, direct inference, source-aware concrete function/method/associated-function/drop specializations, concrete generic aggregate signatures, and supported generic bodies build and run. Unspecialized calls, generic bounds, and interface-bound dispatch reject or defer. |
| Arrays, pointers, and general views | runtime ship narrow | Type syntax and borrow/view facts are checked. Runtime supports the current scalar, `&str`, and copy-aggregate slice element paths, non-empty local fixed array literals, local copy bindings, matching call-result bindings, value parameters, local/call-result returns, whole-local assignment, constant index reads/writes for `i32`, `u8`, `usize`, `bool`, and `&str`, numeric constant index compound assignment for `i32`, `u8`, and `usize`, public `std/ptr.addr`/`from_ref`/`from_ref_mut` pointer address conversions, and trusted `pub(nocter)` raw-storage pointer primitives. Direct array literal returns, variable index reads/writes, move-only element arrays, pointer dereference, broad storage, and general iteration reject or defer. |
| Standard library | runtime ship narrow | Tracked `development/std/` packages into `.nocter/std` and exposes only APIs classified by `spec/11` and `std-runtime-status.md`; shipped APIs work or fail recoverably; check-only APIs do not fake success; target-dependent declarations use `#target(...)`. |
| CLI diagnostics | ship | `check`, `build`, `run`, and `fmt` render source-backed text diagnostics and JSON diagnostics where supported. Internal compiler errors are not acceptable for source covered by this matrix. |
| LSP | ship basic | LSP initializes, syncs documents, publishes diagnostics, semantic tokens, hover, definition, references, document symbols, and basic completions from compiler facts. Rename, code actions, formatting requests, workspace-wide indexing, and advanced completions are deferred. |
| Documentation | ship | Root `README.md`, `spec/`, `development/README.md`, and `development/docs/` have distinct responsibilities and agree on public syntax, ABI decisions, runtime boundaries, and deferred features. |

## Autonomous Work Order

When continuing without a fresh user choice, close the largest open rows in
this order:

1. Frontend closure audit: every parser-accepted form needs resolver/typechecker
   facts or a stable rejection diagnostic.
2. Backend rejection boundary: replace accidental IR/backend unsupported cases
   with source-backed diagnostics.
3. Aggregate ABI and ownership: broaden field-level state, enum payload facts,
   direct/indirect ABI edges, cleanup, and drop glue.
4. Standard-library runtime: keep `development/std/`, `spec/11`, and
   `std-runtime-status.md` aligned while promoting only stable APIs.
5. Runtime promotion decisions: promote broader optionals, control flow, arrays,
   methods, generics, or collection behavior only by updating this matrix and
   tests in the same commit.
6. Release hardening: run the full closure suite, fix diagnostics, remove stale
   docs, and keep LSP aligned with compiler facts.
