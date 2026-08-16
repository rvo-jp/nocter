# Nocter Development Handoff

## Current Task

Continue v0.14.0 Phase 3 by defining the closed typed-body node model and construction boundary on
top of the completed name, conformance, and normalized type-position authorities.
The previous compiler is preserved by commit `f6c08da3` and removed from the active working tree.
No previous source, test, binary behavior, or implementation document may be used as an
implementation input.

## Immediate Work

1. Implement expected-type and constraint collection for scalar, reference, aggregate, outcome,
   and callable expressions, then construct the existing closed typed-node schema.
2. Consume every temporary syntax-backed resolved use while building nodes and extend
   `SourceIndex` with exact `BodyNodeId` projections. Do not add expression side maps.

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
No production path constructs checked bodies yet; expected-type and constraint construction is the
active increment.

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
