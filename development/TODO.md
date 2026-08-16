# Nocter Development Handoff

## Current Task

Continue v0.14.0 Phase 3 by connecting the existing ownership-state lattice to checked control
flow and cleanup planning.
The previous compiler is preserved by commit `f6c08da3` and removed from the active working tree.
No previous source, test, binary behavior, or implementation document may be used as an
implementation input.

## Immediate Work

1. Apply the existing branch join to checked conditionals and loops. The entry state must exclude
   branch-local paths while joining every visible outer path to initialized, uninitialized, or
   maybe initialized.
2. Materialize scope-exit cleanup from the same initialization state. Complete and remaining
   fields must be dropped in normative order without creating a second liveness analysis.

Phase 2 is complete. `lower_compile_unit_declarations` is the sole production declaration facade
and returns one immutable `DeclarationProgram` plus an independent `SourceIndex`. Every facade
failure is exhaustively classified as an authored rule or an internal compiler/discovery integrity
error. Declaration-owned G006-G010, G012-G013, and G015-G018 fixtures compare complete projected
diagnostics under reversed package and module input order. Type equalities are validated after
alias expansion, and projection-free general equalities project `E0320` without retaining syntax
inside canonical requirement identity.
The Phase 3 responsibility map is recorded in `development/docs/checked-program-design.md`.
`DeclarationProgram` now retains authored and prelude-fallback module namespace layers as the sole
body-lookup authority. `nocter-checking` catalogs every `BodyId` from exact source projection and
validates its physical source against the semantic owner module. Missing or inconsistent
projections remain internal boundary errors.
Body-owned resolution now creates dense scope, local, and explicit-capture identities for every
lexical construct. It resolves value uses to parameter, local, capture, exported, or built-in
identity; rejects implicit captures; selects block imports through exact discovery-to-module
projection; extends `SourceIndex`; and compares complete diagnostics under reversed input order.
The synthetic prelude is consistently a shadowable fallback rather than an authored collision
layer.
The program-wide `ConformanceTable` now owns refinement normalization, overlap unification, exact
required/default method selection, signature substitution, conditional requirements, associated
bindings, and associated interface/callable bound proof. Generic matching and bound proof query
that table; they do not reconstruct declaration patterns or rank a more-specific conformance.
One iterative normalized-type validator now covers every declaration-owned data position,
callable result, non-value type operand, borrow/raw-pointer pointee, generic argument, structural
callable, and outcome layer. It is source-independent so concrete substitution can invoke the same
rules before specialization enters checked bodies or later representations.
`PreparedChecking` now owns the single graph/type/conformance/name input after program-wide rules,
while `CheckedProgram` and `CheckedBody` define the syntax-independent output schema. Places and
static dispatch retain exact decisions, and generic arguments are identity-keyed and canonical.
`check_prepared_program` now consumes the preparation state and produces a closed `CheckedProgram`
for the first vertical body slice: scalar literals, inferred locals, copyable parameter/local
places, readonly borrows, binding/discard, return/body-result checking, and recursive outcome
injection. Every typed node receives an exact `BodyNodeId` source projection, and no partial program
escapes an unsupported construct or failed rule. `CopyabilityTable` collects normalized `copy`
proof identities once, memoizes structural outcome/array/borrow/enum and substituted `copy struct`
facts by canonical `TypeId`, closes over the final type store, and remains owned by
`CheckedProgram`. Ordinary structs, unconstrained generics, readwrite borrows, and callable
contracts are never guessed copyable. Copy-struct families retain `Always`, generic `Requires`, or
`Impossible` conditions; an unconditionally move-only field now projects `E0366` at its declaration
instead of creating a never-copy family. Whole-binding state now tracks parameter and local move
paths, emits exact `Move` nodes, rejects moves of copy values and borrow bindings, and reports
later uses through `E0376`-`E0378`. Statically named fields now resolve through one visibility-aware
selector that substitutes the nominal owner's generic arguments and projects the exact field
identity back to source. Move paths retain field identity, preserve disjoint siblings, invalidate
their parent, and join inherited field state without enumerating a struct eagerly. `DropTable` is
the sole nominal-family-to-drop authority; partial moves inspect nearest enclosing families and
project `E0381` with the owning drop declaration. The entry-relative branch join cannot leak
branch-local paths. Annotation binding, calls, operators, aggregates, branches, loops, closures,
literals, and interpolation remain incomplete.

## Guardrails

- Do not restore or inspect the archived compiler.
- Do not migrate archived tests or diagnostics.
- Do not run a released compiler to discover unspecified behavior.
- Do not treat the existing standard-library implementation as language semantics.
- Do not mark specification closure complete while an observable choice remains implicit.
- Do not let Phase 3 reparse declaration headers, infer syntax from resolved names, or place source
  ranges and rendered names in checked semantic identity.

## Verification

```sh
cargo fmt --manifest-path development/compiler/Cargo.toml --all --check
cargo clippy --manifest-path development/compiler/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path development/compiler/Cargo.toml --workspace
node docs/build-docs.js
git diff --check
```
