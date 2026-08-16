# Nocter Development Handoff

## Current Task

Continue v0.14.0 Phase 3 by implementing indexed writable-place planning and the remaining closed
primitive operator families on top of the completed assignment transition.
The previous compiler is preserved by commit `f6c08da3` and removed from the active working tree.
No previous source, test, binary behavior, or implementation document may be used as an
implementation input.

## Immediate Work

1. Build the program-wide instance-operation selector, then use it for source-defined index
   projections and the permitted one-step receiver coercion. It must prove conditional
   requirements without rescanning declarations or ranking candidates by input order.
2. Complete unary numeric, shift, logical, primitive equality, and primitive ordering selection
   without mixing source-defined operator dispatch into the primitive path.
3. Extend the closed checked-operation traversal as calls, aggregates, outcomes, and pattern
   control enter body construction. No new construct may carry a private ownership side channel.

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
for the current vertical body slice: scalar literals, inferred locals, parameter/local/named-field
places, readonly borrows, binding/discard, return/body-result checking, recursive outcome
injection, ordinary conditionals, and while/infinite/integer-range loops. Every typed node receives
an exact `BodyNodeId` source projection, and no partial program escapes an unsupported construct or
failed rule. `CopyabilityTable` collects normalized `copy`
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
branch-local paths. Annotation binding, calls, general operators, aggregates, pattern conditionals,
`match`, loops, closures, literals, and interpolation remain incomplete.

Typed HIR construction is now independent of flow-dependent ownership. It freezes each body and
its stable node/place/loop identities exactly once; a repeatable ownership analysis then evaluates
that immutable graph. Ordinary `if` and `else if` join only reachable branch exits. While,
infinite, and integer-range loops use exact `LoopId` targets and a conservative header fixed point;
zero-iteration exits, `break`, `continue`, and body backedges cannot leak loop-local paths. Range
endpoints are evaluated once before iteration and the typed loop binding is initialized per
iteration. A repeated move is therefore rejected without rebuilding HIR or allocating different
semantic identities on an analysis pass. Unreachable source after a terminal remains under an
explicit `Unreachable` edge. It is still name-, type-, visibility-, requirement-, and structurally
checked but creates no flow-dependent initialization continuation. Collection iteration, pattern
conditionals, and `match` remain incomplete.

Every checked block now retains its exact `BodyScopeId`; name resolution passes that identity
directly into HIR instead of requiring a later syntax or source-index reverse lookup. Ownership
analysis materializes one dense `CleanupTable` keyed by the checked node that owns each scheduled
event. Normal block exits, `return`, `break`, and `continue` all derive cleanup from the same
field-sensitive initialization state. Actions preserve reverse declaration order, distinguish
unconditional from maybe-initialized destruction, omit moved roots and non-owning borrows, expand a
partially moved struct to only its remaining fields, and represent a discarded move-only result as
a value cleanup rather than an invented local. Loop-edge cleanup removes loop-local roots before
the fixed-point join. Simple assignment accepts whole mutable bindings, their statically named
fields, and fields reached through readwrite borrows. It checks the RHS before replacement, applies
the destination expected type, restores moved and maybe-initialized paths, rejects immutable or
unavailable-parent targets, and obtains old-value cleanup from the same partial-path planner used
by scope exit. Each cleanup schedule declares whether it runs before control transfer or before
assignment storage, so later MIR cannot infer ordering from the node kind. Checked integer
arithmetic selects `Add`, `Subtract`, `Multiply`, `Divide`, or `Remainder` once and evaluates
operands left-to-right. Compound assignment reuses that selection, retains one target and one RHS,
requires a definitely initialized numeric place, and never constructs a fictional binary
expression. Body errors retain their `BodyRule` identity separately from the projected diagnostic,
so the compound boundary can classify its required dedicated diagnostic without comparing rendered
codes. Built-in fixed-array, slice, and `str` indexing now uses the same checked-place constructor
as field reads and borrows. Every implicit borrow dereference is an explicit place projection, so
the owned initialization prefix and final storage authority remain distinct. Index expressions
occur once in projection order. Simple and compound indexed assignment visit the RHS first, then
those index nodes, and retain the evaluated place for pre-store cleanup. Source-defined index
selection, remaining operators, temporary cleanup for calls and aggregates, and executable MIR
lowering remain incomplete.

Explicit `drop name` now constructs the same root `CheckedPlace` used by move analysis. Structural
checking rejects copy and borrow bindings as `E0383` even in unreachable source. Reachable drop
requires an exactly initialized path, emits one unconditional path cleanup on the drop node, and
then marks the binding uninitialized; later use and a second drop therefore use the ordinary
`E0378` state rule. Automatic scope cleanup sees the updated state and cannot destroy the binding
again. Loan-conflict checking remains incomplete with the general loan analysis.

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
