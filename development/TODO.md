# Nocter Development Handoff

This file is the short-lived handoff note for the next compiler session.
Durable design belongs in `development/docs/`; user-facing language rules belong
in `spec/`.

## Current Repository State

- Branch: `develop`
- Release tag: `v0.1.0` points at `660aba7 License Nocter under Apache-2.0`.
- Current development version: `0.2.0-dev`
- Latest known repository-state commits:
  - `8dd1740 Lower match payload expressions`
  - `c96914c Lower match payload bindings`
  - `514a587 Lower if-is payload bindings`
  - `ab2cbc1 Align payload enum match statement lowering`
  - `c1deabe Lower tag-only payload enum match`
  - `ccb2235 Update handoff after payload if-is lowering`
  - `0e0a182 Lower tag-only payload enum if-is`
  - `d2738d5 Promote copy payload enum values`
  - `864230a Define payload enum ABI layout`
  - `2695752 Update handoff after docs merge`
  - `e9cba2b Regenerate docs for v0.2.0 development`
  - `e21231b Merge branch 'main' into develop`
  - `885f3b7 Add GitHub Pages documentation site`
  - `b4510a5 Update handoff after GitHub release publish`
  - `cc1ee22 Update handoff after publishing v0.1.0 tag`
  - `811a78b Update handoff after Apache release retag`
  - `99d1306 Update handoff after v0.2.0 development start`
  - `d817c99 Start v0.2.0 development`
  - `660aba7 License Nocter under Apache-2.0`
  - `e0e47ff Package v0.1.0 release archive`
  - `4018f98 Declare v0.1.0 release scope`
  - `f84d4e7 Pin v0.1.0 release identity metadata`
  - `6b60cd5 Update handoff after fixed array diagnostics`
  - `967ef88 Avoid generic fixed array literal diagnostics`
  - `f722275 Contextualize unsupported fixed array literals`
  - `a31858c Clarify unsupported fixed array literal bindings`
  - `7be19ee Cover alias arms in match expression runtime`
  - `f8d3471 Update handoff after match buildability work`
  - `91fa4c9 Use canonical enum coverage for match buildability`
  - `978c99e Extract completion recovery helpers`
  - `6e28464 Document struct literal completion recovery`
  - `bd3a0c5 Recover struct literal field completions`
  - `e5a4111 Complete struct literal field completions`
  - `4fffdba Recover member completion after trailing dots`
  - `8e6ca70 Complete fields and methods in member contexts`
  - `0c93e11 Cover type member completion in LSP`
  - `3112fba Complete type members in expression contexts`
  - `4d1c676 Keep pattern member completion contextual`
  - `4a31d05 Cover payload if-is binding build boundary`
  - `2d93e2b Complete enum variants in pattern contexts`
  - `3ba1ec4 Cover pattern variants in definition queries`
  - `3b65bd8 Track enum variants in patterns for editor facts`
  - `6c73219 Cover wildcard patterns in AST JSON`
  - `a2a2d12 Lower wildcard-only match without synthetic if`
- The repository root is user-facing. `development/` is the development root.
- The canonical standard-library source lives under `development/std`; local
  release packaging generates `dist/.nocter/std`.
- The active v0 completion definition is `development/docs/v0-closure.md`.
- Current implementation capability is summarized in
  `development/docs/implementation-status.md`.

## Current Priority

Continue `v0.2.0-dev` after the narrow `v0.1.0` release.

Recommended order:

1. Keep `spec/`, `development/docs/v0-closure.md`, and
   `development/docs/implementation-status.md` consistent whenever source
   syntax, standard-library API, ABI behavior, or runtime support changes.
2. Preserve the v0.1.0 boundary: reachable accepted source
   that cannot run must fail before IR or backend emission with a source-backed
   diagnostic.
3. Continue the v0.2.0 payload enum promotion. The first runtime slice covers
   payload-carrying enum construction/local/return/value-argument support for
   copy/no-drop payloads and tag-only payload enum `if is` / `match` statements
   over existing values, plus scalar, string/slice view, and copy aggregate
   payload binding in `if is` statements and value expressions and `match`
   statement/value-expression arms; next, design non-copy payload binding and
   broader cleanup before collection expansion.
4. Continue backend and ABI work around aggregates, ownership cleanup, direct
   and indirect calls, enum payload lowering, and supported collection storage.
5. Continue std runtime work only when the public API is stable in
   `spec/11-stdlib-primitives-os.md`.
6. Keep LSP behavior backed by compiler facts. Do not add editor-only semantic
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
- `std/mem.RawBuffer` is a public owning byte-buffer handle, but its
  `ptr`/`len`/`align` representation fields are `pub(nocter)`. User code must
  use `std/mem` functions and methods instead of forging or inspecting raw
  buffers directly.
- `Vec<T>` supports scalar, `&str`, and current copy-aggregate element storage
  paths. Non-copy aggregate element drop glue, insertion/removal APIs, and
  iteration helpers remain deferred.
- Interface declarations are contract-only. v0 has no interface dispatch,
  generic bounds, trait declarations, or code reuse through interfaces.

## Recent Notes

- `v0.1.0` was verified with `./development/compiler/scripts/verify.sh`, tagged
  at `660aba7`, and packaged locally as
  `dist/nocter-v0.1.0-arm64-darwin.tar.gz`. The package script writes
  `LICENSE` and `NOTICE` into `dist/.nocter/` and the host archive. Develop
  moved on to `0.2.0-dev` in `d817c99`.
- `develop` and `v0.1.0` have been pushed to `origin`. GitHub Release
  `https://github.com/rvo-jp/nocter/releases/tag/v0.1.0` is public with
  `nocter-v0.1.0-arm64-darwin.tar.gz` and `SHA256SUMS` attached. The uploaded
  tarball digest is
  `sha256:88bc55f353f0d78beeb8a953a4e8d65b8a7d43a10860aa04df48d5e6ed1263cb`.
- `main` now contains the v0.1.0 GitHub Pages site under `docs/` in
  `885f3b7`. `assets/logo.svg` moved to `docs/assets/logo.svg`, and the root
  `README.md` uses that logo path. The docs generator intentionally skips
  `development/TODO.md` and `AGENTS.md` as generated pages and maps links to
  skipped source files, such as `development/TODO.md` and `LICENSE`, to their
  GitHub source URLs. After merging `main` into `develop`, `e9cba2b`
  regenerated the HTML from the `v0.2.0-dev` Markdown sources so development
  docs do not present stale v0.1.0-only status text.
- `match` fallback arms now use `_ { ... }`; legacy `match` `else` arms
  reject at parse time. Enum payload discard patterns use `Enum.variant(_)` in
  both `match` and `if is`, and the discard does not introduce a local binding
  into resolver, typecheck facts, hover, ownership, or buildability state.
  Runtime lowering now supports tag-only payload enum `if is` statements and
  value expressions plus `match` statement and value-expression arms over
  existing enum locals/parameters for scalar/string/slice view payload bindings
  and `_` discards. Copy aggregate payload binding is promoted for `if is`
  statements and value expressions and `match` statement/value-expression arms
  over existing enum values. Non-copy aggregate payload binding still rejects
  before IR lowering. Full `./development/compiler/scripts/verify.sh` passes for
  the current payload binding subset.
  Pattern enum variants are recorded in typecheck facts for semantic tokens,
  hover, definition, and references. Completion now offers enum variant members
  after `Enum.` in `match` and `if is` pattern contexts, enum variants and
  associated functions after `Type.`, fields and methods after typed `value.`,
  and struct fields inside struct literal field lists. Open-document completion
  recovers `Type.`, `value.`, and pattern `Enum.` trailing-dot forms, plus
  empty or unclosed struct literal field lists, by inserting a completion-only
  placeholder before single-file analysis.
- Buildability now checks wildcard-free payloadless `match` coverage using the
  resolved canonical enum and covered variant set. Exhaustive `match`
  expressions remain buildable when different visible names, such as an import
  name and an import alias, are used across arms; CLI run coverage pins that
  import-alias match expression path.
- Buildability now treats array literals in fixed-array typed binding,
  assignment, return/fallback, function argument, and struct-field initializer
  contexts as contextual fixed-array values even when the element ABI remains
  outside v0 runtime support. Unsupported aggregate-element arrays now report
  the surrounding binding, assignment, signature, or member E0435 and avoid the
  generic `array literals` fallback diagnostic before IR lowering. CLI build
  coverage pins each reachable position, and full
  `./development/compiler/scripts/verify.sh` passed after `967ef88`.
- Parser diagnostics now reject the reserved `_` and `Self` spellings across
  v0 name-introducing syntax, including declarations, parameters, local
  bindings, payload bindings, import-introduced names, and import aliases.
- Typechecking now accepts `Self` type syntax only in inherent member,
  qualified associated-function, and interface method signature contexts, with
  a source-backed diagnostic in other type positions.
- Resolver diagnostics now reject built-in type spellings as introduced value
  names, and reject reserved type-position spellings such as `i32`, `str`, and
  contextual `error` as type declaration or generic parameter names.
- Typechecking now rejects fallible return success types that are `error`
  directly or through optional success layers such as `error?!`, including
  alias-expanded forms.
- Copyability now treats built-in `error` as copyable and rejects implicit
  copies of move-only owned values across binding, assignment, argument, and
  return paths, including fixed arrays with non-copy elements, optionals,
  fallibles, and payload-carrying enums when the source is an existing binding
  or member path.
- Generic `copy struct` copyability now substitutes concrete type arguments
  before deciding whether an instantiation is copyable, so fields that store
  `T` by value are copy only for copy concrete `T`.
- Backend copy-aggregate classification and `Vec<T>` element-storage
  buildability now use the same substituted `copy struct` copyability, so
  concrete instantiations such as `Box<Text>` do not enter copy-only runtime
  paths when `Text` is move-only.
- Buildability now resolves slice binding, field, and call-result `TypeExpr`
  facts before allowing `Slice(Other)` index reads or assignments, so non-copy
  aggregate slice elements reject with source-backed E0435 before IR lowering.
- Return provenance now treats built-in `error` as borrow-like, rejecting
  returned errors derived from local borrows while allowing errors derived from
  parameter borrows.
- Return provenance now distinguishes static, input-borrow, and escaping sources
  internally and merges aggregate/call inputs, so a static borrow before a local
  borrow cannot hide the escaping local source.
- Return provenance now inspects value-producing `if`, `if is`, `match`, and
  `otherwise` expressions and uses shallow same-AST callable return summaries,
  so local borrows cannot escape through control-expression body results while
  static-returning helpers do not inherit unrelated local borrow arguments.
- Resolver diagnostics now identify synthetic standard prelude name collisions
  explicitly across top-level definitions, parameters, local bindings, and
  block imports, instead of reporting them as ordinary hidden duplicate names.
- Synthetic prelude loading now resolves `std/prelude.nct` directly from the
  active Nocter home, and non-relative imports inside active Nocter home files
  no longer search the user source root. User source imports still keep the
  source-root-before-Nocter-home shadowing rule.
- Frontend validation now rejects non-primitive `pub(nocter)` declarations
  outside the active Nocter home across top-level definitions, struct fields,
  and impl methods. `pub(nocter) primitive` declarations still report through
  the primitive Nocter-home boundary so registry diagnostics stay distinct.
- Fixed array ABI layout/classification is implemented. Runtime currently ships
  local fixed array literals, including zero-length literal bindings,
  aggregate-field fixed array literals, local copy bindings including
  zero-length copies, aggregate-field fixed array value copies in supported
  binding, assignment, argument, return, and aggregate-field initializer
  positions, whole-local assignment including zero-length
  literal/copy/call-result/optional-call-otherwise assignment, fixed array
  value parameters including zero-length values, direct fixed array literal value
  arguments including
  zero-length literal arguments, matching fixed array call-result bindings
  including zero-length results, fallible-call `catch` bindings, whole-local
  assignments, and aggregate-field assignments, optional-call `otherwise`
  fixed array bindings, value arguments, aggregate-field initializers,
  assignments, and returns, and
  direct literal/local/call-result/field fixed
  array returns including zero-length returns for `i32`, `u8`, `usize`, `bool`,
  and `&str` elements. Constant and variable index reads and simple writes build
  and run for local and aggregate-field fixed arrays in the same element subset,
  including fixed array fields inside concrete generic structs, plus constant
  and variable numeric index compound assignment for `i32`, `u8`, and `usize`.
  Move-only element arrays and broader array expressions outside the supported
  binding, assignment, argument, and return positions remain rejected or
  deferred.
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
- Buildability now rejects trusted `std/ptr.from_addr(...)` calls whose address
  expression is statically zero as null raw pointer construction before IR
  lowering. The integer literal decoder is shared with type checking, so
  decimal, hex, binary, underscored zero, and transparent cast spellings use the
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
- Ownership borrow liveness now uses typed terminal-control detection, including
  exhaustive payloadless `match` without wildcard fallback arms, when deciding
  whether later unreachable borrow uses keep a borrow active.
- Return and control-exit terminal analysis now advances a typed lookahead
  environment through block-local bindings before checking later terminal
  statements, so nested branches with local exhaustive enum `match` statements
  no longer report false missing-return diagnostics.
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
  in supported scalar/view value, binding, scalar/view assignment, and return
  positions, and supported aggregate/fixed-array member-root projection,
  binding, argument, aggregate-field initializer, assignment, and return
  positions. Nested/general expression positions and broader fallback
  terminal-control shapes reject before IR lowering with E0435.
- i32 unary negation now lowers as checked `0 - value` for non-literal operands;
  negative integer literals still use the existing constant-literal path.
- Public `std/ptr.from_ref_mut` address conversion is now covered by CLI build,
  CLI run, and distributed-home runtime tests alongside `from_ref`.
- Buildability now resolves imported type aliases by declaring source when it
  decides whether value-producing control expressions are in scalar/view
  binding, assignment, or call-argument positions, and when it classifies
  discardable scalar/view/aggregate expression statements.
- Buildability also resolves imported borrow aliases by declaring source when it
  gates read-write borrow call arguments, so unsupported writable fixed-array
  element borrows reject before IR lowering.
- Slice element kind checks used by compound assignment and read-write element
  borrows now resolve imported call-result aliases by declaring source.
- Local binding, field-member value, and explicit control-flow aggregate move
  preflights now resolve imported nested aliases by declaring source, so
  storage-only scalar locals and unsupported aggregate moves reject before IR
  lowering even when a public alias hides a private alias hop.
- IR type normalization and drop glue resolution now resolve source-qualified
  imported type names without dropping required cleanup for discarded imported
  aggregate call results.
- Buildability and IR reachable-prefix handling now treat exhaustive
  payloadless `match` statements without wildcard fallback arms as proven
  terminal, so unreachable body tails after those statements are neither
  preflighted nor lowered.
- TypecheckFacts now records expression `TypeExpr` values. Buildability and IR
  lowering use the target expression type to accept wildcard-only payloadless
  enum `match` forms while still keeping payload enum `match` binding and
  temporary-pattern control outside the runtime subset.
- Payload-carrying enum ABI layout now backs runtime-supported payload enum
  construction, local slots, returns, value arguments, and tag-only `if is` /
  `match` statements over existing values, including wildcard-only,
  nonexhaustive no-wildcard, and exhaustive no-wildcard statement forms.
  `if is Enum.variant(binding)` statements/value expressions and `match`
  statement/value-expression arms also build/run for `i32`, `u8`, `usize`,
  `bool`, `&str`, and copy aggregate payloads over existing enum bindings and
  parameters.
  Single direct-drop aggregate payload variants now drop only the active payload
  in scope, parameter, discarded call-result, call-result binding, and
  whole-local replacement cleanup paths. Slice and non-copy aggregate payload
  binding, temporary-pattern control, and multi-field droppable payload cleanup
  remain the next promotion boundary.
- Static `error` payload helpers are now limited to input-free function or
  associated-function wrappers. Helpers with parameters and methods returning
  `error` reject before IR lowering so runtime input or receiver evaluation is
  not silently skipped by static payload extraction.
- Imported direct `error` constructors are treated as payload constructors only
  when their ABI is `(&str, &str) -> error`, including source-resolved aliases
  such as `ErrorCode`.

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
