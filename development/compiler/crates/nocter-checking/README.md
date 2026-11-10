# nocter-checking

## Responsibility

Consume one accepted declaration program and produce syntax-independent typed semantics, ownership
facts, dispatch decisions, and explicit source-justified recovery evidence.

## Contract

Checking receives immutable program facts, exact body syntax projections, diagnostic origins, and
one semantic construction authority. Program-wide authorities can be prepared from the stable
declaration-symbol prefix and reused independently of body text. Opening a current generation
adds body spellings and source access without rebuilding those authorities. A successful result
exposes `CheckedProgram` and immutable semantic authority. A rejected analysis result classifies
every reached name/body domain and may expose only the recovery capabilities justified by its
diagnostics. Source projection is extended beside, never inside, semantic output.

## Internal Responsibilities

- program-wide preparation and standard semantic roles
- source projection of the declaration-owned callable contract compatibility policy
- lexical name evidence and body scopes
- source-neutral lexical recipes with current body-local locator and spelling rebinding
- one current-generation body-source catalog that owns each block or initializer-expression root
  and its body-local locator projection
- body-local structural type-extension recipes independent of sibling allocation order
- source-neutral checked-body and source-evidence recipes with canonical current replay
- type checking, inference, operations, construction, and calls
- exhaustive projection of eligible ordinary checked bodies into compile-time callable and
  initializer plans
- interface implementation and instance-operation selection
- specialized interface-capability evidence and prerequisite validation
- shared nominal field/generic-storage traversal for invocation-origin and carried-loan queries,
  preserving each query's distinct unresolved-generic policy
- ownership, cleanup dependencies, execution facts, loans, provenance, regions, and destruction
- checked local provenance contracts and call-site proofs over the declaration/model-owned
  constraint graph
- checked iteration steps freeze owning versus receiver-lending item origin; loan and provenance
  analysis consume that fact without repeating protocol selection
- checking-owned safety dispositions for value-sensitive operations; execution analysis and later
  lowering consume the frozen disposition instead of re-proving it from checked value shapes
- one exact-coverage body-relation catalog shared by provenance, execution, and loans
- persistent type/copyability/closure transactions
- checked and recovery semantic queries

## Invariants

- One source-neutral `ProgramEnvironment` carries stable facts through the complete checking
  lifetime. Generation-local `SourceAccessTable` storage is paired only by prepared and checked
  current-generation owners.
- Eager prepare/check convenience entries exist only behind the `test-api` feature or the crate's
  own test build. Production orchestration consumes query contracts and cannot reopen the stage.
- `ReusableCheckingQuery` owns source-neutral preparation together with the declaration projection
  recipe that may reopen it. Its exact-current body context materializes bindings, spellings, and
  source projection internally; callers cannot supply or pair those components independently.
- Query-owned program preparation accepts only a closed set of authored rule variants. It retains
  declaration recovery and repair evidence as one exact-current rejection; opening a session
  branch clones that authority and cannot rerun preparation or publish an internal error as source.
- Declaration recovery is constructed only inside checking from one preparation transaction. Its
  graph, type authority, declaration values, source ownership, and source projection cannot be
  supplied independently through the public contract.
- Declaration proof requirements cannot carry runtime evidence. Body requirements always carry
  one evidence identity; no optional-evidence state exists.
- One independent capability-evidence table owns each specialized predicate and every authored
  root/origin derivation that establishes it. Requirement order cannot select one derivation as
  the semantic authority.
- Type and copyability authority cannot be paired across generations.
- A body transaction commits all semantic mutations together or is discarded/frozen as one branch.
- Constant and immutable-static initializers enter lexical resolution and typed expression
  checking through the same semantic-body contract as executable blocks. Their expression form,
  result type, declaration owner, and source root are fixed upstream and mutually validated; the
  checker does not rediscover initializer meaning from declaration syntax.
- Initializer plans retain the same already-selected direct-call targets as callable plans. Their
  call edges seed the shared specialization query, including generic callees that have no other
  non-generic root; plan availability therefore cannot depend on an unrelated callable body.
- Checked dispatch is selected once; Target and MIR receive no lookup inputs.
- A source-visible safety proof is selected only in checking. Each checked index retains whether a
  runtime bounds check is required, the access is proven in range, or the access is proven to trap.
  Execution analysis consumes that disposition, and later stages transport it without
  reinterpretation. A path proof cannot be validated from a context-free constant after control
  flow has been erased.
- Local safety facts are keyed only by stable semantic value identities. Primitive comparisons
  refine branch-local integer intervals; every branch construct starts from one entry state and
  joins only reachable continuations. Mutable, captured, static, and projected storage is excluded
  until checking has an explicit invalidation model for it.
- Symbolic view bounds refer to the exact receiver identity and compiler-selected standard
  length role. A fact about one slice or string cannot justify indexing another, and checking does
  not infer length semantics from a method name.
- Integer negation, arithmetic, division-family operations, and shifts retain one checking-owned
  trap disposition. Constants and admitted flow intervals can prove an operation safe or certain
  to trap; incomplete evidence requires the complete runtime check. Execution analysis consumes
  that disposition directly when validating `notrap`.
- Loop exits use the same flow join as branches. A while-condition false edge and each reachable
  `break` contribute explicit states; potentially zero-iteration loops retain their entry state.
- In the released checked-body representation, scalar literals retain intrinsic values and
  declaration references retain `ConstantId`; evaluated declaration values are not copied into
  those bodies. The private value-access representation has its separate initializer boundary below.
- `ProgramEnvironment` carries the immutable structural-constant table selected with its graph.
  Body type construction reads only that narrow capability. Finalization executes checked
  initializer plans into the dense declaration-value table owned by `CompileTimeProgram`;
  presentation consumes a shared read-only lookup contract rather than assuming both phases have
  the same storage product.
- Generic lookup, provenance, loans, concrete dispatch, and editor queries consume the same frozen
  capability-evidence identity; a later stage cannot reinterpret the predicate or collapse its
  source derivations to whichever requirement was visited first.
- One normalized lexical type substitution accompanies every body assumption set. Refined
  instance bodies apply that same substitution to their result, parameters, authored type uses,
  `Self`, and nested closure arguments; body checking cannot reinterpret a declaration-pattern
  binder as an unconstrained generic after operation indexing has made it concrete.
- One checking-owned constructor creates an unspecialized generic value from its declared domain.
  Body environments, construction inference, interface assumptions, and instance selection cannot
  independently assume that a parameter denotes a type.
- Provenance containment uses the model-owned all-branches graph proof for direct declarations,
  structural callables, closures, interface implementations, results, local initialization, and
  reassignment. A consumer cannot substitute a one-path reachability test or reconstruct edges
  from source names.
- Direct calls receive one explicit result context: a complete expected type, an outcome payload,
  or a propagation result. `?`, `catch`, and `otherwise` route through that same call-planning
  boundary, so result-only generic parameters are not inferred differently by elimination syntax.
- Member-call disambiguation resolves an addressable receiver exactly once. Callable-field lookup
  operates on a private place draft; method fallback consumes the original resolved receiver and
  cannot retain nodes, uses, or source projections from a failed speculative check.
- A checked query derives type and visibility from its own body generation.
- Reusable body-name evidence contains body-local locators and spellings, never `NodeId`,
  `SyntaxToken`, `SourceId`, source spans, or current symbol IDs.
- A reusable checked body contains no current source identity, span, syntax identity, or symbol ID.
  Node origins, references, and associated-type completion sites use one body-local locator recipe.
  Local and capture declarations are restored from the lexical recipe instead of being duplicated
  in the typed result.
- Opening one current body-query context constructs its exact body-source catalog once. Name
  resolution, typed checking, rejection recovery, and canonical materialization share the same
  body/source pair and locator projection; none may walk the body again to reconstruct that join.
- A body type extension distinguishes the immutable prepared-program prefix from dense body-local
  additions. Closure references are body-local as well. Canonical program finalization re-interns
  both domains, so one sibling cannot change another body's reusable type identities.
- Body type capture proves the exact program prefix from persistent storage identity in logarithmic
  time and classifies dense suffix identities by a `TypeCursor`. It never rebuilds a program-wide
  type map or revisits every prefix type for each body.
- Each body checker opens from the same prepared semantic prefix and an empty closure domain. It
  cannot observe inferred types, copyability memoization, or closure allocation from a preceding
  sibling. Only a successful body recipe is replayed into the canonical program authority, and one
  closed rebinder rewrites every checked type, closure, dispatch substitution, place, and witness.
- Successful body queries are replayed in canonical `BodyId` order before ownership, provenance,
  execution facts, and loans run over the complete program. Canonical replay closes semantic
  completion, body arenas, and exact-current source projection into immutable shared authorities.
  Session never invokes body checking again for a complete query-owned body set.
- Program-relation results contain only source-neutral provenance, execution, loan, and body-locator
  facts. The exact-current materialization and final `CheckedProgram` share their closed semantic
  authorities rather than cloning a second program graph across the query boundary.
- Program-wide relation analysis receives one canonical `BodyRelationCatalog`. Its constructor
  proves that checked bodies cover the declaration graph exactly once and pairs each body with its
  declaration owner. Provenance, execution, and loans cannot accept independently ordered input
  slices, inspect syntax, or rediscover body membership by scanning them.
- Provenance, execution, and loans report authored failures as source-neutral body/node locators.
  A separate exact-current `BodyRelationProjection` projects those locators through origin maps.
  Relation computation therefore cannot retain a `SourceDiagnostic`, syntax lifetime, or stale
  source coordinates across an editor revision.
- Execution facts consume the already-checked operation graph and ownership-owned cleanup
  schedules. Ownership freezes each cleanup's exact drop dependencies after generic substitution
  and residual-payload selection. A positive `MayAllocate` fact reaches one least fixed point
  across callables, closures, and drop bodies; the execution pass has no type-store input and cannot
  reconstruct dispatch or destruction.
- Every checked call execution variant carries the result produced in that execution scope.
  Provenance and loans cannot pair a temporal classification with a separately recovered result
  identity.
- A source-backed `blocking` contract seeds `MayBlock` even when its current body happens not to
  wait. Callers consume the authored contract, so whole-program visibility cannot silently weaken
  a callable type or make asynchronous safety depend on the current implementation body.
- Callable guarantees may be forgotten only through an explicit checked operation. An unqualified
  callable value cannot acquire `noalloc`, and a downstream phase cannot recover a guarantee after
  that operation erased it.
- Canonical replay and whole-program authorities are exposed only through one finalization
  contract. Recipes own their body IDs, exact-current checked/failure outputs open explicit owned
  branches, and no caller can pair a recipe with a separately supplied identity.
- `CheckedProgramOutput` has no public parts constructor. Its component-preserving transform may
  pass `CheckedProgram` to a semantic consumer while retaining `SourceIndex` beside it, and restores
  the exact pair if that consumer rejects the program.
- Authored typed-body rejection is an exact-current query value. Successful siblings replay into
  editor evidence while the rejecting body contributes its diagnostic and typed-interruption
  capability; session assembles `BodyAnalysisRecovery` without checking either body again.
- Actionable checker failures attach their exact repair capability and authored name evidence when
  the owning rule is emitted; downstream analysis never classifies them by diagnostic code.
- Authored lexical rejection retains its exact diagnostic and an optional source-neutral partial
  recipe. Canonical catalog materialization produces either complete names or one
  `NameAnalysisRecovery`; callers cannot provide a body ID separately from the recipe that owns it.
  The complete lexical catalog is the only input accepted by this materialization contract, so a
  consumer cannot reconstruct recovery by rerunning one rejected body independently.
- A queried lexical rejection can reproduce its exact `PreparationFailure` branch. Session never
  reconstructs a name-resolution error variant from a diagnostic and separate recovery value.
- `SourceIndex` cannot affect a semantic decision.
- Program finalization projects each eligible checked body exactly once into a callable recipe,
  including generic bodies that have no current call site. Unsupported authored `const` operations
  therefore reject at their declaration rather than depending on reachability. It then seeds every
  closed non-generic root, follows already-selected direct calls, and specializes each reachable
  `CompileTimeCallTarget` exactly once through one query. Specialization reads recipes, not checked
  bodies, and transforms only type and call-target edges. Its `CompileTimePlanTable` uses closed
  evaluator-domain type shapes rather than store-relative `TypeId` arguments. Consumers cannot
  request projection, pair a declaration with an unrelated body, or repeat lookup, typing,
  conversion, operator, or dispatch selection. Failures remain source-neutral until finalization
  projects them through canonical node origins.
- Declaration construction and callable specialization are ordered immutable strata rather than
  one re-entrant heterogeneous query. Finalization joins the exact shared declaration value table
  and the closed specialization table into one `CompileTimeProgram`. Downstream consumers cannot
  observe or assemble a partial pairing, while neither declaration lowering nor checking can call
  back into the other's internal state.
- `CompileTimeProgram` retains the declaration graph's selected compilation target. Opening its
  executor therefore cannot pair plans and values with a caller-selected target profile.
- Final declaration-value construction validates its dense constant/static arenas against the
  exact declaration graph and final type store. A domain or shape mismatch becomes an internal
  checking failure and cannot be projected as an authored initializer diagnostic.
- The same boundary compares every early structural constant with its final checked-plan result.
  Structural type construction and the published constant value therefore cannot drift even
  though the bootstrap boundary necessarily evaluates that restricted subset twice.
- Program preparation interns every effective parameter value type, including borrowed receiver
  shapes, before sealing its reusable authority. Projection and body checking cannot make type
  availability depend on whether a body happens to mention `self`.

The [checked-program boundary](../../../architecture/pipeline/checked-program.md) documents contracts shared
with adjacent stages.


## Bounded Future Value Binding Typing

`accepted::SourceUnit` pairs the resulting complete Unit with its reusable header query. Its source
entry delegates contract readiness, ordinary body preparation and required execution to the
[initializer coordinator](#required-private-initializers). Known but unready declarations request
their complete contracts; unknown names remain source failures. The complete name domain forbids
shadowing independently of readiness, and dead source retains its requirements. Record construction
and annotation leaves, including closure input data, use that same request. This stage does not yet
request values during source type normalization; raw reference/containment validation remains eager.

The private-function goal has one typed-body query over shared immutable declaration facts and
an owner-scoped body-type extension. Each query retains its origin header prefix; binding accepts
only exact later extensions of that construction. `types` canonically replays those extensions and projects all
data-bearing body slots before control construction. Every accepted body shares the Unit's final
program/type authority. Its separate header handle proves source reuse; it is never a type-lookup
fallback. The common data projection also serves result closure, without parallel slot walkers.
Its inputs/results are data types; input Read/Edit/Take and local FixedValue/MutableValue/ReadAlias/
EditAlias are separate facts. Transient operation access descriptors are not qualified data types.
Take inputs receive writable local value slots; fixed local owners can transfer but cannot update.
Read inputs never become take from body use. Unmarked value supply requires Copy evidence;
explicit postfix transfer requires an owner and retires it even for Copy data. Alias creation
never manufactures an owner. Direct call selection is frozen
before access checking and has no permission-driven alternative target.

Primitive unit comparisons are selected as fixed-result intrinsics during typing. Their one
operation-owned result is shared by Boolean facts, evaluator projection and MIR computation.
Both operand evaluations, access checks, interruption and cleanup remain in the ordinary graph;
no consumer derives unit equality from a zero-byte representation or repeats type selection.

`calls` resolves function references and freezes declaration or formal-input dispatch through
local values, aliases and computed callees. `inputs` freezes input operations and optional
formal-to-actual witness bindings. `supply` alone selects an unmarked Copy snapshot or pending result-supply obligation.
`selection` freezes explicit source transfer after validating owning authority. Ordinary value
boundaries and call preparation consume those choices without a later Copy fallback.
Forwarding records the source formal identity; recursive forwarding keeps a finite semantic edge
instead of expanding witness expressions.
Binding a callback checks its complete flat contract and retains the selected actual view/data;
it never coerces the actual environment into the receiving formal's type identity. A terminating
unknown-shaped input carries neither an environment nor a witness binding. `annotations` projects local structural
constraints through the declaration-owned matcher, retaining witness identity. It does not create
an erased structural datatype. The ordinary value boundary applies the selected invocation
adapter after selected value supply; initialization and replacement use that same transition. Aliases
cannot construct a changed invocation view. Different witnesses do not join or replace one another
merely because their signatures match.

A value call prepares its environment as input zero, before explicit inputs. Its invocation view
selects Read/Edit/Take independently of data ownership. Direct declaration calls need no runtime
environment preparation. Each call records direct versus value invocation explicitly, so no
consumer infers the environment's presence from argument count. Every frozen argument plan carries its input operation; later passes
never reopen the callable parameter list or infer an environment offset. Direct and value calls
share argument preparation, atomic commit, interruption and cleanup.

Typed bodies record known Copy/materialization, explicit source transfers, scoped delegation, call input
preparation, RHS-first replacement and structured control. Unresolved result supply stays pending
until its exact witness closes; it does not acquire a provisional Copy operation. `construction`
freezes record/tuple components in authored order and applies the ordinary value boundary.
Tuple components receive that boundary after inference too; a readable place cannot become an
owned component merely because its data type is known. Its `FieldValue` plans
contain resolved identities and no source names. Empty, nested and reordered structural values
have no constructor-specific access operation. Ordinary block/conditional results construct values;
no old read/update result or standalone-prefix IR path remains.

`selection` owns member-chain decomposition and one declaration-backed component resolver, also used
by construction. Existing local-rooted chains become body-owned selected places containing the
root local, logical field/tuple path and data TypeId. Computed-rooted chains instead become one value
selection with the producing expression, full field path and selected Copy/Move action. Transparent grouping
does not change either category. No intermediate container extraction, second field lookup path
or implicit temporary alias is introduced. The construction-only place index is discarded on
publication. Alias creation and mutation still require existing storage.
Tuple diagnostic labels use a bounded rendering traversal so shared type graphs do not expand
exponentially in an error message; this does not limit source tuple arity or depth.

`integers` owns contextual integer-operand preparation and a construction-only memoized syntax
classification of literal-only arithmetic. Selected data constraints can flow from either operand
without reevaluating an expression or converting a typed name/call. Execution and scalar snapshots
remain left to right. Input data hints remain separate from binding operations, so a read/edit
input does not accidentally acquire a copied or moved value. Unsigned negation freezes the same
checked subtraction operation as zero minus the operand. i32/usize range checks use the model's
numeric profile; the private source currently qualifies its 64-bit usize only. Array lengths and
early structural dependencies still need their own integration before array source admission.

`ValueType` keeps known result access separately from possible normal completion. Calls,
construction, operators and compatible branches retain known data on interruption. Type inference
and field validation read that known access; flow/resource consumers require a possible normal
result. An absent result shape cannot be treated as an arbitrary record. Private constructors
exclude an unknown-shaped normal value, and continuation restriction cannot invent a runtime
value. Conditional typing checks both known shapes but selects a normally producing branch's
identity when the other cannot produce a value. Possible completion is not a termination guarantee.

Data annotations do not select local binding operations. Body-local arenas have no source
coordinates, syntax IDs, names or symbols. Successful queries own exact authority lineage/body bytes and
locator recipes. Rebinding requires preserved header contracts and an exact own body; a result-closed
query also verifies the producer-body evidence supporting its result identities. It constructs
current projection without retyping. This product cannot convert to released CheckedProgram or native
products.

## Callable Result Closure

`results::typing` checks result constraints while retaining the actual environment identity.
Compatible joins/replacements involving symbolic results retain exact identity equalities; equal
signatures never erase distinct code identities. A result-family value supply records a pending
obligation, and public flow admission rejects the query until closure. Structural contracts alone
still grant neither Copy nor trivial cleanup to formal inputs or unresolved results.

The private `flow::Control` prepares only the selected graph. Over its reachable successors,
`results::producers` propagates code/view facts through actual reachable evaluations,
local bindings, strong replacement, conditional forwarding, prepared arguments and staged returns.
Actual prepared producer facts, rather than provisional data types, establish recursive anchors. These are static producer facts, not a second owner or
availability ledger. Moves and drops remain subject to initialization/access/resource validation.

`results::solve` first closes reachable result relations to one named or own-input witness per
function. `results::relations` owns one finite positive-equation solver for unions and selected
applications. Recursive substitution uses formal IDs and finite application nodes rather than
expanding witness names. Unanchored recursion and heterogeneous return relations reject.
The established summaries then instantiate every typed call application through its frozen
actual bindings, retaining known types even on interruption. A missing producer for an application
without a reachable commit closes as explicit Unproduced evidence, preserving the structural
contract under the existing application identity. A reachable application still requires a witness.
Prepared value flow establishes return anchors; declared argument types alone cannot do so.

After deferred identity equalities are proved, `results::projection` replaces result/application
identities with already interned named or caller-formal keys, or retains a proved-unproduced
application's contract. During that same traversal, `supply` freezes pending operations against
the resolved data and source access. Unproduced supplies close explicitly without choosing Copy
or move. `results::unproduced` rejects any reached evaluation, input preparation, forwarding,
binding, value consumption or call commit using this absent data. This proof runs before
initialization over the same graph. Schema cleanup projection rejects result/application keys;
the accepted Unit's `DataEvidence` distinguishes payloads from unproduced structural contracts.
Known generic formal transfers remain transfers; later concrete
payload Copy cannot reinterpret them. Only then does Control compute initialization and
access/resource validation, once each. Control IDs and the type authority do not change, and no
graph is rebuilt. A taking formal environment can therefore pass through a result without
acquiring Copy, while a concrete named result at a caller can independently prove Copy. General captured
producers and their dependency footprints remain separate work.

All closed queries share one result-evidence object with producer-body byte dependencies.
Standalone query rebinding checks those dependencies; complete Unit projection verifies them once
and lends an opaque proof to its body projections. A sibling factory edit therefore cannot reuse
a stale closed result, even when the caller's own body and header are unchanged. Reusable-query
rejection projection lives in `diagnostics`, shared by result closure and pending supply; it binds
the owning query to current source without influencing semantic selection.

## Shared Flow, Access and Resource Validation

One selected-operation graph supplies initialization, backward permission demand and alternative
access/resource states. It preserves scalar snapshots, short-circuit/conditional edges, loop
successors, scope exits, staged value results, evaluation frames and interrupted call prefixes.
Read/edit direct operands reserve without early activation. Direct marked sources reserve
transfer independently of Copy and receiving operation; unmarked Copy take snapshots at preparation.
The argument recipe retains the exact selected callable-adapter operation identities in
materialization order, which replay only at invocation commit. Resource flow does not infer an
adapter from a difference between source/result type IDs. A marked source to read/edit moves
into a caller-owned temporary then lends access;
produced read/edit values use that same owned evaluation extent. Direct inputs skip child place evaluation;
`CopyPlace` reads its selected place and constructs exactly one snapshot at preparation. Its
initialization and read-demand obligations occur there, without retaining a read access through
later arguments or commit. Pending supply uses that same graph shape and cannot enter
initialization. Commit checks the full bundle before any source is transferred; no sequential
activation makes conflicting arguments valid. Earlier nested moves
are not rolled back. A read/update loan of a produced or transferred value retains its evaluation temporary until
the owning frame ends. Return survival is proved by the resource ledger alone; no parallel staged
referent state, old result-referent alternatives or callable-result permission survives.

Initialization distinguishes initialized/uninitialized/maybe selections on reachable paths and
solves backedges before reporting uses. Its sparse abstract storage separates container presence
from selected-value completeness, retaining invalidation causes without expanding record trees.
Copy does not invalidate its source; moves invalidate only the selected region. Restoration requires
present ancestor containers, while complete-value use requires every selected component. Fixed
owners cannot be reinitialized. Diagnostics retain which of those requirements failed.
`booleans` derives one expression-local normal-result fact while typing literals, negation,
short-circuit operations and value-preserving boundaries. It neither reads storage contents nor
inspects a callee body. Typing, capture inference and control construction consume that same
stored fact; no consumer grows a second predicate evaluator. RHS demand also controls reachable
breaks and normal completion: a skipped operand cannot introduce a loop exit, and a required
interrupted operand cannot manufacture a value. Known results retain the complete selected
evaluation and its effects. These are proofs under the [Boolean contract](../../../spec/language/booleans.md),
not constant replacement of calls or permission to skip name/data checks in dead branches. A dead break cannot create a normal loop exit or a missing-result
obligation; a reachable break still can. Selected terminating branches have no continuing value.
Edit replacements keep their outside place valid. Cleanup conditions are not permission to read
missing data or skip required destruction. Unreachable source retains structural checking without
invented runtime uses. Source projection reports already selected identities/locations.

Access demand includes alias children and prepared uses. Referent regions pair an owning/input/
temporary root with its resolved field path. Prefix overlap preserves exclusivity for a parent
and its child while allowing disjoint sibling accesses. Alias delegation concatenates paths;
access checking and call reservations do not resolve field names again. An unused intermediary can release its
own demand while its children retain read/update protection; remaining read demand does not force
update exclusivity forever. Delegation never upgrades read or duplicates exclusivity. Scope/root
survival reads the resource ledger directly: there is no second alive-root authority. Alternative
worlds preserve mutually exclusive branch states and converge over finite identities, relations,
resource holders and lexical/evaluation frame shape, without a dynamic history or iteration cutoff.

The resource ledger alone tracks initialized owner responsibility in each concrete alternative.
Initialization's joined availability facts serve use diagnostics and fixed-point reporting; they
cannot substitute for the ledger's exact alternative cleanup and survival proof. Logical places combine one
holder with a resolved field path; overlap uses field identity and containment, never physical
offsets. `resources::payload` owns sparse unavailable selections and ordered cleanup frontiers;
`resources::storage` owns transfers, replacements and frame registration. Missing siblings stay
separate because their containing slot still permits restoring an individual field. No complete
payload-tree expansion or second alive-owner registry is introduced.

Only a complete selected value can transfer or be invoked. Replacement cleans the selected old
initialized remainder before installing the already-evaluated value. A missing parent cannot be
reconstructed by writing a deeper child. Cleanup keeps intact subtrees as single aggregate operations
and expands only around unavailable paths, visiting surviving regions in reverse declaration
order. Partial containers emit an explicit ownership-end operation after surviving-field cleanup,
including the all-fields-moved case, without destroying moved fields again or choosing a physical
deallocation policy. Integrity rejection commits no partial ownership, cleanup steps or frame registration. Local
mutability and access permission remain prerequisites proved by body checking, not permissions
granted by a resource place.

`resources::assembly` transfers complete component obligations into one aggregate atomically.
Components remain ordinary expression owners in the existing evaluation frame until completion;
interrupted construction therefore cleans them in reverse production order without an additional
partial-constructor state or whole-aggregate destruction. `Step::Assemble` retains resolved
field/source pairs in authored order. Once complete, cleanup consumes the declaration schema's
field order. Integrity rejection preserves all original component owners and emits no partial step.

Computed component moves use the same selected resource-place transfer as named fields. Copy
selection preserves the complete source temporary and constructs only the selected snapshot.
The source remainder stays in its original evaluation frame; lending the selected result retains
that result's owner through a read/edit call. Neither operation changes the source's frame or adds
an independent temporary-survival registry.

`resources::function_view` consumes a materialized named witness and installs the selected view
atomically after validating the declaration-owned adapter policy. It keeps the code identity and
uses the existing resource/frame ledger, with no function-specific owner registry.

Its ordered steps construct, assemble, adapt function views, move, receive, dispose, invoke and
replace outside inputs. Each transition retains one ordered sequence and its boundary around
the selected operation. Complete bundle validation precedes before-call transfers/adapters;
result receipt follows the call. Ordinary before/after phases come from ownership evaluation,
never a downstream classification by step name. Sparse owning-container/missing-path conditions
project the same ledger for lifecycle lowering; they do not include a physical drop-flag policy.
The report retains entry evidence only after successful initialization analysis. Transition
snapshots additionally retain the ownership state between the before/after phases, so consumers
can project bit updates from actual ledger states without replaying source initialization rules.
Payload projection maps every obligation through one exhaustive step transform without changing
selected operations or places. A dead access holder is not automatic early resource destruction. A staged
return survives evaluation/lexical cleanup, whose order is reverse actual construction/preparation. A normal return has no evaluation frames and exactly one
Return owner with the checking-closed result data type, including unit. Trapping/diverging paths do
not acquire unwind guarantees.

Payload evidence consumes the declaration authority's exact bool/i32/usize/void and opaque owned-error
snapshot contracts, plus its closed record/tuple compositions over those leaves and exact
capture-free named-function witnesses. Scalars/unit/named functions have trivial disposal; error owns graph cleanup; aggregates
retain their own schema cleanup rather than becoming scalars.
Empty external D/S for those aggregates follows the exact component contracts, not Copy, ownership,
or a missing outside R. Named-function evidence follows its exact declaration schema; a formal callback does not inherit
Copy, trivial cleanup or empty D/S. Owning formals retain a symbolic environment cleanup operation
tied to their witness. The producer domain also admits the bounded single owned-capture schema described below;
general retained footprints and concrete executable witness specialization remain separate obligations. Dependent aggregates,
user destructors, capabilities, general captured environments, nested callback contracts, generics,
outcomes, async and regions require their own later schemas/validation.

The source/body goal admits closed record/tuple data and local/input-rooted component selections through
this same typing, initialization, access and resource path. Source integration tests assert exact
selected field cleanup, Copy versus move, restoration, partial drop, disjoint/overlapping access
and sparse traversal of deeply shared schemas. Construction tests cover nested values, exact
component mapping, scalar snapshots before later mutation, branch/loop interruption and atomic
rejection. Computed-selection tests inspect exact value transfers, preserved Copy sources,
read-temporary and remainder cleanup, interrupted known types, precise field diagnostics and a
64-level shared computed field chain. Named-function tests inspect static dispatch, environment
input ordering, view adaptation, explicit taking-input indices, Copy-before-drop and interrupted
preparation without invocation or result receipt. Formal-input tests inspect exact binding edges,
distinct positions, recursive forwarding, non-Copy environment consumption, symbolic adapter cleanup
and early return. Indexing remains outside this fragment. Resource
transitions alone cannot replace the combined source proofs or establish executable/native behavior.

## Closed Accepted Private Units

The whole-unit constructor validates every definition, including unused/recursive callees, and
publishes no partial accepted value after any typing/init/access/resource failure. It owns the
same shared declaration facts, canonical type authority, closed result evidence and all body/flow
reports. It rejects an incomplete ready-header view before evaluation or acceptance. Admission
also proves exact callable-body and required-initializer-value coverage; a coordinator's
omitted body or missing, extra or role-substituted value cannot become a complete Unit. Source
binding validates every original body before exposing current projections. An equal but separately rebuilt program or changed sibling
cannot substitute for that authority. The sole body catalog exposes exact semantic-ID lookup for
the target owner's private instance consumer. That consumer receives only the complete Unit,
never typed-only evidence. The schema-to-payload projection exposes a cleanup obligation without
constructing an initialized owner. There is no public parts constructor, compatibility adapter,
source-version CLI or general D/S solver. The
[private handoff](../../../architecture/pipeline/value-access-declarations.md) owns downstream
qualification; remaining work belongs in the [realization plan](../../../design/compiler-realization.md),
not a duplicate crate-internal status table.

## Constant-Domain Acceptance

Private `const func` declarations retain the same body, typed operations, selected callees and
resource ledger as ordinary functions. The authored header capability alone is not body evidence;
typed-only queries remain incomplete. Whole-unit acceptance additionally checks every const
body's operation domain, including unused definitions, dead branches and interrupted call inputs.
Direct calls require the selected declaration's authored const capability. Recursive const
headers close without executing bodies or proving termination; every definition must independently
pass ordinary typing, initialization, access and resource validation.

`value_access::constant` owns this bounded domain check. It admits primitive Boolean/i32/usize/unit
operations, finite record/tuple compositions of supported data, local values, read/edit aliases,
updates, structured control, explicit transfer and direct const calls. Aggregate admission is independent of Copy and
memoized by data identity rather than expanded through shared fields. Function-value formation,
captures, invocation through values and opaque error-data operations have no evaluator domain here.
Explicit unsupported drops fail even in dead source. Implicit disposal and edit-input replacement
consume the actual sparse resource transitions; ending a partial container does not destroy its
missing payload again. An unused read input has no owned cleanup, and an interrupted known result
has no produced value to classify. Such vacuous cases do not grant an evaluator representation to
their opaque annotations.

Failures retain source-neutral rules and expression/statement/cleanup sites; exact-current binding
projects E0421 without repeating lookup, operation selection or ownership analysis. Ordinary
runtime calls continue to consume the same accepted body. Checked arithmetic, including operations
that would trap if evaluated, is a supported operation domain. This domain check executes nothing and proves no termination or trap freedom.
[Required initializers](#required-private-initializers) additionally execute admitted plans. Generic
conditional eligibility and exported const products remain
[evaluator integration work](../../../design/compiler-realization.md#conditional-const-admission).

## Required Private Initializers

The private source goal admits `const NAME: Type = expression` and
`static NAME: Type = expression` with required explicit data types and initializers. A constant is
storage-independent data; a static denotes one immutable place. Neither is a function witness. All
initializers are ordinarily typed and checked for the const domain, including unused declarations.
The same body catalog holds their input-free initialization thunks; no source function, synthetic
source text or separate expression checker is created. Function lookup and witness publication
exclude those thunks. Their execution role grants no source return target: an initializer cannot
use `return` to leave its value-expression boundary. Nested source callables retain their own
return targets. Runtime instance closure rejects an initializer as an entry root.

`evaluation::preparation` owns the common coordinator for contract readiness, typed bodies and one
`DependencyQuery` for header, typed-body and required-value requests. `dependencies` owns those
disjoint key/fact roles; there is no parallel typed-body cache or active dependency stack. Header
facts acknowledge the declaration owner's ready contracts without containing another program or
type store. The coordinator accepts either a closed context or the exact-current
declaration preparation. A missing callable/record contract advances the ready header view; a
missing constant requests its value only after selecting its declaration identity. An unpublished
draft stops at either request and retries through the same checker after its dependency is ready.
Successful typed bodies become completed facts in that same query. No provisional value, syntax
dependency scanner, second source interpreter or completed-body retyping supplies a fact.
Constant references become validated primitive literals or immutable frozen-aggregate expressions in
ordinary expression checking,
before conditional completion, reachable breaks, capture invocation inference or control freezing.
All authored source remains structurally checked, including unevaluated branches. Selected const
callees are prepared through all authored call edges; callable-only recursion remains a graph,
while a constant reference requests a value and exposes an active declaration cycle.

Required values can execute with unrelated callable/record contracts still unready. Every selected
const body retains its origin header view until canonical replay and contributes its exact body
bytes to source dependencies. A body becomes complete only after its static-value requirements;
re-entry into an unfinished body/value path reports the cycle at the responsible value reference.
Already completed dependencies survive a failed request, while its active branches unwind without
publishing unfinished facts. Callable-only recursion still closes through ready contracts and the
ordinary selected call graph. The same pass also checks ordinary and
unused definitions. Whole-unit closure requires every header, body and required value; selected
const-closure acceptance never grants complete Unit admission. Source type normalization cannot
yet request these values; header-dependent lengths remain a separate integration gate.

The same `accepted::check_queries` routine canonically replays and ordinarily validates a selected
const closure before any execution, and later closes the complete Unit. Each temporary closure
has one shared type authority. Only validated frozen data crosses from required execution into
further body preparation; temporary store-relative types or access evidence do not
become the Unit's final authority. Final canonical replay still pairs every accepted body with
that one authority. Definition-time operation checks remain independent of selected execution;
cleanup-domain checks additionally consume actual ordinary resource obligations. All required
values and every definition must succeed before Unit publication, including unused definitions.

`evaluation::projection` closes the required root's reachable calls from those checked products
into stateful evaluator plans. Each alternative preserves its sparse owning guard and exact
before/computation/after phases. Resolved data/fields, scalar snapshots, aliases, input supply,
protected returns and cleanup come from ordinary checking. The projector cannot repeat lookup,
infer a move or select another target. The executor receives literals, not unresolved declaration
references, and cannot request body checking. One work budget spans dependency preparation and
nested executions; pending value requests and execution frames share the depth limit. Header/body
keys participate in cycle detection but do not add source execution-call depth. This role count
reads the query's exact active path, without another stack or depth counter. A shared live-storage
meter also covers nested execution and retained completed values;
outer requests restart work without resetting storage. Its accounting contract belongs to the
[evaluator](../nocter-constant-evaluation/README.md#live-working-storage). Cycles, arithmetic failure,
work/depth/storage exhaustion and unsupported final publication retain
distinct local diagnostics. `prepare_with_limits` supplies explicit deterministic limits.

Final publication admits bool, i32, usize, unit and finite closed record/tuple compositions of those
leaves. Publication uses an explicit final-data policy over the shared schema traversal, independently of temporary
execution support and Copy. The admitted aggregate schema has no user destructors or retained
resources. Each constant reference creates a fresh ordinary value without granting its type Copy.
Constant names remain non-addressable. Constant member selection resolves fields through the
ordinary field authority, then selects established frozen components; scalar selections become
ordinary literals before control facts and flow admission. No runtime initializer call results.
Fixed arrays, enums, generic conditions and early structural type dependencies still require
integration. General outcome/callback/resource evaluation is not
admitted by this path. Draft preparation can repeat work before a missing dependency resolves;
resumable checking and reusable closed const plans remain compilation-cost improvements, not
permission to cache mutable call results or change source validity.

A static reference selects one cached body-local address view with `LocalOrigin::Static`; its data
type stays ordinary and its access is read-only. That view is initialized on entry but is not a
callable input or a source owner. `TypedBody::static_views` is the shared projection for consumers;
input-layout validation separately proves the declaration role, exact type, read-only kind and
unique global key. Typing a static view requires only its declaration metadata, not its computed
initializer. Preparation derives required static values from those views, including capture bodies,
and resolves them through the same dependency query before a typed body becomes a completed fact.
A pending body cannot hide an active static cycle behind cached typing; reusing a completed body
already has completed prerequisites in that same authority. Access worlds use the global static root, whose extent is independent
of any invocation frame. The resource ledger does not create static cleanup obligations. Reads/aliases
are allowed, independent value extraction needs ordinary Copy, and editing/retirement/drop reject.
Static reads remain place operations rather than named-literal substitution. Their initializer
values and mixed constant/static dependencies use the same required-value query, source guards
and final-data policy. Published Units retain only frozen initializer data, with role-checked
constant/static accessors.

For example, named constant facts reach ordinary access and initialization checks:

```nocter
const FLAG: bool = false
func run(): i32 {
    let value = 42
    if FLAG { drop value }
    value
}
```

This is accepted like the literal condition. Setting FLAG to true rejects the later use. Known
constant facts also reach loop completion and closure invocation inference, while dead source
retains name, type and const-operation obligations. Required evaluation is separate from optional
runtime optimization; runtime calls to const functions still execute their ordinary bodies.

Each completed dependency retains its evaluator value lease alongside one derived model-owned
frozen artifact until all required evaluation and body preparation finish. Query publication
consumes those entries into the Unit's sole frozen initializer table and drops evaluator state.
Bodies share the immutable artifact graph; no public product retains a temporary type authority.
The completed query is consumed rather than cloned. Bodies and captures
share an exact-source guard over const initializer/callee definitions. Header reuse cannot reuse
values or resulting control/access evidence after a supplying body changes, including through a
standalone typed/flow source binding. `source_dependencies` owns that verification mechanism for
both evaluator products and callable-result evidence. Whole-unit binding still verifies every
body. Ordinary editing calls have no result-only cache that could erase writes.

## Owned Capture Bodies

`closures` checks a first explicit single-value environment under the private source contract.
Formation uses the ordinary value-supply boundary. A separate body scope receives that environment
and flat explicit inputs; names outside it cannot become implicit captures. The body returns
ordinary closed data and cannot introduce nested closures or callable-result applications.
The enclosing header remains its source-reuse owner, while lexical owner/slot identifies its code.

The environment's provisional construction authority permits demand collection, not accepted
execution. Its control graph is prepared once. Reached environment moves/drops or transfer inputs
select once; updates/delegations select mut; otherwise invocation is func. Unreachable moves do
not add demand. The selected environment input kind freezes before initialization/access/resource
validation, which consumes that same graph. Explicit parameter operations never come from body
use. Parent and closure bodies share canonical type replay and the same flow-admission procedure.
One immutable type authority covers their data and closure schemas; no second body checker or
ownership solver is introduced.

Input referents are keyed by body-local input slots, not named-header parameter identities. Capture
and anonymous parameter slots use that same initialization, root, liveness and edit-replacement
path. Header parameter identities still select formal callable witnesses and source contracts;
diagnostics project an input slot directly through its local source locator.

The existing resource ledger's atomic Capture transition transfers a complete environment into
one closure value. Unmarked capture supply snapshots with Copy evidence; a marked capture transfers one owning
source, including Copy data.
OwnedCapture disposal retains the schema's environment cleanup obligation. A consuming body owns
and cleans its remainder; a read/edit body receives scoped input authority. Adapters preserve the
code/environment identity. Result equations accept only actually produced closure witnesses.
Unit admission validates every nested body, even an uninvoked or unreachable producer, before
publishing its closure-body catalog. Capture/body edits invalidate the enclosing source recipe.

## Frozen Body Input Channels

A typed query's code identity selects its named or captured body. Its derived source owner selects
only the enclosing source reuse guard; these are separate roles, not interchangeable lookup keys.
The accepted Unit exposes exact code-to-body selection without a name or header/body fallback.

`body_inputs` owns coverage and contract validation of the typed body's input layout: one optional
environment channel and an authored-order sequence of explicit input slots and operations. Named
parameters retain their exact declaration identities; captured and anonymous inputs retain their
body-local roles. Every input local occurs exactly once, with a matching slot kind and contract.
Control construction consumes this order for root scope cleanup. Capture demand freezes the
environment operation with its local authority before final contract validation and flow admission.
No downstream consumer infers input position or an environment channel from local allocation order,
body use, source names or argument count. Mutation tests reject reordered/duplicate/missing slots,
changed operations, substituted environments and an enclosing source owner used as closure code.
