# Checked Program Design

This document assigns implementation responsibility for v0.14.0 Phase 3. It derives work from the
public specification and does not define language behavior. The specification remains authoritative
when this plan and a normative rule disagree.

## Boundary

`CheckedProgram` is the first complete, syntax-independent executable-semantics graph. It consumes
the immutable `DeclarationProgram` from Phase 2 exactly once and owns its `DeclarationGraph` plus
the same `TypeStore` extended with checked-body types. A body, type, callable, or module ID therefore
cannot be paired with declarations from another compile unit, and Phase 3 cannot create a parallel
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
| Conformance completeness, normalized signature compatibility, associated binding satisfaction, and overlap | program-wide Phase 3 conformance checker | body dispatch and instantiation |
| Data-position type well-formedness after normalization | Phase 3 type-validity checker | every checked destination and generic constraint |
| Expected types, inference constraints, outcome injection, direct/abstract calls, members, operators, coercions, construction, literals, iteration, and interpolation | typed body node construction | instantiation and MIR |
| Reachability, initialization, moves, copies, loans, provenance, regions, destruction, and generated semantic operations | checked control-flow and ownership analysis | target validation and MIR |
| Target gates, selected primitive availability, entry validity, and toolchain capability | `TargetProgram` | executable instantiation |
| Concrete generic substitution, requirement proof, conformance dispatch, opaque witness, and reachable callable graph | executable-program instantiation | MIR |
| Basic blocks, explicit cleanup edges, concrete places, and operation sequencing | MIR | machine lowering |

The checker may record an abstract interface or structural requirement selected for a generic
operation. It must not choose a concrete conformance until instantiation supplies a concrete type.
Conversely, MIR never receives a method name or requirement set from which it could repeat dispatch.

## Grammar Conformance Ownership

The remaining grammar semantic boundaries enter Phase 3 as follows:

| Rows | Phase 3 responsibility |
|---|---|
| G011 | normalized conformance signature compatibility and overlap |
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
- compiler-selected prelude fallback, which is shadowable and never exportable

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

`CleanupTable` is a dense checked-node-indexed edge annotation. A cleanup target is either an owned
root plus exact `FieldId` path or a consumed temporary value node. Its condition is `Always` or
`IfInitialized`; it never embeds a source name, syntax range, or independently inferred liveness
bit. Later MIR expands the target type through the program's `DropTable` and structural drop glue.

## Construction Order

1. Validate and index every `BodyId` projection against the supplied immutable syntax snapshots.
2. Validate program-wide conformance and normalized type-position rules needed by all bodies.
3. Check bodies in canonical `BodyId` order while assigning only body-local dense identities.
4. Infer and validate body-owned callable provenance and opaque witnesses.
5. Freeze all body arenas, consume temporary scope/origin tables, and validate every cross-ID edge.
6. Return `CheckedProgram` plus the extended source projection.

An error before step 5 destroys the builder. A later stage therefore cannot observe a body where
name resolution succeeded but ownership or provenance checking did not.

## Phase 3 Increments

1. Retain canonical module/prelude namespaces in `DeclarationProgram` and move block-import
   ownership out of declaration imports.
2. Add the checked-program model, source-projection extension, body-source catalog, and exhaustive
   internal boundary validation.
3. Implement lexical declaration/capture identity and value-name resolution.
4. Implement normalized conformance and data-position type validity.
5. Implement typed expressions, expected-type inference, calls, members, operators, coercions,
   construction, literals, outcomes, and closures.
6. Implement control flow, reachability, initialization, ownership, loans, provenance, regions,
   destruction, and complete checked-program validation.

An increment is complete only when its superseded temporary authority is consumed, its public
failures retain exact source subjects, and input-order permutation tests pass.

Increment 4 is complete. `ConformanceTable` is the only structural
dispatch authority: it stores refinement-normalized target/interface patterns, exact default or
implementation method selections, normalized conditional requirements, and associated bindings.
One pattern matcher serves associated-bound proof and future dispatch, while a symmetric unifier
rejects every pair of patterns that can denote one application. The independent iterative
type-position validator classifies data, callable-result, non-value type operand, borrow-pointee,
and pointer-pointee roots. It traverses nested structural types once, validates every declaration
position after alias expansion, and is reused after concrete generic substitution rather than
encoding `void`, `never`, outcome, or unsized exceptions in inference and layout consumers.

Increment 5 now has a closed output schema and a non-output preparation state. `PreparedChecking`
opens `DeclarationProgram` once, retains the same extended `TypeStore`, and owns the conformance
table, body-source catalog, resolved lexical identities, temporary syntax-backed uses, and source
projection. It cannot be mistaken for a checked program. `CheckedProgram` has no syntax lifetime
and owns the graph, type store, conformance table, copyability table, type-owned drop table, and one
`CheckedBody` per `BodyId`.

Each checked body has dense scope, typed local, typed capture, place, loop, and node domains. The
closed node operation distinguishes constants, places, copy/move/borrow, static or callable-value
calls, selected coercions, primitive operations, aggregates, outcomes, closures, typed literals,
iteration/spread, interpolation, and control. `StaticDispatch` is the only operation-selection
edge and records a direct callable, an exact interface requirement plus method, or an exact
structural requirement. A place records its owned or borrowed root and exact field/builtin-index/
selected-index projection path; only an owned field-only path is an eligible explicit move source.
This schema prevents MIR from repeating member, conformance, coercion, iterator, or move-place
selection.

Generic matching and inference share one iterative structural unifier. Every invocation supplies
the exact `GenericParameterId` set that may receive bindings. A generic identity outside that set
is an opaque term even if a repeated binding later places it on the left of an equation. This
prevents conformance matching from accidentally solving variables owned by its requester and keeps
call inference independent of argument or declaration order.

`CallableInference` collects exact receiver/equality constraints and contextual argument/result
constraints before solving once. A statically known optional or fallible parameter shape projects
to its payload before ordinary evidence is unified; a source value already carrying the matching
complete layer is unified exactly. Absence, contextual failure, `never`, and `void` contribute no
payload constraint. They are validated after other evidence has produced a complete substitution.
Every inferred argument is structurally rewritten to its final canonical `TypeId` and rerun through
the common data-position validator, so `void`, `never`, unsized, and invalid outcome substitutions
cannot enter a checked call.

`plan_expected_type` is the single syntax-independent recursive outcome-injection rule used after
inference or other leaf selection. It checks exact complete type identity before opening a layer,
classifies absence, failure, and divergence explicitly, and returns presence/success injections in
inner-to-outer construction order. Binding initializers, arguments, fields, fallbacks, returns, and
body results must consume this plan rather than encode their own optional or fallible cases.

The production checked-body slice consumes `PreparedChecking` through
`check_prepared_program`. It builds blocks, scalar constants, inferred local bindings,
parameter/local/named-field places, readonly borrows, explicit discards, returns, body results,
ordinary `if`/`else if` control, and while/infinite/integer-range loops. Bare completion uses an
explicit `Complete` checked operation only when an enclosing fallible success
must be represented; it is never exposed as a source value. Outcome plans become concrete
`CheckedOutcome` nodes from the payload outward. Each constructed node extends `SourceIndex`
directly with `SemanticEntity::BodyNode`; no expression-to-type side map survives construction.

`CopyabilityTable` is the sole copy-proof authority. It collects normalized `copy` requirements by
`GenericParameterId`, classifies ordinary structs and readwrite borrows as move-only, recognizes
payloadless enums and readonly borrows directly, and evaluates arrays, outcomes, and `copy struct`
specializations structurally. Nominal field types use the shared canonical substitution engine.
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
unreachable `break` does not change its `never` result. Collection iteration, pattern
conditionals, and `match` remain subsequent increments.

The same ownership walk materializes cleanup after each outgoing edge reaches its final abstract
state. Normal block fallthrough removes that block's locals. `return` removes active scopes from
inner to outer and then owned parameters. `break` and `continue` remove scopes through the target
loop body's exact scope before their states enter the exit or backedge join. Locals are considered
in reverse declaration order. A complete initialized path becomes an unconditional action, a
maybe-initialized path becomes a conditional action, and an uninitialized path produces no action.
When a named-field override makes a struct partial, cleanup recursively follows declared fields in
reverse order and emits actions only for remaining live field paths; the earlier partial-move rule
guarantees no expanded parent has a type-owned drop body. Discarding a move-only expression records
the consumed value node itself, so cleanup does not invent a hidden local or lose the transferred
value. Assignment/reinitialization and temporary ownership for unsupported expression families
remain subsequent increments.

Explicit `drop name` reuses the root-place constructor and the cleanup planner. Construction
rejects a copy or borrow target before HIR can claim a destruction operation. On a reachable edge,
ownership analysis requires the exact path to be initialized, attaches one unconditional path
action to the checked drop node, and transitions that path to uninitialized. Scope exit therefore
cannot schedule a second action. Unreachable valid drop source remains typed HIR but receives no
executable cleanup edge. Loan conflicts, assignment, and reinitialization remain subsequent
increments.

The body builder verifies dense local/capture identity completion before freezing. The production
facade owns the declaration graph, extended type store, conformance table, checked-body arena, and
source projection only after every body succeeds. Unsupported valid syntax remains an internal
incomplete-implementation error, preventing both a partial program and a misleading source
diagnostic.
