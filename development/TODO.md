# Nocter Development Handoff

## Current Task

Continue v0.14.0 Phase 3 with the general loan authority, then use the completed ownership,
provenance, and loan boundaries to close closure expressions.
The previous compiler is preserved by commit `f6c08da3` and removed from the active working tree.
No previous source, test, binary behavior, or implementation document may be used as an
implementation input.

## Immediate Work

1. Implement source-level non-lexical loan conflicts and destination-lifetime escape checks over
   checked places and control flow.
2. Complete closures through the ownership, provenance, and loan authorities, including explicit
   captures and callable capability.
3. Complete typed literals and interpolation only through the same expected-type,
   temporary-flow, and cleanup authorities already used by ordinary values.

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
that table; they do not reconstruct declaration patterns or rank a more-specific conformance. A
parallel `InstanceOperationTable` is the sole normalized index for instance-owned operations. It
consumes binder refinements and retained predicates once, rejects overlapping instance target
patterns as `E0355`, and supplies identity-keyed generic substitutions to body selection.
One iterative normalized-type validator now covers every declaration-owned data position,
callable result, non-value type operand, borrow/raw-pointer pointee, generic argument, structural
callable, and outcome layer. It is source-independent so concrete substitution can invoke the same
rules before specialization enters checked bodies or later representations.
`PreparedChecking` now owns the single graph/type/conformance/construction-surface/name input after
program-wide rules,
while `CheckedProgram` and `CheckedBody` define the syntax-independent output schema. Places and
static dispatch retain exact decisions, and generic arguments are identity-keyed and canonical.
`check_prepared_program` now consumes the preparation state and produces a closed `CheckedProgram`
for the current vertical body slice: scalar literals, inferred locals, parameter/local/named-field
places, readonly borrows, binding/discard, return/body-result checking, recursive outcome
injection and elimination, `catch`/`otherwise` recovery, ordinary conditionals,
while/infinite/integer-range loops, calls and receiver methods, named construction functions,
named-field struct/enum construction, fixed arrays, and enum pattern control. Every typed node
receives an exact `BodyNodeId` source projection, and no partial program escapes an unsupported construct or
failed rule. `CopyabilityTable` collects normalized `copy`
proof identities once, memoizes structural outcome/array/borrow/enum and substituted `copy struct`
facts by canonical `TypeId`, closes over the final type store, and remains owned by
`CheckedProgram`. Ordinary structs, unconstrained generics, readwrite borrows, and callable
contracts are never guessed copyable. Copy-struct families retain `Always`, generic `Requires`, or
`Impossible` conditions; an unconditionally move-only field now projects `E0366` at its declaration
instead of creating a never-copy family. `ConstructionSurfaceTable` is the sole target-family
index for `construct` declarations and remains in the final checked program for body and editor
queries. Construction calls resolve unqualified or qualified semantic owners, enforce member
visibility, project the exact member identity, infer omitted owner arguments, accept only complete
explicit owner arguments, combine owner and callable generics by identity, and validate both the
callable and specialized nominal requirements through the common proof authority. One enum-only
pattern plan serves both `if is` and `match`. It freezes the target's retained-place,
consumed-place, owned-temporary, or borrowed preparation; exact nominal and variant identity;
positional parameter-to-local binding map; fallback reachability; and unmatched `if is` path.
Coverage rejects duplicate variants, missing variants, and non-final fallbacks. Payload binding
types are specialized from the subject's nominal arguments. Retained places may name only copyable
payloads, while borrowed subjects bind every named payload with the subject borrow capability.
When a type-owned drop body must run before a move-only payload leaves, the pattern freezes its
exact `DropId`; copy-only bindings retain the complete enum for ordinary value cleanup instead.
Whole-binding state now tracks parameter and local move
paths, emits exact `Move` nodes, rejects moves of copy values and borrow bindings, and reports
later uses through `E0376`-`E0378`. Statically named fields now resolve through one visibility-aware
selector that substitutes the nominal owner's generic arguments and projects the exact field
identity back to source. Move paths retain field identity, preserve disjoint siblings, invalidate
their parent, and join inherited field state without enumerating a struct eagerly. `DropTable` is
the sole nominal-family-to-drop authority; partial moves inspect nearest enclosing families and
project `E0381` with the owning drop declaration. The entry-relative branch join cannot leak
branch-local paths. Typed binding annotations, expansion operators, collection iteration, regions,
closures, typed literals, and interpolation remain incomplete.

Typed HIR construction is now independent of flow-dependent ownership. It freezes each body and
its stable node/place/loop identities exactly once; a repeatable ownership analysis then evaluates
that immutable graph. Ordinary `if`, `if is`, `match`, and `else if` join only reachable branch
exits. While, infinite, and integer-range loops use exact `LoopId` targets and a conservative
header fixed point;
zero-iteration exits, `break`, `continue`, and body backedges cannot leak loop-local paths. Range
endpoints are evaluated once before iteration and the typed loop binding is initialized per
iteration. A repeated move is therefore rejected without rebuilding HIR or allocating different
semantic identities on an analysis pass. Unreachable source after a terminal remains under an
explicit `Unreachable` edge. It is still name-, type-, visibility-, requirement-, and structurally
checked but creates no flow-dependent initialization continuation. A fallback after exhaustive
explicit pattern arms is still ownership-checked but cannot create a runtime continuation or loop
edge. Collection iteration, executable regions, closures, typed literals, interpolation, and
general loan conflicts remain incomplete.

Every checked block now retains its exact `BodyScopeId`; name resolution passes that identity
directly into HIR instead of requiring a later syntax or source-index reverse lookup. Ownership
analysis materializes one dense `CleanupTable` keyed by the checked node that owns each scheduled
event. A node may own independent pre-store, statement-end, control-header, propagation, and
control-transfer events; no node kind is asked to imply timing. Pattern residual storage has an
identity distinct from its subject value and from every other arm. Named owned payloads transfer
their obligations to branch locals; only unnamed move-only payload fields remain in the residual
action. A fallback retains the complete active enum. Branch joins make mutually exclusive
residuals conditional, and normal statement, `return`, and postfix-propagation edges consume the
same temporary authority without double drop. Normal block exits, `return`,
`break`, and `continue` all derive cleanup from the same
field-sensitive initialization state. Actions preserve reverse declaration order, distinguish
unconditional from maybe-initialized destruction, omit moved roots and non-owning borrows, expand a
partially moved struct to only its remaining fields, and represent a discarded move-only result as
a value cleanup rather than an invented local. Loop-edge cleanup removes loop-local roots before
the fixed-point join. Simple assignment accepts whole mutable bindings, their statically named
fields, and fields reached through readwrite borrows. It checks the RHS before replacement, applies
the destination expected type, restores moved and maybe-initialized paths, rejects immutable or
unavailable-parent targets, and obtains old-value cleanup from the same partial-path planner used
by scope exit. Each cleanup schedule declares its exact event timing, so later MIR cannot infer
ordering from the node kind. Evaluated owned
temporaries use the same flow state as named paths: call/aggregate staging consumes them on
success, branch joins make one-sided creation conditional, and statement/control-header edges
destroy remaining values in reverse creation order. Postfix propagation owns a distinct failure
edge that destroys active temporaries before scope storage, while forced unwrap and a `never` call
retain the specified no-unwinding behavior. Checked integer
arithmetic selects `Add`, `Subtract`, `Multiply`, `Divide`, or `Remainder` once and evaluates
operands left-to-right. Compound assignment reuses that selection, retains one target and one RHS,
requires a definitely initialized numeric place, and never constructs a fictional binary
expression. Body errors retain their `BodyRule` identity separately from the projected diagnostic,
so the compound boundary can classify its required dedicated diagnostic without comparing rendered
codes. Built-in fixed-array, slice, and `str` indexing now uses the same checked-place constructor
as field reads and borrows. Every implicit borrow dereference is an explicit place projection, so
the owned initialization prefix and final storage authority remain distinct. Index expressions
occur once in projection order. Simple and compound indexed assignment visit the RHS first, then
those index nodes, and retain the evaluated place for pre-store cleanup. Source-defined readonly
and readwrite index operations and the permitted one-step receiver coercion now enter that same
place model. Selection prefers a unique direct operation over coercion paths, rejects equally
ranked paths as `E0388`, and carries one complete `StaticSelection` containing dispatch identity
and generic arguments. Lexical structural index requirements dispatch through their exact
`RequirementId`; concrete instance candidates must satisfy normalized declaration and callable
requirements, while unresolved generic receivers require lexical evidence. Executable MIR
lowering remains incomplete.

Closed prefix, shift, logical, and comparison selection is complete. A directly negated
integer literal becomes one signed `i128`-domain constant, including each exact signed minimum;
runtime negation remains an explicit unary operation. Signed and unsigned right shift are distinct
checked operations. One comparison plan covers primitive, lexical structural, and source-defined
implementations. It freezes readonly place/temporary preparation, readwrite weakening, per-source
one-step coercions, static dispatch, source operands, and independent `reverse`/`negate` derivation
facts. Exact receiver declarations outrank coercion routes; ambiguity is `E0389`. Conditional
equality and ordering requirements recursively re-enter the same selector and fail closed on
cycles. `&&` and `||` remain control nodes whose ownership joins the RHS path with the bypass.

Direct module function and primitive calls are now checked from resolved callable identity. One
ranked `CallableInference` result supplies canonical generic arguments; normalized callable
requirements re-enter the shared instance-operation proof authority. Concrete parameters
contextualize literals before inference, `none` remains deferred until another constraint fixes its
payload, and the result context prefers complete identity before outcome injection. `CheckedCall`
retains exact static dispatch and source-order arguments. Ownership visits the callee value,
receiver, and arguments in language order, so explicit moves and use-after-move
share ordinary place state. The common expected-type boundary now owns exact compatibility,
recursive outcome injection, built-in readwrite-to-readonly weakening, and one-step source-defined
borrow coercion. It records the exact target, source preparation, and static selection in
`CheckedBorrowConversion` and never chains conversions. Readwrite place arguments remain place
drafts through generic inference so a reborrow cannot be misclassified as an implicit copy.
Generic parameter and result evidence admit the same built-in capability weakening. The operation
selector prefers minimum receiver authority, falls back to a readwrite receiver only when required,
and uses lexical coercion requirements in generic bodies. Duplicate coercion identities are
rejected by the program-wide table as `E0356` before body selection. Calls through generic values
now select one exact lexical callable requirement and retain its `RequirementId`, capability, and
callee place. Readonly and readwrite calls borrow the place without copying its environment;
readwrite calls require writable storage. Owned calls consume the callee before their arguments,
independent of closure copyability. Construction functions use that same planner after the
construction-surface table has selected one accessible semantic member. Omitted owner arguments
participate in inference; explicit owner arguments become fixed substitutions before callable
generic inference begins. Receiver methods now use one semantic selector over normalized instance
and conformance tables. Exact lookup combines inherent, concrete conformance/default, and lexical
generic-interface candidates without overload ranking. Interface `Self`, interface arguments,
associated types, instance arguments, and callable generics enter the shared declared-call planner
as one substitution. Only an empty exact set permits one receiver coercion; minimum-authority
coercion tiers, ambiguity, and direct-method priority match other instance operations.
`CheckedCallReceiver` freezes owned copy/move, place or temporary borrowing, existing-borrow
preservation/weakening, selected coercion dispatch, and post-coercion weakening. Concrete calls
freeze their implementation/default callable; generic calls retain the exact interface
requirement. A program-wide provenance fixed point now derives exact caller-visible origins and
compiler-owned current-allocation dependence after ownership has attached cleanup. It retains
field, enum-payload, outcome, and element projections independently, maps results through static
and structural calls, and records a dense node/body/callable authority in `CheckedProgram`.
Return validation rejects local, owned-parameter, temporary, region, unknown, and undeclared input
origins as `E0395`; conformance implementations are additionally bounded by the corresponding
interface method contract. General loan conflicts and closure expressions remain incomplete.

`ConstructionSurfaceTable` now indexes the complete construction surface of every nominal family:
structural field identity and declaration order, enum variants by semantic name, and any authored
`construct` declaration. Structural visibility restrictions from explicit defaults and empty
construct declarations are answered there, while the shared field selector remains the sole
field-visibility authority. Named struct literals, payload and payloadless enum variants, and
fixed-array literals now produce closed aggregate operations. Struct fields and variant payloads
reuse the same source-order contextual-inference planner as callable arguments, so omitted owner
arguments, expected result evidence, deferred absence, explicit moves, and nominal requirements do
not form a parallel inference system. Aggregate ownership traverses retained children in source
order. Earlier initialized children become staged value temporaries until the aggregate commits;
a later propagating child cleans them on its failure edge and successful construction consumes
them into the aggregate.

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
