# Checked Program Design

This document defines the current checked-program implementation boundary. It derives its contract
from the public specification and does not define language behavior. The specification remains
authoritative when this document and a normative rule disagree.

## Boundary

`CheckedProgram` is the first complete, syntax-independent executable-semantics graph. It consumes
the immutable `AcceptedDeclarationProgram` exactly once and owns its `DeclarationGraph` plus
the same `TypeStore` extended with checked-body types. A body, type, callable, or module ID therefore
cannot be paired with declarations from another compile unit, and checking cannot create a parallel
type interner. Every body owns one typed node arena. Nodes contain the exact declaration, local
binding, field, variant, requirement, conversion, dispatch, ownership, loan, provenance, region,
and cleanup decisions selected while checking them.

Syntax trees and source ranges exist only in the checking boundary. Temporary scope tables and
syntax-origin indexes may exist while a body is being checked, but they are consumed before the
`CheckedProgram` is frozen. Source projection remains a separate value that is extended with
checked-node and body-local identities. A canonical checked node never contains a syntax node,
byte range, rendered name, or reverse lookup key.

Authored checking failures use the phase-neutral diagnostic envelope shared by compiler stages.
The checker owns rule selection and projects the retained failing syntax subject exactly once;
diagnostic construction must not rerun lookup, typing, ownership analysis, or source discovery.

The production checker consumes the complete declaration-lowering result and the same explicit
compile-unit syntax snapshots. It locates each body by the existing `BodyId` projection; it never
finds a declaration by source containment. It returns a complete `CheckedProgram` or one typed
authored/internal failure. No public partial checked program exists.

## Authority Map

| Decision | Sole authority | Later consumers |
|---|---|---|
| Packages, modules, declaration identity, header requirements, authored module imports, and prelude fallback | `DeclarationGraph` frozen through `DeclarationProgram` | checker, target validation, instantiation, presentation |
| Header, body, closure, inferred, and specialized structural type identity | the single inherited and extended `TypeStore` | every semantic stage |
| Block imports, lexical scopes, parameters, locals, pattern payloads, catch bindings, loop bindings, and closure captures | body checker | checked nodes and source projection |
| Interface-implementation completeness, normalized signature compatibility, associated binding satisfaction, and overlap | program-wide interface-implementation checker | body dispatch and instantiation |
| Instance target normalization, retained requirements, operation members, and overlap | program-wide instance-operation table | body operation selection and instantiation |
| Data-position type well-formedness after normalization | checked-program type-validity authority | every checked destination and generic constraint |
| Expected types, inference constraints, outcome injection, direct/abstract calls, members, operators, coercions, construction, literals, iteration, and interpolation | typed body node construction | instantiation and MIR |
| Opaque witness selection, interface and associated-binding proof, and public-surface dispatch | checked opaque-witness authority | instantiation and MIR |
| Reachability, initialization, moves, copies, loans, provenance, regions, destruction, and generated semantic operations | checked control-flow and ownership analysis | target validation and MIR |
| Target gates, selected primitive availability, entry validity, and toolchain capability | `TargetProgram` | executable instantiation |
| Concrete generic substitution, requirement proof, interface-implementation dispatch, opaque-witness specialization, and reachable callable graph | executable-program instantiation | MIR |
| Basic blocks, explicit cleanup edges, concrete places, and operation sequencing | MIR | machine lowering |

The checker may record an abstract interface or structural requirement selected for a generic
operation. It must not choose a concrete interface implementation until instantiation supplies a concrete type.
Conversely, MIR never receives a method name or requirement set from which it could repeat dispatch.

## Grammar Conformance Ownership

Grammar semantic boundaries enter checking as follows:

| Rows | Checked-program responsibility |
|---|---|
| G011 | normalized interface implementation signature compatibility and overlap |
| G014 | `void`, `never`, unsized, optional, and fallible data-position validity |
| G019-G021 | body result, assignment, and control-transfer checking |
| G022-G024 | loops, regions, iteration, pattern branches, recovery, and fallback |
| G025-G030 | operators, conversions, moves, calls, construction, literals, spread, and interpolation |
| G031-G032 | explicit closure capture and contextual control-expression typing |
| G033 | contextual source spellings already bound by syntax/declarations; remaining value uses follow ordinary body lookup |

Each row receives a valid, boundary, and invalid case through the production checking facade.
Package and module input permutations must not change the selected semantic target or complete
diagnostic.

## Name Resolution

The immutable declaration program retains two namespace layers for every module:

- authored declarations and imports, including effective visibility and re-exports
- compiler-selected named built-in fallback, which is universal, unshadowable, and never
  exportable
- compiler-selected prelude fallback, which is package-external, shadowable, and never exportable

The body checker consumes those layers directly. It does not reconstruct a namespace by iterating
declarations or imports. Block imports are body-owned because their visibility and collision scope
are lexical; they do not enter `DeclarationProgram` as hypothetical declaration imports.

One temporary scope stack covers parameters, locals, block imports, pattern payloads, catch
bindings, loop bindings, closure parameters, and explicit captures. A declaration records its
semantic identity at the point where its name becomes visible. A reference immediately resolves to
that identity or to one exact module namespace entity. Because Nocter forbids shadowing, insertion
checks every enclosing lexical binding, parameter, authored module name, and built-in type name
before accepting a new visible name. Compiler-selected prelude names remain a distinct fallback
layer and are deliberately shadowable by valid authored or lexical names.

Closure capture lookup is a distinct operation over enclosing callable bindings. The capture list
selects exact outer identities first; the closure body resolves the capture spelling to a new
environment projection identity. Free-name scanning and implicit capture are prohibited.

## Checked Body Shape

Each body owns dense arenas for scopes, local/capture identities, typed nodes, places, and
control-flow edges. Every checked block carries the exact `BodyScopeId` produced by name
resolution. A typed node stores its `TypeId` and one closed operation variant. Examples are
direct call, abstract requirement call, selected coercion, outcome injection, field place, index
place, move, borrow, branch, loop, propagation, cleanup, and terminal operation. Compiler-generated
operations use the same variants and differ only in source role.

Reachability is an explicit control-flow fact, not an absent node. Unreachable source is still
name-resolved and type-checked, but flow-dependent initialization, move, loan, and provenance state
does not invent an incoming executable edge. Scope exit records generated drops in reverse
declaration order and conditional drops for maybe-initialized storage.

`CleanupTable` is a dense checked-node-indexed event annotation. One node may own independent
`AtStatementEnd`, `AtControlHeaderEnd`, `BeforeStore`, `OnOutcomePropagation`, and
`BeforeTransfer` schedules, each with an ordered action list. A target is an owned root plus exact
`FieldId` path, an already evaluated writable `PlaceId`, an evaluated temporary value node, an enum
residual, or a compiler-managed region release. Its condition is `Always` or `IfInitialized`; it
never embeds a source name, syntax range, or
independently inferred liveness bit. Later MIR expands the target type through the program's
`DropTable` and structural drop glue without recovering timing from the node kind.

A lexical region is a checked control node containing its exact context `LocalBindingId`, retained
parent operand, and body node. The context local is never ordinary movable or destructible storage.
Its body scope owns a distinct region-release cleanup target that follows all ordinary body-value
actions. The same target terminates the retained parent loan. Consequently fallthrough, return,
loop transfer, and propagation use one cleanup planner, nested regions release inside-out, and a
diverging edge adds no cleanup. Provenance enters the same region identity as the current allocation
context and validates that no region-derived component crosses the body boundary.

## Construction Order

1. Validate and index every `BodyId` projection against the supplied immutable syntax snapshots.
2. Validate program-wide interface implementations, instance-operation patterns, and normalized type-position
   rules needed by all bodies.
3. Check bodies in canonical `BodyId` order while assigning only body-local dense identities.
4. Infer and validate body-owned callable provenance and opaque witnesses.
5. Freeze all body arenas, consume temporary scope/origin tables, and validate every cross-ID edge.
6. Return `CheckedProgram` plus the extended source projection.

An error before step 5 destroys the builder. A later stage therefore cannot observe a body where
name resolution succeeded but ownership or provenance checking did not.

## v0.14.0 Implementation Record

The following completed increments explain how the boundary was established. They are historical
implementation evidence, not the owner of current milestone status or public behavior.

1. Retain canonical module/prelude namespaces in `DeclarationProgram` and move block-import
   ownership out of declaration imports.
2. Add the checked-program model, source-projection extension, body-source catalog, and exhaustive
   internal boundary validation.
3. Implement lexical declaration/capture identity and value-name resolution.
4. Implement normalized interface implementation and data-position type validity.
5. Implement typed expressions, expected-type inference, calls, members, operators, coercions,
   construction, literals, outcomes, and closures.
6. Implement control flow, reachability, initialization, ownership, loans, provenance, regions,
   destruction, and complete checked-program validation.

An increment is complete only when its superseded temporary authority is consumed, its public
failures retain exact source subjects, and input-order permutation tests pass.

Increment 4 is complete. `InterfaceImplementationTable` is the only structural
dispatch authority: it stores refinement-normalized target/interface patterns, exact default or
implementation method selections, normalized conditional requirements, and associated bindings.
One pattern matcher serves associated-bound proof and future dispatch, while a symmetric unifier
rejects every pair of patterns that can denote one application. The independent iterative
type-position validator classifies data, callable-result, non-value type operand, borrow-pointee,
and pointer-pointee roots. It traverses nested structural types once, validates every declaration
position after alias expansion, and is reused after concrete generic substitution rather than
encoding `void`, `never`, outcome, or unsized exceptions in inference and layout consumers.

Increment 5 now has a closed output schema and a non-output preparation state. `PreparedChecking`
opens `AcceptedDeclarationProgram` once, retains the same extended `TypeStore`, and owns the interface-implementation
table, body-source catalog, resolved lexical identities, temporary syntax-backed uses, and source
projection. It cannot be mistaken for a checked program. `CheckedProgram` has no syntax lifetime
and owns the graph, type store, interface-implementation table, copyability table, type-owned drop table, and one
`CheckedBody` per `BodyId`.

Compiler setup also supplies exact declaration-name tokens for standard semantic roles. Preparation
resolves them through the shared source index, validates standard-package ownership and complete
role-specific declaration shapes, and freezes one `StandardSemanticTable` into the checked
program. Allocation and formatting consumers query semantic IDs from that table; they never search
module paths or declaration spellings. The interpolation contract validates a public, body-backed,
zero-input owned-String constructor and a public readwrite String method accepting one readonly
`str` borrow. A checked interpolation freezes both direct selections alongside each selected
`Format.format_into` dispatch. Typed literal allocation overrides retain one checked place operand,
and every forward analysis visits it before the literal elements.

Each checked body has dense scope, typed local, typed capture, place, loop, and node domains. The
closed node operation distinguishes constants, places, copy/move/borrow, static or callable-value
calls, selected coercions, primitive operations, aggregates, outcomes, closures, typed literals,
iteration/spread, interpolation, and control. `StaticDispatch` is the only operation-selection
edge and records a direct callable, an exact interface requirement plus method, or an exact
structural requirement. A place records its root, final storage authority, and exact
field/implicit-borrow-dereference/builtin-index/selected-index projection path; only an owned
field-only path is an eligible explicit move source.
This schema prevents MIR from repeating member, interface-implementation, coercion, iterator, or move-place
selection.

Generic matching and inference share one iterative structural unifier. Every invocation supplies
the exact `GenericParameterId` set that may receive bindings. A generic identity outside that set
is an opaque term even if a repeated binding later places it on the left of an equation. This
prevents interface-implementation matching from accidentally solving variables owned by its requester and keeps
call inference independent of argument or declaration order.

`CallableInference` collects exact receiver/equality constraints and contextual argument/result
constraints before selection. A result context tries complete expected-type identity first, then
its optional/fallible payload chain from outermost to innermost. The first contextual rank producing
a complete, valid substitution is revalidated by the ordinary expected-type planner. A final
context-free rank is allowed only when argument evidence already determines every generic; the
common expected-type boundary then selects any permitted result coercion or reports the mismatch.
This preserves exact complete-result identity ahead of outcome injection without making generic
inference a second coercion selector. A
statically known optional or fallible parameter shape projects to its payload before ordinary
evidence is unified; a source value already carrying the matching complete layer is unified
exactly. Absence, contextual failure, `never`, and `void` contribute no payload constraint. They are
validated after other evidence has produced a complete substitution. Every inferred argument is
structurally rewritten to its final canonical `TypeId` and rerun through the common data-position
validator, so `void`, `never`, unsized, and invalid outcome substitutions cannot enter a checked
call.

`plan_expected_type` is the single syntax-independent recursive outcome-injection rule used after
inference or other leaf selection. It checks exact complete type identity before opening a layer,
classifies absence, failure, and divergence explicitly, and returns presence/success injections in
inner-to-outer construction order. Binding initializers, arguments, fields, fallbacks, returns, and
body results must consume this plan rather than encode their own optional or fallible cases.

Outcome elimination is equally explicit. `CheckedOutcome::Propagate` retains the immediate
optional/fallible operand layer plus the ordered enclosing success/presence layers required by the
callable result. `Force` retains the immediate layer and deliberately receives no trap cleanup.
`Recover` retains the operand, matching clause kind, optional catch binding, and fallback block.
The fallback is checked against the operated-on payload type; ownership initializes the catch
binding only on failure and joins only normally completing success/fallback paths. Propagation has
its own cleanup event so it can destroy active statement temporaries before ordinary scope and
parameter storage without pretending to be a source `return` node.

The production checked-body slice consumes `PreparedChecking` through
`check_prepared_program`. It builds blocks, scalar constants, inferred local bindings,
parameter/local/named-field places, readonly borrows, explicit discards, simple and compound named-
place assignment, checked integer arithmetic, returns, body results, ordinary `if`/`else if`
control, and
while/infinite/integer-range loops. Bare completion uses an
explicit `Complete` checked operation only when an enclosing fallible success
must be represented; it is never exposed as a source value. Outcome plans become concrete
`CheckedOutcome` nodes from the payload outward. Each constructed node extends `SourceIndex`
directly with `SemanticEntity::BodyNode`; no expression-to-type side map survives construction.

`CopyabilityTable` is the sole copy-proof authority. It collects normalized `copy` requirements by
`GenericParameterId`, classifies ordinary structs and readwrite borrows as move-only, recognizes
payloadless enums and readonly borrows directly, evaluates arrays and optionals structurally, and
classifies `error` plus every fallible layer as unconditionally move-only. Mixed outcomes inherit
that fallible result. `copy struct` specializations remain structural, and nominal field types use
the shared canonical substitution engine.
Every result is memoized by `TypeId`; finalization closes the table over the complete extended type
store before moving it into `CheckedProgram`. Body checking and later stages therefore consume one
fact instead of traversing nominal fields independently. Closure environments remain a checked-
value responsibility because callable signature capability does not determine capture copyability.
The same traversal retains one normalized `Always`, generic `Requires`, or `Impossible` condition
for every `copy struct` family. Preparation rejects an impossible field as source rule `E0366` at
the exact field declaration. A generic-dependent family remains valid and evaluates its condition
again only after canonical argument substitution creates a distinct type identity.

Ownership transfer uses a separate semantic `MovePath` domain keyed by
`PlaceRoot`, never by a source name or place occurrence. Callable and drop parameters enter their
body initialized; a local enters only after its initializer succeeds. Copy and borrow reads require
an initialized path, while `move` requires a move-only owned value and changes the path to
uninitialized before later reachable expressions are checked. Typed-HIR construction performs the
structural move checks and freezes stable node/place/loop identities once. A separate ownership
walker then interprets that immutable HIR, so a fixed-point iteration never rebuilds nodes or
changes semantic identity. Source rules `E0376`-`E0378`
distinguish moving a copy value, moving a borrow binding, and reading an uninitialized path. The
same path domain extends roots with exact `FieldId` projections. Field state is inherited lazily
from the nearest recorded ancestor, so moving one field preserves disjoint siblings but makes the
complete parent unavailable. Branch joins include field overrides visible under entry roots while
excluding branch-local roots; differing incoming states become maybe initialized.

One visibility-aware field selector accepts the canonical base type, substitutes the nominal
owner's actual generic arguments through `TypeSubstitution`, and returns the exact owner, field,
and selected type. Body checking never scans names or fields independently. It also projects each
selected field token to `SemanticEntity::Field`. Borrow layers weaken place access before
selection, so writable field access through `&+T` still cannot become an owned move path.

`DropTable` is the sole nominal-family-to-drop-body association in preparation and the final
checked program. A field move examines its enclosing nominal families from nearest to farthest;
the first family with a type-owned drop body projects `E0381` and the exact drop declaration as a
related source. Cleanup planning can therefore assume a user drop body always receives a complete
`Self`.

Ordinary conditional ownership analysis snapshots state after the condition, checks each branch
from that exact entry, and feeds only normally completing exits to
`OwnershipState::join_reachable`.
The join projects every exit back to entry roots, so a branch-local never escapes even when it is
the only reachable exit. Branches that return or otherwise produce `never` do not create a
continuation. An explicit checked `Unreachable` control operation retains source after a terminal
for diagnostics and editor features while preventing that subtree from contributing
initialization transitions or executable buildability. Type, visibility, requirement, and
structural place checks still run in that subtree.

Each loop owns one reserved `LoopId` before its body is constructed; body construction must define
every reservation before the checked body can freeze. `break` and `continue` carry that exact
identity. Ownership analysis joins the preheader with all normal and `continue` backedges until the
header state stabilizes. It then joins only reachable `break` exits plus the false-condition exit
for `while` and integer ranges. Entry-relative joining filters the per-iteration binding. Range
endpoints execute once, left-to-right, before the preheader; their typed immutable binding becomes
initialized only on the body edge. A nonbreaking infinite loop has no continuation, while an
unreachable `break` does not change its `never` result. Collection iteration remains a subsequent
increment.

Enum pattern control has one checked representation for `if is` and `match`. A subject freezes its
exact enum family and retained-place, consumed-place, owned-temporary, or borrowed preparation.
Each arm stores one `VariantId`, the declared `ParameterId` of every positional payload slot, an
optional branch-local binding for that slot, and its checked body. Fallback reachability and an
implicit unmatched edge are explicit facts. Coverage, exact qualifier and variant selection,
payload arity, generic specialization, and branch result compatibility are therefore complete
before ownership analysis begins.

An owned pattern that transfers a move-only payload from an enum with a type-owned drop body also
freezes that exact `DropId` as a pre-transfer operation. MIR must call it while `Self` is complete.
The later residual action contains only unnamed initialized payload fields and cannot invoke the
drop body twice. When all named payloads are copyable, the complete enum instead remains a normal
value cleanup target.

The same ownership walk materializes cleanup after each operation reaches its final abstract
state. Normal block fallthrough removes that block's locals. `return` removes active scopes from
inner to outer and then owned parameters. `break` and `continue` remove scopes through the target
loop body's exact scope before their states enter the exit or backedge join. Locals are considered
in reverse declaration order. A complete initialized path becomes an unconditional action, a
maybe-initialized path becomes a conditional action, and an uninitialized path produces no action.
When a named-field override makes a struct partial, cleanup recursively follows declared fields in
reverse order and emits actions only for remaining live field paths; the earlier partial-move rule
guarantees no expanded parent has a type-owned drop body. Discarding a move-only expression records
the consumed value node itself, so cleanup does not invent a hidden local or lose the transferred
value.

Temporary liveness is part of `OwnershipState`, not a second expression walker. The body-wide
temporary catalog retains one semantic temporary identity, cleanup action, and creation order;
ordinary values and arm-specific enum residuals occupy distinct identity variants. Branch
joins combine only its initialized state. Callables, owned receivers, arguments, and aggregate
children are activated while their enclosing sequence is incomplete and consumed when it commits.
Borrowed temporary receivers and comparison operands remain active until the enclosing statement
or boolean control header ends. Statement and control-header boundaries clean only identities
created below their entry snapshot, preserving temporaries owned by an enclosing expression.
Branch-only values therefore become `IfInitialized` actions, and reverse catalog order implements
reverse runtime creation order without adding hidden locals.

For an owned enum subject, pattern selection transfers every named payload obligation to its local
and leaves each unnamed move-only payload in an arm-specific `EnumResidual`. A fallback or implicit
non-match path retains the complete active enum. The ordinary flow join makes mutually exclusive
residuals conditional. Statement completion, return, and postfix propagation then use the common
temporary cleanup query; no pattern-only early-exit walker or whole-value double drop exists.

Simple assignment owns one destination place and one RHS node. Construction accepts a complete
`var` binding, a statically named field below it, a built-in fixed-array index below writable owned
storage, or a field/index reached through a readwrite borrow; immutable bindings, owned parameters,
readonly projections, and call-shaped targets project `E0384`.
The ownership walk visits the complete RHS first, then asks the shared cleanup planner for the old
destination state. Initialized values produce unconditional cleanup, maybe-initialized values
produce conditional cleanup, and moved fields produce none. Whole replacement of a partial struct
expands only its remaining fields. A successful transition removes subordinate partial facts and
marks the destination initialized; a field cannot recreate storage below a moved whole parent.
Replacement actions use `BeforeStore`, while scope and transfer cleanup uses
`BeforeTransfer`. Readwrite-borrowed and dynamically indexed targets retain their evaluated
`PlaceId` as the cleanup target because they are not owned `MovePath` identities.

One postfix-place constructor now serves field/index reads, readonly and readwrite borrows, simple
assignment, and compound assignment. It classifies call-shaped syntax once instead of maintaining
an assignment-only syntax walker. Each implicit borrow dereference is a first-class projection;
therefore `owned.borrow_field.member` retains `owned.borrow_field` as its initialization prefix
without pretending that the selected member is owned. Built-in fixed arrays, slices, and `str`
store each checked `usize` index node once and preserve nested source order. Ownership evaluation
visits the RHS before assignment target nodes, and visits target nodes before replacement cleanup
or storage.

For a non-built-in receiver, the constructor queries the program-wide `InstanceOperationTable`.
That table stores each instance target after binder refinement, the declaration's retained
requirements, its operation member identities, and a canonical refinement substitution. It rejects
overlapping target patterns and duplicate receiver-capability/target coercion identities before
any body is checked; declaration order and specificity never rank candidates. A body selector combines the pattern match with lexical assumptions, proves
instance and member requirements, applies visibility, and emits one `StaticSelection`. That value
contains both the direct or structural dispatch identity and every declaration-generic argument,
so instantiation and MIR cannot repeat matching.

The selector owns one recursive requirement-proof context. `copy`, interface, callable, index,
coercion, equality, and ordering predicates therefore share the same lexical assumptions and
concrete dispatch tables. Proving a concrete index, coercion, equality, or ordering requirement
re-enters the ordinary selector, including primitive precedence, direct priority, visibility,
one-step coercion, and ambiguity; it does not use a reduced capability test. An active-predicate
set makes recursive requirement cycles fail closed.

Declaration lookup is restricted to fully concrete receiver types. A receiver that still contains
a lexical generic, interface `Self`, or associated projection can select only an exact lexical
requirement. Concrete instance dispatch is deferred until executable specialization; generic body
checking cannot silently assume that a future type argument will match an instance.

Index selection permits exactly one receiver coercion. A unique direct operation outranks all
coercion-derived candidates; equally ranked candidates are ambiguous. Readonly and readwrite
selections preserve their capability in the place, while writability also depends on the original
receiver storage. A selected index, a selected index after coercion, and a coerced built-in index
are distinct projections. Lexical `where (&C[K]): &V` and `where (&+C[K]): &+V` requirements emit
structural dispatch by exact `RequirementId`. Temporary ownership for unsupported expression
families remains a subsequent increment.

Integer `+`, `-`, `*`, `/`, and `%` select the closed `PrimitiveBinary` operation once. An
authoritative destination integer type contextualizes literal operands; otherwise the typed left
operand contextualizes the right literal. Both operands must have the same integer identity, and
the checked operation retains the result type that later MIR uses for width, signedness, overflow,
division, and remainder guards. Compound assignment stores the same selected operation alongside
one target place and one RHS node. It requires a writable integer target and a matching RHS, then
the ownership walk visits the RHS before requiring the target to be definitely initialized. It
does not build or analyze a duplicate ordinary binary expression. `BodyCheckError` retains the
exact `BodyRule` that produced its separate `SourceDiagnostic`, allowing this boundary to replace a
nested general mismatch with required compound rule `E0386` without inspecting a rendered code.
Prefix `!` and runtime numeric negation select closed unary operations. Directly negated integer
literals instead become one signed mathematical constant after combined range checking, including
the exact minimum of every signed integer type. Shift checking requires one exact integer type for
both operands and freezes signed and unsigned right shift as different operations.

Primitive equality accepts booleans, matching integers, and matching payloadless enums. Primitive
strict ordering accepts matching integers. The same selector then considers an exact lexical
requirement or accessible instance declaration and, only if no viable exact receiver remains, one
readonly receiver coercion. The other operand may use one readonly coercion to the selected owner.
Conditional instance requirements recursively use this same selection operation; unresolved
generic owners require exact lexical evidence.

One comparison node freezes the primitive or `StaticSelection` implementation, readonly
preparation of each source place/temporary/borrow, any coercion attached to each source operand,
left-to-right source evaluation, and independent `reverse` and `negate` derivation bits. Thus `>`,
`<=`, and `>=` remain one strict `<` operation without reversing ownership evaluation or forcing
later lowering to reconstruct semantic arguments. Missing and ambiguous comparison plans are both
`E0389`. `&&` and `||` are checked control nodes, not eager primitive binaries. Ownership evaluates
the left operand, then joins the possible RHS state with the short-circuit bypass state.

A direct module function or primitive call consumes its name-resolution identity rather than
looking up source spelling again. It validates positional arity, checks already-concrete parameter
contexts early, collects generic argument evidence plus one ranked result context, and freezes one
`StaticSelection` only after normalized callable requirements pass through the same recursive
instance-operation proof authority. `none` is retained as deferred evidence and materialized once
the substitution supplies its exact optional type. Generic inference admits the built-in
readwrite-to-readonly relation for both parameter and result evidence without treating the two
borrow types as equal.

One common expected-type boundary first preserves exact complete type identity, then projects
optional and fallible destinations to one leaf. At that leaf it may freeze either built-in
readwrite-to-readonly weakening or one selected source-defined borrow coercion. A
`CheckedBorrowConversion` records the source value or place, exact target, receiver preparation,
and direct or structural dispatch once; coercions never chain. Place arguments are retained as
place drafts until generic inference supplies their final expected type, preventing a readwrite
reborrow from becoming an implicit copy. Minimum-authority selection prefers a readonly receiver
entry and uses a readwrite receiver returning readonly only as a fallback. The checked call stores
arguments in authored order; ownership visits them in that order and applies ordinary copy/move
rules.

A call whose resolved callee is a parameter, local, or capture selects one exact lexical structural
callable requirement for that value type. Its `CallTarget::CallableValue` retains the callee place,
capability, and structural `RequirementId` dispatch. Readonly calls inspect initialized storage,
readwrite calls additionally require a writable place, and owned calls consume the callee before
evaluating arguments. These are invocation preparations rather than copyability claims, so a
move-only readonly callback remains repeatedly callable and a copyable consuming callback still
uses consuming-call semantics. Callable arguments use the same expected-type conversion boundary
as static calls.

`ConstructionSurfaceTable` indexes every nominal family once during preparation and remains part
of `CheckedProgram`. Each entry owns structural field identity and declaration order, intrinsic
enum variants by semantic name, and any validated `construct` declaration. It independently
answers whether an explicit default or empty construct hides raw structural entry outside the
representation source and its direct seers; the common source-access authority still owns
field visibility. Construction-function
calls resolve their owner through semantic module/type identity and select an accessible named
member through that table. Named struct literals and enum variants select their exact field or
variant identities from the same surface. No consumer scans declarations or treats a rendered path
as type identity.

Omitted nominal owner arguments join source-order fields, variant payloads, callable arguments,
and the result context in the common contextual-inference planner. Complete explicit owner
arguments become fixed substitutions first. Construction functions then use declared-call and
callable-requirement planning; aggregates use their field or payload destinations and the same
specialized nominal-requirement proof. Struct aggregates preserve authored field evaluation order
independently of declaration order. Payloadless variants require member syntax, payload variants
require call syntax, and both retain the exact `VariantId`. Fixed arrays either consume a matching
expected element/length contract or infer one data-valid element type, then retain all element
nodes in source order. Aggregate ownership traverses those retained children directly. Each
completed child is staged until construction commits; propagation from a later child cleans
earlier staged children and successful construction consumes them into the resulting aggregate.

Method lookup uses the same normalized instance-operation and interface-implementation authorities. A name-index stored
with `InterfaceImplementationTable` finds interface surfaces without a declaration scan at each call. Exact
receiver lookup combines accessible inherent methods, applicable concrete interface implementation selections,
and lexical interface requirements for unresolved generic receivers; any surviving collision is
ambiguous without signature ranking. Interface `Self`, interface arguments, associated bindings,
instance arguments, and callable arguments enter one owner substitution and the same declared-call
planner. A concrete interface implementation freezes its selected implementation or default body together with
the proven receiver/parameter correspondence between the interface contract and that body.
Provenance analysis consumes this correspondence directly rather than reconstructing parameter
positions from declarations. A generic call retains the exact interface `RequirementId` and method
identity.

Only when exact lookup has no candidate may method selection traverse one borrow coercion.
Readonly coercion receivers form the minimum-authority tier; a readwrite receiver tier is tried
only when the first tier has no route. An exact method shadows every coercion route, and multiple
routes in one tier are ambiguous. `CheckedReceiver` separates the source value from its owned,
place-borrow, temporary-borrow, preserved-borrow, or weakened-borrow preparation. An optional
checked receiver coercion retains its static selection and whether its result borrow is preserved
or weakened for the selected method. Owned methods freeze a copy or move node immediately;
lowering and ownership analysis never reconstruct receiver semantics from syntax or callable
spelling. Program-wide result provenance is a post-ownership checked authority rather than a path
inside call selection. Its fixed point maps exact receiver and parameter origins through direct,
interface, and structural calls while retaining current-allocation dependence as a separate
compiler-owned dimension. Values retain field, variant-payload, outcome, and element projections,
so selecting one component does not acquire sibling origins. The same return validator enforces
authored contracts, inferred contracts, temporary-receiver escape, and interface method bounds.
Closure expressions reserve lexical `ClosureId` identities before type inference and freeze a
structural closure type, normalized callable signature, parameter bindings, capture fields,
callable requirements, and body root in the program-owned `ClosureTable`. A closure type contains
its definition plus the complete enclosing owner-and-callable generic domain. Concrete
substitution therefore fixes environment identity, layout, copyability, and destruction together;
`ClosureId` alone never claims that a generic environment is concrete. Contextual callable contracts and
ordinary argument evidence participate in one dependency-driven inference pass, so closure
parameter and result types do not depend on source argument order. Unannotated results join tail
values, explicit returns, absence, propagation, and divergence without allowing closure control to
escape into the enclosing callable. Capture storage types determine environment copyability;
actual capture access and nested callable invocation determine readonly, readwrite, or owned
invocation capability independently. Ownership, provenance, and loan analyses treat every closure
body as a separate execution root and map parameter, capture-value, and environment-storage
origins through both direct closure calls and structural callable calls. General loan analysis is
now a separate post-provenance authority: reverse structured liveness identifies
the source-level last use of every checked place and node temporary, while forward value flow maps
loans through fields, outcomes, calls, branches, and loops. `LoanRoot::External` distinguishes
input-referenced storage from the place carrying that borrow. Scheduled type-owned drop bodies are
semantic observers, so destruction order participates in the same conflict relation.

Explicit `drop name` reuses the root-place constructor and the cleanup planner. Construction
rejects a copy or borrow target before HIR can claim a destruction operation. On a reachable edge,
ownership analysis requires the exact path to be initialized, attaches one unconditional path
action to the checked drop node, and transitions that path to uninitialized. Scope exit therefore
cannot schedule a second action. Unreachable valid drop source remains typed HIR but receives no
executable cleanup schedule. The loan authority consumes executable cleanup schedules and lexical
storage-end order rather than reconstructing destruction from syntax. Result contracts remain
owned by provenance; lexical, region, temporary-statement, and destination escape use the same
provenance value tree before the program is frozen.

Typed-sequence spread uses the same identity-based operation authorities. Standard `Iterator`, its
`Item` declaration and `next` method, plus `ExactSizeIterator.remaining_len`, enter checking as
exact semantic roles rather than spellings. A dedicated `IteratorAcquisition` node separates the source receiver
from the resulting iterator temporary. Readonly spread selects one readonly expansion; consuming
spread gives a direct iterator fixed priority and consults owned expansion only when no direct
Iterator evidence exists. Direct selection never falls back because exact-size evidence is absent.
`TypedIteration` freezes the acquisition node, specialized `next`, yielded item type, and exact-size
selection. Fixed elements and spreads contribute constraints to one construction inference session
in source order. Ownership transfers each acquired iterator into the argument pack and schedules it
on partial propagation, while provenance and loans derive contributed element storage through the
selected `next` contract. The common `SpreadMode::contribution_type` projection prevents those
analyses from retaining a source loan for a copied scalar or losing it for a retained borrow.

An argument-pack parameter is not an ordinary value of its element type and does not enter
ordinary parameter ownership or cleanup. Checked HIR represents length as
`ArgumentPackLength(ParameterId)`, consumption as an `ArgumentPack` loop whose binding has the
element type, and sole tail forwarding as a `CheckedArgumentPack` edge to its source `ParameterId`.
The declaration model records this distinction as `ParameterRole::ArgumentPack`, not as a flag on
an ordinary parameter, so every downstream consumer must handle the separate contract explicitly.
Any other value use or mixed forwarding projects `E0409`. Provenance and loan flow map both
per-iteration values and forwarded result origins from the pack parameter, while the pack itself
cannot escape, move, borrow, or acquire an ordinary ABI position. This boundary lets executable
construction choose a concrete heterogeneous-source pack representation without changing source
semantics.

Collection `for` consumes the same acquisition and `TypedIteration` facts without reopening
selection or requiring exact-size evidence. Explicit readonly/readwrite modes select only their
matching expansion, moved sources use direct Iterator evidence before owned expansion, and bare
sources admit only direct Iterator evidence under ordinary ownership. The loop-owned iterator is a
retained temporary lifetime rather than an ordinary body temporary: `continue` preserves it,
whereas normal exhaustion and outward transfers clean the current item before the iterator. Nested
loops interleave those lifetimes by body scope. Provenance and loans map the per-step binding
through the selected `next` result, and liveness keeps a borrowed acquisition's source loan active
through every body execution. `CheckedLoop` publishes that body scope as part of the loop contract;
later analyses do not recover it by reopening the block operation. Provenance distinguishes an
ordinary statement temporary from a scoped retained temporary. A yielded borrow from loop-owned
iterator storage may therefore survive every statement in the body and enter nested scopes, but it
cannot enter an outer binding or a callable result.

Authored local and closure annotations enter one body type-use authority. It resolves lexical
generics, `Self`, imports, aliases, nominal arguments, callable types, and requirements into the
shared normalized `TypeStore`, then validates the exact data or callable-result position through
the same type-validity rules used for declarations and concrete substitutions. Local initializers
receive that resolved type through the ordinary expected-type planner, so literals, outcome
injection, borrow weakening, and one selected coercion do not acquire annotation-specific paths.
Type-use resolution, normalized type-shape validity, and initializer compatibility retain distinct
diagnostic families. Discard binding restrictions are checked before initializer construction.

The body builder verifies dense local/capture identity completion before freezing. The production
facade owns the declaration graph, extended type store, interface-implementation table, instance-operation
table, checked-body arena, and source projection only after every body succeeds. Unsupported valid syntax remains an internal
incomplete-implementation error, preventing both a partial program and a misleading source
diagnostic.
