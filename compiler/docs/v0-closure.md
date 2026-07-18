# Nocter v0 Closure Definition

This document fixes what "complete" means for Nocter v0 implementation work.
It is meant to stop the finish line from moving while parser, resolver,
typechecker, lowering, backend, runtime, and standard-library work continue.

`../../spec/00-v0-contract.md` is the source-language contract.
`implementation-status.md` records the current implementation state.
This file defines the completion gates. A change that broadens accepted source
syntax, observable typechecking, Nocter ABI, or public `.nocter/std` API must
update this file in the same commit.

## Target Scope

Nocter v0 completion is scoped to:

- the self-contained compiler in this repository
- whole-program builds through the native Mach-O backend
- the initial `arm64-darwin` Nocter ABI
- a distributed `.nocter/std` tree whose target-dependent declarations are
  expressed with `#target(...)`, not target-specific source file names
- CLI `check`, `build`, `run`, `fmt`, `tokens`, and `ast`
- basic LSP behavior that presents compiler-owned facts

Nocter v0 completion does not include:

- Linux, Windows, wasm, or other backend targets
- C ABI compatibility, dynamic linking interop, or stable public binary ABI
- package management, separate compilation, incremental compilation, debug info,
  optimization, self-hosting, or a runtime garbage collector
- `trait` declarations or code reuse through interfaces
- editor-only language semantics that differ from compiler semantics

## Closure Terms

- `ship`: Accepted v0 source must work through the phase named in the row. For
  runtime rows, this means native `build` and `run` behavior on `arm64-darwin`.
- `reject`: Source is outside v0 and must produce a stable, source-backed
  diagnostic before a later compiler phase can panic or emit an internal
  unsupported error.
- `defer`: The feature is intentionally not part of v0. It may stay unparsed or
  parser-rejected. No implementation work is required for v0 closure.
- `frontend ship`: Parser, resolver, and typechecker support are required, but
  no backend behavior is required unless the row also says `runtime ship`.
- `runtime ship`: `check`-accepted source in this row must lower, build, and run.
- `narrow`: Only the subset named in the row is part of v0 completion. Broader
  forms in the same language area must remain rejected or deferred until this
  document promotes them.

An area is complete only when every source form in that area is classified as
`ship`, `reject`, or `defer`, and the corresponding tests lock that decision.
Unknown parser-accepted forms are not allowed at v0 closure.

## Global Completion Gates

These gates apply to every row in the closure matrix.

1. Malformed source must not panic the compiler. It must produce diagnostics with
   source spans when a span can be recovered.
2. The backend must not infer language semantics that are missing from resolver
   or typechecker facts. If typed facts are insufficient, frontend rejection is
   required before lowering.
3. User source that is outside the runtime-shipped subset must fail with a
   user-facing diagnostic. It must not reach Mach-O emission as an accidental IR
   or backend `unsupported` case.
4. ABI changes require updates to `../../spec/09-abi-layout.md`,
   `backend-v0.md`, this file, and ABI tests in the same commit.
5. Standard-library public API changes require distributed std tests and
   documentation updates in the same commit.
6. The final v0 closure check must pass:

```sh
cargo fmt --manifest-path compiler/Cargo.toml --check
cargo test --manifest-path compiler/Cargo.toml --lib
cargo test --manifest-path compiler/Cargo.toml --test cli_build
cargo test --manifest-path compiler/Cargo.toml --test cli_run
cargo test --manifest-path compiler/Cargo.toml --test distributed_home
cargo test --manifest-path compiler/Cargo.toml --test cli_fmt
cargo test --manifest-path compiler/Cargo.toml --test cli_lsp
cargo test --manifest-path compiler/Cargo.toml --test example_corpus
```

## Slice Completion

Frontend v0 is complete when every parser-accepted source form either has
resolver and typechecker facts described in `../../spec/00-v0-contract.md`, or
is rejected before lowering with a stable diagnostic. Frontend completion may be
reached before runtime support for every checked form.

Backend v0 is complete when every `runtime ship` row builds and runs on
`arm64-darwin`, and every parser/typechecker-supported but non-runtime row has a
stable rejection boundary before machine-code emission.

Standard library v0 is complete when `.nocter/std` exposes only v0 APIs, target
dependencies use `#target(...)`, and every public API either has a working body
or returns a documented recoverable error by design.

Full Nocter v0 is complete when frontend v0, backend v0, standard library v0,
the CLI gates, and the documentation gates are all complete.

## Closure Matrix

| Area | v0 decision | Complete when |
|---|---|---|
| Source loading, lexing, parser recovery | ship | All v0 tokens and item/expression forms parse or diagnose without panic. Removed `import`/`from` and top-level `trait` syntax diagnose as removed syntax. |
| Formatting | ship narrow | Comment-free v0 syntax formats idempotently. Comments may remain a v0 rejection boundary for `fmt` until comment preservation is designed. |
| Modules, `use`, visibility, prelude | ship | `use path.name`, grouped `use`, aliases, `pub use`, private-by-default visibility, `pub`, `pub(nocter)`, and distributed std loading are resolved by compiler-owned facts. Old `import` syntax stays rejected. |
| Target gating | ship | Target-specific std declarations use `#target(...)`; target-specific file names under `.nocter/std` are not required for distribution. Unsupported targets diagnose explicitly. |
| Resolver facts | ship | All functions, primitives, namespaces, aliases, structs, enums, interfaces, impl blocks, methods, drop members, and locals resolve or produce source-backed diagnostics. LSP consumes these facts instead of duplicating lookup. |
| Scalar types and scalar expressions | runtime ship | `i32`, `usize`, `u8`, `bool`, `void`, `never`, scalar locals, scalar calls, scalar arithmetic, comparisons, bool operations, and v0 runtime trap checks build and run in the documented subset. Unsupported scalar shapes are rejected before backend emission. |
| Static strings and views | runtime ship narrow | String literals type as `&str`, static `&str` values pass and return through the documented two-word ABI, and supported byte-slice views used by std I/O build and run. General view iteration is deferred unless promoted by this document. |
| Owned `String` | runtime ship | `String` remains an ordinary std type with private pointer, length, and capacity fields; explicit allocation-backed construction, view access, formatting append, move, return, and drop run through distributed std tests. Bare interpolation is rejected by the buildability preflight until an explicit allocator source is designed. |
| Fallible values | runtime ship narrow | Entry `T!`, static `error` failures, propagation, forced unwrap, and `catch` work for the scalar/view/void and supported aggregate call-result subset. Other fallible payload shapes reject before backend emission. |
| Optional values | runtime ship narrow | Optional scalar/view and supported aggregate success/none returns, force unwrap, `let ... else`, and `??` defaults build and run in the documented subset. Optional `if let` and `while let` branches remain frontend-only and are rejected by build until promoted. |
| Control flow | runtime ship narrow | Terminal `if`, supported non-terminal `if`, `while`, and `loop`, `break`, `continue`, `return`, and `never` cleanup build and run in the documented subset. `if let`, `if is`, `while let`, range `for`, full `match`, and pattern conditional `?{}` are frontend-only or rejected by build until promoted. |
| Aggregates and ABI | runtime ship narrow | Non-generic struct layout, direct and indirect aggregate parameters, arguments, returns, call-result slots, supported struct literals, field stores, field reads, aggregate copies, explicit moves, and supported aggregate assignments match Nocter ABI v0 tests. Optional, full enum payload runtime, arrays, and general aggregate expressions remain outside runtime closure unless promoted. |
| Ownership, borrowing, move, drop | ship | Typechecking rejects use-after-move, double move/drop, invalid explicit drop, escaping local borrows, borrow conflicts, and implicit non-copy aggregate copies for all frontend-shipped forms. Runtime lowering inserts drops for the documented aggregate/control-flow subset and rejects remaining cases before backend emission. |
| Field-level ownership state | reject until promoted | Non-copy field assignment, field moves, and field reinitialization stay rejected with stable diagnostics. If broader aggregate mutation is promoted, field-level live/drop state and tests are required first. |
| Arrays, raw pointers, and general views | frontend ship narrow, backend reject | Type syntax, borrow/view facts, and primitive pointer boundaries are checked where specified. By-value array literals are rejected by the buildability preflight. General view iteration, pointer dereference, and storage-dependent runtime behavior are deferred or rejected before lowering. |
| Methods | frontend ship, backend reject narrow | Inherent method declarations, receiver rules, method resolution, drop members, and associated functions are checked. Runtime method calls are required only for std-supported paths; general method lowering may reject before backend emission. |
| Interfaces | frontend ship | `interface` declarations, `pub` interface methods, and explicit structural `impl Interface for Type` conformance are checked. Interfaces provide no v0 code reuse, dynamic dispatch, generic bounds, or backend dispatch requirement. |
| Generics | frontend ship narrow, backend reject | Type arity, direct generic inference, generic aggregate substitution, and generic enum checks are frontend responsibilities. Reachable generic functions and generic impl members are rejected by the buildability preflight. Monomorphization is the intended future backend direction but is not required for v0 runtime closure unless promoted. |
| Standard library primitives | runtime ship narrow | Trusted primitives used by distributed std have explicit target gates, privacy boundaries, typechecked declarations, and native lowering or documented recoverable failure. Placeholder public APIs must not silently succeed. |
| `std/process` | runtime ship narrow | Process exit/status behavior works. `cwd`, `args`, and `env` must either return owned std values through explicit allocator-aware APIs or return documented recoverable unsupported errors until their runtime path is implemented. |
| `Vec` and collections | defer | `Vec` may be specified as the owned variable-length array direction, but a general collection implementation is not required for v0 runtime closure unless needed by a promoted std/process API. |
| CLI diagnostics | ship | `check`, `build`, and `run` render source-backed diagnostics in text and JSON where supported. Internal compiler errors are not acceptable for user source covered by this document. |
| LSP | ship basic | LSP initializes, syncs documents, publishes compiler diagnostics, semantic tokens, hover, references, definition, document symbols, and basic completions from compiler facts. Richer editor behavior is not a v0 closure blocker. |
| Documentation | ship | `spec`, `implementation-status.md`, `backend-v0.md`, `roadmap.md`, and this document agree on public syntax, ABI decisions, runtime boundaries, and deferred features. |

## Autonomous Work Order

When continuing without a fresh user choice, close the largest open rows in this
order:

1. Frontend closure audit: enumerate parser-accepted forms and add either
   typechecker coverage or stable rejection diagnostics.
2. Backend rejection boundary: replace accidental IR/backend unsupported cases
   with source-backed diagnostics for non-runtime rows.
3. Aggregate ABI and ownership: close field-level ownership state, enum payload
   facts, drop glue, direct/indirect ABI edge cases, and aggregate cleanup.
4. Standard library runtime: finish allocator behavior, owned `String`, `fmt`,
   `process.cwd`, then only add `Vec`, `args`, and `env` if their public API is
   stable enough to keep.
5. Runtime promotion decisions: promote optionals, full control flow, arrays,
   methods, or generics only by changing the relevant matrix row and adding
   tests in the same commit.
6. Release hardening: run the full closure command set, fix diagnostics, remove
   stale docs, and keep LSP aligned with compiler facts.

Do not add a new language feature merely because it is convenient for the next
test. First classify it in this document, then implement the smallest behavior
that closes that row.
