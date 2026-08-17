# Nocter Development Handoff

## Current Task

Continue v0.14.0 Phase 4 from the completed deterministic `ExecutableProgram` boundary and lower
its already resolved items, roots, types, dispatch, and cleanup plans into validated MIR.
The previous compiler is preserved by commit `f6c08da3` and removed from the active working tree.
No previous source, test, binary behavior, or implementation document may be used as an
implementation input.

## Immediate Work

1. Extend the established checked-HIR-to-MIR lowering from scalar expressions, ordinary places,
   value-producing branches, direct/primitive calls, receiver and operand coercions, borrow
   conversions, comparisons, selected/coerced index places, outcome CFG, and unconditional
   cleanup destruction and conditional drop flags to loops, patterns, closures, and regions. Never
   repeat requirement, conformance, or drop-pattern selection.
2. Materialize process and ordered test runner control flow from `ExecutableRoot` metadata without
   synthetic source declarations or backend-name lookup, then validate the complete MIR graph.

The Phase 4 responsibility map is recorded in
`development/docs/target-program-design.md`. A closed `CompilationTarget` is now explicit in
compile-unit input and retained by `DeclarationGraph` through `CheckedProgram`. One shared
target-selection inventory excludes inactive items before block-import validation, symbol-table
construction, and declaration reservation. Unknown target gate names project `E0233`; recognized
reserved names remain distinct from implemented target availability.
Discovery-selected package target directives now pair their exact syntax node with one resolved
module identity. Declaration lowering derives target kind, name, and order from that directive,
allocates canonical `PackageTargetId` values, and projects each identity to its exact name literal;
it never parses an authored module path.
`nocter-target-program` now owns implementation availability. Recognition-only
`CompilationTarget` can no longer grant backend capability. An immutable `ToolchainSnapshot`
selects one inseparable backend, ABI, executable-writer, standard-package, and complete primitive
registry; currently only `arm64-darwin` can produce one. The registry has 49 closed semantic roles,
requires a unique callable for every role, and validates exact standard-package authority, module,
name, visibility, generic and parameter shape, result, provenance contract, target gate, and
bodylessness. The target-specific `SyscallResult` representation is validated down to copy shape,
field order, field types, and visibility. Extra primitives are rejected. `TargetProgram::build`
consumes `CheckedProgram`, proves target and standard-package identity plus package-target
integrity, and is the first public selected-target success boundary. An integration fixture crosses
the complete parser-to-target-program pipeline and proves that even same-shaped primitive roles
cannot be swapped.
Single-file lowering now creates one ordinary semantic executable target from its discovery-owned
package mode, root module, and display name. Its `PackageTargetId` projects to the file root, so
file and package execution have no parallel entry algorithm. Executable selection uses only the
selected module's authored namespace and freezes the exact `main` callable, body, module, target,
and one of the six accepted process-result contracts. Prelude fallbacks, re-exports, imported
modules, non-functions, generic or parameterized entries, bodyless entries, and other result types
cannot become executable roots. Test selection freezes only direct `TestId` declarations in the
selected module and retains their canonical declaration order; it never scans imports or
dependencies.
Callable specialization now uses one canonical key containing callable identity plus the complete
owner-and-callable generic domain. Missing, extra, and symbolic arguments are rejected, and owner
target types are derived rather than duplicated as receiver state. A single checked-body traversal
enumerates every executable static selection, closure, explicit pattern drop, referenced type, and
cleanup type while excluding unreachable retained source. Every explicit pattern drop retains its
declaration plus canonical generic substitution, so generic drop bodies do not lose the concrete
subject type before executable specialization. `ConcreteDispatchResolver` forks the
checked type store and resolves direct, interface, and structural dispatch into invocation,
comparison-lane, or index-lane plans containing direct, primitive, or indirect-callable steps.
Composite plans never encode operand ownership through array position. MIR will not receive an
unresolved requirement or repeat conformance selection.
Closure types now pair their lexical `ClosureId` with the complete enclosing generic domain. A
generic closure is no longer misclassified as globally concrete; specialization substitutes those
arguments into one distinct environment type, and the shared copyability authority carries its
capture condition across that specialization.
Opaque callable results now select one reachable witness pattern during body checking. A single
table proves the advertised interface and associated bindings, and checked conversions retain the
hidden representation through outcome injection. Callers see only advertised methods through an
`OpaqueMethod` edge; concrete dispatch opens the witness after specializing the opaque type's own
generic argument vector.
Concrete destruction now uses that same specialization authority. Exact generic drop selections
precede recursive reverse-order struct fields and active enum payloads; arrays, outcomes, closure
environments, and opaque witnesses retain explicit representation plans. Closure environment
metadata stores the captured binding and stored type as one field, preventing a non-owning
readwrite capture from being treated as ownership of its referent. The deterministic executable
closure can therefore enqueue every reachable user drop body without re-running source type
matching.
`ExecutableProgram` now owns the deterministic reachable closure. Callable, closure, drop, and test
keys enter one key-ordered work set; dense `ExecutableItemId` values are assigned only after the
set closes. Each concrete body freezes direct item IDs, typed standard/structural primitives,
indirect callable contracts, nested closure and exact drop edges, source-to-concrete type mappings,
and representation-specific cleanup glue. Bodyless callables are accepted only through the closed
toolchain primitive registry. Process and test roots remain compiler metadata, while test cases
retain declaration order. Enum residual cleanup is not collapsed to its nominal type: it excludes
the already-run owner drop and every transferred payload.
Every executable item also freezes its complete concrete runtime signature independently of body
use. Unused parameters therefore remain ABI inputs; receivers precede ordinary parameters, closure
bodies receive one capability-correct environment input before their declared parameters, drops
retain their exact readwrite receiver, and tests retain an empty input domain. MIR never applies a
generic substitution to recover a function signature.

`nocter-mir` now owns the canonical backend-independent representation. Function-local locals,
drop flags, places, SSA values, operations, and blocks use separate dense identity domains and can
be created only through a consuming builder. Block parameters carry typed merge values; exact
terminators own every successor and edge argument. Storage switches inspect enum, optional, and
fallible places without moving them, while conditional cleanup uses explicit drop-flag branches.
The validator checks concrete type references, specialized nominal member projections, aggregate
layouts, operation typing, block closure and reachability, edge arity and types, SSA dominance,
switch subject shape, direct semantic item references, and terminal result behavior. A narrow
`MirValidationEnvironment` supplies only immutable type, declaration, and executable-item
authority, leaving package and source setup outside MIR. `MirProgramBuilder` requires exactly one
function for every executable item and validates direct-call and drop-body signatures across the
closed function arena. The checked-body lowering path now consumes frozen concrete item and
primitive signatures, materializes receiver borrow capability, performs selected receiver and
operand coercions, and lowers primitive or selected comparisons without reopening selection.
Selected and coerced index projections now lower by borrowing the current place prefix, executing
their frozen receiver lane, and continuing from the returned borrow as a new MIR place root.
Outcome injection, absence, failure, propagation, force, and recovery share one typed temporary,
discriminant-switch, and payload-projection path. Propagation preserves every outer outcome layer,
catch bindings receive their failure payload before the fallback block, and the propagation edge
runs its exact checked cleanup schedule. Unconditional cleanup now lowers owned paths and values,
assignment replacement, user drop calls, reverse structural destruction, active outcome/enum
payload switches, opaque witnesses, and lexical region release from frozen executable plans.
Borrowed receiver roots remain initialized for flow checking but are excluded from callee-owned
destruction. One canonical value-storage authority now prevents borrow preparation, outcome
inspection, and cleanup from duplicating ownership. Conditional path and value cleanup reserves
entry-visible drop flags, updates them on initialization, move, replacement, and destruction, and
branches without reconstructing source control history. MIR places are interned by exact typed
shape, so flags and ordinary operations share storage identity. Loop, pattern, closure, and region
construction remain the current task.

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
for the current vertical body slice: scalar literals, inferred and annotated locals,
parameter/local/named-field places, readonly borrows, binding/discard, return/body-result checking, recursive outcome
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
exact `DropId` and canonical declaration-generic substitution; copy-only bindings retain the
complete enum for ordinary value cleanup instead.
Whole-binding state now tracks parameter and local move
paths, emits exact `Move` nodes, rejects moves of copy values and borrow bindings, and reports
later uses through `E0376`-`E0378`. Statically named fields now resolve through one visibility-aware
selector that substitutes the nominal owner's generic arguments and projects the exact field
identity back to source. Move paths retain field identity, preserve disjoint siblings, invalidate
their parent, and join inherited field state without enumerating a struct eagerly. `DropTable` is
the sole nominal-family-to-drop authority; partial moves inspect nearest enclosing families and
project `E0381` with the owning drop declaration. The entry-relative branch join cannot leak
branch-local paths.

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
edge. Collection iteration now shares the exact iterator-acquisition authority used by sequence
spread without requiring exact-size evidence. Explicit readonly and readwrite modes select their
matching expansion; moved sources prioritize direct Iterator evidence over owned expansion; bare
sources admit direct Iterator evidence only. The checked loop owns one retained iterator temporary,
initializes the Item binding per iteration, preserves the iterator across `continue`, and cleans
the current item before the iterator on exhaustion or outward transfer. Provenance and loans map
the binding through the selected `next` contract, while liveness keeps borrowed sources active
through the body. Authored acquisition and Iterator failures project `E0404`-`E0405`. Authored
local and closure annotations now resolve through one body
type-use authority, validate normalized data or callable-result position, and pass their resolved
type into the ordinary expected-type conversion boundary. The checked local therefore retains the
declared destination type rather than an initializer-side approximation. Invalid body type uses
and invalid discard forms project `E0406`-`E0407`; normalized shape violations continue to use
`E0360`-`E0365`.

The construction surface now indexes named functions and both literal shapes once. Literal
selection uses exact construction and callable identities, and a checked literal retains one
`StaticSelection` with every construction-binder argument rather than losing generic
specialization behind a bare callable ID. Fixed sequence elements, empty contextual sequences,
decoded typed strings, and ordinary static `&str` expressions pass expected-type inference,
ownership, provenance, and loan analysis. The sequence delimiter or string opener is the exact
callable source projection. Declaration validation rejects a string literal parameter other than
readonly `&str` and rejects outcome-wrapped literal results before body checking.

Exact-size typed-sequence spread is now closed over the same semantic authorities. Standard
`Iterator`, `Iterator.Item`, `Iterator.next`, `ExactSizeIterator`, and its `remaining_len` method are
exact validated roles rather than source spellings. Readonly and owned expansion use
`InstanceOperationSelector`; consuming direct iterators have fixed priority and cannot fall back
when exact-size evidence is absent. One `IteratorAcquisition` node gives iterator storage an
identity distinct from its source, while `TypedIteration` freezes `next`, `Item`, and exact-size
dispatch. Fixed and spread elements share one source-order construction inference session.
Ownership transfers acquired iterators into the element pack and cleans partial acquisition on
propagation. Provenance and loans map yielded values through `next` and the shared spread
contribution-type projection, preserving retained borrows without extending loans for copied
storage-independent values. Authored acquisition, iterator, and element failures project
`E0401`-`E0403`.

Compilation input can now attach compiler-owned standard semantic roles to exact declaration-name
tokens. One program-wide `StandardSemanticTable` resolves those tokens through `SourceIndex`,
rejects project-owned declarations and duplicate roles independently of input order, and validates
the non-generic allocator/context/String families plus the exact `Format.format_into` semantic
shape. Body checking never searches for a standard spelling or path. Typed literal `using` now
accepts only a place of an established aborting allocator or allocation-context family, records the
place as an explicit HIR operand, and projects `E0399` for an authored wrong type. Ownership,
provenance, loan, and closure-capability consumers all evaluate that operand before literal
elements; current-region literals retain the existing implicit selection.

Executable `region` statements now consume that same allocator-place authority and construct one
typed `AllocationContext` binding plus an explicit checked parent operand and body edge. Region
handles cannot enter ordinary copy, move, owned-receiver, moved-capture, or explicit-drop paths.
Ownership treats a region as a lexical resource rather than ordinary storage: every reachable
fallthrough, `return`, `break`, `continue`, and postfix-propagation edge cleans body-owned values
before one explicit region-release action. Nested cleanup follows scope order, while a `never`
edge schedules no release. The parent allocator/context remains loan-live through the child body
and its loan ends at the release action, before any enclosing parent cleanup. Provenance uses the
same region binding and current-allocation identity to reject direct and indirect storage escape.

Ordinary interpolation now decodes text and expression parts once in source order, normalizes
multiline indentation across interpolation boundaries, and constructs the exact role-selected
owned `String`. Every non-diverging expression is a shared readonly operand plan and selects only
the exact role-selected `Format.format_into` method through a concrete conformance or lexical
generic requirement; a same-spelled project interface has no authority. The checked operation
retains the formatter dispatch, allocation selection, and partial-output type independently of its
possibly diverging result type. Ownership activates the partial `String` before operands, keeps
formatted temporaries alive through their call, and places partial-output destruction on a later
postfix-propagation edge. Provenance and loans consume the same source order without reconstructing
format lookup. Missing or ambiguous formatting evidence projects `E0400`.

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
lowering now has an end-to-end scalar/control/direct-call slice. Non-empty cleanup schedules and
the remaining checked operation families fail explicitly rather than being omitted.

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
`CheckedReceiver` freezes owned copy/move, place or temporary borrowing, existing-borrow
preservation/weakening, selected coercion dispatch, and post-coercion weakening. Concrete calls
freeze their implementation/default callable; generic calls retain the exact interface
requirement. A program-wide provenance fixed point now derives exact caller-visible origins and
compiler-owned current-allocation dependence after ownership has attached cleanup. It retains
field, enum-payload, outcome, and element projections independently, maps results through static
and structural calls, and records a dense node/body/callable authority in `CheckedProgram`.
Return validation rejects local, owned-parameter, temporary, region, unknown, and undeclared input
origins as `E0395`; conformance implementations are additionally bounded by the corresponding
interface method contract. A separate dense `LoanTable` derives source-level non-lexical
liveness over checked places and node temporaries. It retains explicit and implicit loan identity,
capability, canonical field-sensitive places, reborrow ancestry, and per-node live sets. Readonly
and exclusive conflicts, move/drop/assignment conflicts, dynamic-index conservatism, branch and
loop joins, receiver-derived results, lexical storage escape, temporary receiver escape, and
type-owned drop observation order project `E0396`-`E0398`. Closure expressions now have lexically
reserved `ClosureId` identities, concrete closure types, and one program-owned
signature/environment definition. Parameter and result inference may use a structural callable
contract without depending on source argument order. Unannotated results join tail values,
explicit returns, absence, failure propagation, and divergence at the closure boundary. Each
capture is an explicit initialized environment field whose stored type determines copyability;
reads, mutations, moves, and nested callable invocations independently determine invocation
capability. Ownership, provenance, liveness, and loans analyze every closure body as a separate
execution root while mapping parameter, capture-value, and environment-storage origins through
direct and generic calls.

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
again. Explicit destruction and scheduled type-owned destruction enter the same loan analysis;
redundant initialized child move paths are normalized instead of turning whole values with a drop
body into fictional partial states.

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
