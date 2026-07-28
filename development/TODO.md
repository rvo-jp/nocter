# Nocter Development Handoff

This file is the short-lived handoff note for the next compiler session.
Durable design belongs in `development/docs/`; user-facing language rules belong
in `spec/`.

## Current Repository State

- Branch: `develop`
- Latest known compiler-progress commits:
  - `6ad023a Record binding type expressions in typecheck facts`
  - `7bbcb07 Record method receiver kind in typecheck facts`
  - `b5b280a Skip unreachable body tails consistently`
  - `ba54fbd Preflight aggregate moves in control-flow buildability`
- The repository root is user-facing. `development/` is the development root.
- The canonical standard-library source lives under `development/std`; local
  release packaging generates `dist/.nocter/std`.
- The active v0 completion definition is `development/docs/v0-closure.md`.
- Current implementation capability is summarized in
  `development/docs/implementation-status.md`.

## Current Priority

Keep improving Nocter toward a usable v0 standard-library-driven compiler.

Recommended order:

1. Keep `spec/`, `development/docs/v0-closure.md`, and
   `development/docs/implementation-status.md` consistent whenever source
   syntax, standard-library API, ABI behavior, or runtime support changes.
2. Close buildability gaps before broadening syntax: reachable accepted source
   that cannot run must fail before IR or backend emission with a source-backed
   diagnostic.
3. Continue backend and ABI work around aggregates, ownership cleanup, direct
   and indirect calls, enum payload lowering, and supported collection storage.
4. Continue std runtime work only when the public API is stable in
   `spec/11-stdlib-primitives-os.md`.
5. Keep LSP behavior backed by compiler facts. Do not add editor-only semantic
   rules.

## Known Boundaries

- Target support is `arm64-darwin` only.
- `process.env` keeps the future `&str?!` API shape but is not runtime-shipped.
- Bare string interpolation is parsed and typed, but buildability rejects it
  until an explicit allocator source is designed.
- `std/ptr.addr`, `std/ptr.from_ref`, and `std/ptr.from_ref_mut` are public
  runtime-shipped address conversions. `from_addr` and raw storage helpers
  remain trusted `pub(nocter)` boundaries; pointer dereference is still
  deferred.
- `Vec<T>` supports scalar, `&str`, and current copy-aggregate element storage
  paths. Non-copy aggregate element drop glue, insertion/removal APIs, and
  iteration helpers remain deferred.
- Interface declarations are contract-only. v0 has no interface dispatch,
  generic bounds, trait declarations, or code reuse through interfaces.

## Recent Notes

- Fixed array ABI layout/classification is implemented. Runtime currently ships
  non-empty local fixed array literals, local copy bindings, and constant index
  reads for `i32`, `u8`, `usize`, `bool`, and `&str` elements. Variable
  indexing, mutation/index assignment, array parameters/returns, zero-sized
  local arrays, move-only element arrays, and broader array expressions remain
  rejected or deferred.
- Release packaging layout now separates tracked inputs from generated output:
  `development/std` is the canonical standard-library source,
  `development/packaging` contains release metadata inputs, and
  `development/compiler/scripts/package-local-release.sh` generates
  `dist/.nocter`. Distributed-home tests synthesize a temporary Nocter home from
  those tracked inputs.
- Value-producing `if`, payloadless enum `if is`, and payloadless enum `match`
  branch blocks now lower supported leading bindings, assignments, and
  buildable expression statements before their final value. Buildability
  collects those leading statements and still rejects unsupported branch work
  before IR lowering.
- User-facing parser diagnostics now consistently point to `use` declarations
  instead of describing old import terminology. Block-scope import shadowing
  against parameters and locals is covered by resolver tests.
- CLI coverage now pins bare string interpolation as a source-backed E0435
  buildability rejection before IR lowering, and `fmt` now has coverage for
  block-scope grouped `use` declarations.
- Buildability now rejects trusted `std/ptr.from_addr(0)` as null raw pointer
  construction before IR lowering. The integer literal decoder is shared with
  type checking, so decimal, hex, binary, and underscored zero spellings use the
  same interpretation.
- Buildability signature and std `Vec<T>` element-storage checks now resolve
  substituted `TypeExpr` values by source file. This matters for std generic
  helpers specialized with user project aggregate types, such as
  `Vec<Pair>.push` and `Vec.with_capacity` through a user generic wrapper.
- IR lowering now applies function call specializations before deriving call
  return `TypeExpr` values, so drop glue for aggregate results targets concrete
  drop functions such as `Vec<Pair>.drop` instead of `Vec<T>.drop`.
- Buildability now rejects explicit drops of outer aggregate locals inside
  non-terminal `if`/`match`/loop branches before IR lowering, while still
  allowing branch/body-local explicit drops and outer drops on paths that
  immediately exit the function.
- Buildability now mirrors the current IR boundary for explicit aggregate
  `move` inside control flow: value-producing control conditions, non-terminal
  loop/branch conditions, compound terminal conditions, and unsupported
  non-terminal outer moves reject before IR lowering, while terminal single-call
  conditions and outer moves into bindings/assignments before immediate function
  exit remain buildable.
- Return checking, buildability, and IR lowering now share the same
  reachable-prefix rule for block bodies: statements after a proven terminal
  statement do not force missing-return diagnostics, are not runtime lowered,
  and do not trigger runtime-subset buildability diagnostics.
- Generic impl method specialization now carries the receiver-derived impl
  substitutions into nested generic calls in the method body.
- TypecheckFacts now records method receiver kinds as structured compiler data.
  Buildability uses that fact to identify `&+self` calls instead of parsing
  LSP hover labels.
- TypecheckFacts also records binding `TypeExpr` values. Buildability and IR
  range-for lowering use structured binding/scalar-view facts; `binding_type_label`
  stays presentation-only for hover.
- `catch` blocks that can fall through are now typechecked as E0337. Runtime
  buildability rejects broader catch terminal-control shapes, such as terminal
  `if` inside a catch block, before IR lowering with E0435.
- Runtime `otherwise` is now explicitly gated to direct optional-returning calls
  in supported binding or return positions. General expression positions, such
  as call arguments, and broader fallback terminal-control shapes reject before
  IR lowering with E0435.
- i32 unary negation now lowers as checked `0 - value` for non-literal operands;
  negative integer literals still use the existing constant-literal path.
- Public `std/ptr.from_ref_mut` address conversion is now covered by CLI build,
  CLI run, and distributed-home runtime tests alongside `from_ref`.
- Buildability now resolves imported type aliases by declaring source when it
  decides whether value-producing control expressions are in scalar/view
  binding, assignment, or call-argument positions, and when it classifies
  discardable scalar/view/aggregate expression statements.
- IR type normalization and drop glue resolution now resolve source-qualified
  imported type names without dropping required cleanup for discarded imported
  aggregate call results.

## Session Start

Before editing compiler behavior:

```sh
git status --short
git log --oneline -5
```

Read:

- `README.md`
- `spec/README.md`
- `development/README.md`
- `development/docs/README.md`
- `development/docs/implementation-status.md`
- `development/docs/v0-closure.md`
- the relevant `spec/` chapter for the behavior being changed

Do not revert unrelated user changes.

## Verification

Use the narrowest sufficient command set for the change. For broad shared
compiler work, prefer:

```sh
./development/compiler/scripts/verify.sh
```

For documentation-only changes,
`cargo fmt --manifest-path development/compiler/Cargo.toml --check` is usually
enough unless examples, CLI contracts, or generated outputs were changed.
