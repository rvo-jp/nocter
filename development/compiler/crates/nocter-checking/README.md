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
and annotation leaves, including closure input data, use that same request. Header type
normalization can request required usize values through that same query; raw type recipes load
only as their declarations are requested.

The private-function goal has one typed-body query over shared immutable declaration facts and
an owner-scoped body-type extension. Each query retains its origin header prefix; binding accepts
only exact later extensions of that construction. `types` canonically replays those extensions and projects all
data-bearing body slots before control construction. Every accepted body shares the Unit's final
program/type authority. Its declaration-lowering-owned `BodySource` pairs the original header view with its source owner
and exact bytes for reuse; that header proof is never a type-lookup fallback. The common data projection also serves result closure, without parallel slot walkers.
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
checked subtraction operation as zero minus the operand. All fixed-width signed/unsigned integer
range checks use the model's numeric profile; isize and usize retain distinct identities under the
qualified 64-bit target profile. Header and
[body-local array lengths](#body-local-required-expressions) use ordinary usize checking and
required execution. [Listed array values](#array-construction) share the ordinary aggregate path.
[Selection and complete replacement](#array-selection) use evaluated element addresses;
element transfer and partial-state operations retain explicit implementation gates.

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
Input schemas seed received callable witnesses, including owned callable environments. Construction
transports actual component facts into each structural value; member/index selection filters those
facts rather than manufacturing a producer from the selected schema. `producers::facts` indexes
actual alternative terms by callable-leaf data identity. Equal leaf data denotes one static witness
after result equalities close, so homogeneous arrays and shared tuple shapes need no per-element or
expanded-path metadata. Structural forwarding aligns those leaves positionally without erasing
their actual terms. Whole replacement is strong; component replacement retains conservative terms
without guessing evaluated indices or maintaining another alias registry. Deferred transfers and
prepared arguments retain the already evaluated facts; view adapters preserve their code identity.
No provisional result/application signature supplies a return anchor. These are static code facts,
not an owner, availability or numerical-disjointness proof. Moves and drops remain subject to the
separate initialization/access/resource validation.

`results::solve` first closes reachable result relations to one named or own-input witness per
function. `results::relations` owns one finite positive-equation solver for unions and selected
applications. Recursive substitution uses formal IDs and finite application nodes rather than
expanding witness names. Unanchored recursion and heterogeneous return relations reject.
The established summaries then instantiate every typed call application through its frozen
actual bindings, retaining known types even on interruption. A missing producer for an application
without a reachable commit closes as explicit Unproduced evidence, preserving the structural
contract under the existing application identity. A reachable application still requires a witness.
Prepared value flow establishes return anchors; declared argument types alone cannot do so.

After deferred identity equalities are proved, the declaration owner's
[`resolve_body_types`](../nocter-declarations/README.md#future-value-declaration-boundary)
normalizes result/application leaves and their containing structural shapes in the same type
authority, including the declaration owner's captured-environment containment check. A
result-induced positive environment cycle projects to its defining capture expression as
InvalidConstruction rather than becoming an integrity failure. `results::projection` rewrites every data-bearing body slot against that one projection,
or retains a proved-unproduced application's contract. During that same traversal, `supply` freezes pending operations against
the resolved data and source access. Unproduced supplies close explicitly without choosing Copy
or move. `results::unproduced` rejects any reached evaluation, input preparation, forwarding,
binding, value consumption or call commit using this absent data. This proof runs before
initialization over the same graph. Schema cleanup projection rejects result/application keys;
the accepted Unit's `DataEvidence` distinguishes payloads from unproduced structural shapes.
Positive tuple/array payload edges propagate an unproduced component; a zero array has no such
element edge. The marker adds no duplicate signature: the existing Program remains the shape and
contract authority. Interrupted structural construction retains typing but supplies no aggregate
owner, Copy operation, storage or instance edge; already completed component owners still clean
normally. Reached use of any such shape rejects before flow admission.
Known generic formal transfers remain transfers; later concrete
payload Copy cannot reinterpret them. Only then does Control compute initialization and
access/resource validation, once each. Control IDs and the type authority do not change, and no
graph is rebuilt. A taking formal environment can therefore pass through a result without
acquiring Copy, while a concrete named result at a caller can independently prove Copy.

Inferred arrays/tuples can retain valid pending component descriptors. Compatible structural
joins/replacements defer only differing callable-leaf identities; arity, array length, invocation,
input operations and result data must already agree. Once those identities close, construction
requires actual component payloads, repetition above one requires Copy, and unmarked aggregate
supply requires Copy of the complete resolved shape. Computed tuple selection chooses Copy/move
from that closed payload; computed array selection retains its Copy requirement. Explicit postfix
retirement stays explicit even for a finally Copy shape. Pending evaluated referents use the same
type projection as locals rather than consulting stale expression data. No placeholder environment,
new ownership ledger or cardinality expansion is introduced.

The shared `structural-result-values.nct` source qualifies factory-produced named/captured values,
distinct environments of one code, arrays, nested tuples/arrays, repetition, replacement,
conditional joins, Copy snapshots, non-Copy retirement and factory returns of selected components
through required and native execution, including maximum-count zero-byte arrays. Runtime source
also qualifies formal forwarding into arrays and interrupted nested structural values, with no
unbound storage/instance edges and cleanup only for completed components. Different witnesses,
non-Copy repetition, unmarked duplication and unanchored structural recursion reject at source;
producer facts cannot hide retirement. Factory body changes invalidate closed callers even when
their own source is unchanged. General formal callable storage and dependent capture footprints
remain separate work; a constructor whose component payload cannot close reports a local rejection
before executable admission. Owned captures follow the [result-resolved environment boundary](#result-resolved-owned-environments).

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
Initialization publishes availability and diagnostics; the resource transition report alone owns
cleanup conditions and ordered steps.
`Expression.normal_value` owns one typed normal-result scalar fact. `booleans` derives its
Boolean alternative while typing literals, negation, short-circuit operations and value-preserving
boundaries. It neither reads storage contents nor
inspects a callee body. Typing, capture inference and control construction consume that same
stored fact; no consumer grows a second predicate evaluator. RHS demand also controls reachable
breaks and normal completion: a skipped operand cannot introduce a loop exit, and a required
interrupted operand cannot manufacture a value. Known results retain the complete selected
evaluation and its effects. These are proofs under the [Boolean contract](../../../spec/language/booleans.md),
not constant replacement of calls or permission to skip name/data checks in dead branches. A dead break cannot create a normal loop exit or a missing-result
obligation; a reachable break still can. Selected terminating branches have no continuing value.
Same-operator builtin logical chains share one lazy control region. Iterative traversal passes
through their owned-bool supply boundaries, retaining leaf evaluation, operand order and the
original evaluation frame. It does not materialize intermediate bool results and then invent
paths that continue after a decided short-circuit result. Read-input call chains are qualified
with a linear prefix bound on ownership alternatives, alongside execution checks for every
short-circuit position and the final operand's value. No source length cutoff is introduced.
`integers` derives the integer alternative from established literals, value-preserving boundaries
and the common result of every reachable normal conditional branch. Named constant data arrives
through the ordinary required-value authority. Unknown storage/call results remain unknown; no
arithmetic evaluator, lexical-value read or callee inspection is added. A known numeric result
cannot turn a typed name into a contextual literal or suppress index evaluation and its effects.

Edit replacements keep their outside place valid. Cleanup conditions are not permission to read
missing data or skip required destruction. Unreachable source retains structural checking without
invented runtime uses. Source projection reports already selected identities/locations.

`RetentionKind::Place` keeps a selected place's identity without activating a read or update grant.
Read/edit alias boundaries activate their declared access; call inputs retain reservations until
bundle commit. Projection liveness propagates update demand to the existing base authority, and
world trimming compresses dead neutral intermediates while retaining the first actual authority
grant. Address-only identification does not read the whole containing payload; actual value supply,
alias activation and call commit retain their read/update checks. A real read grant stops update
delegation. No consumer upgrades a read alias, infers writable authority from Copy, or maintains a
second place/permission registry. Retained-place invalidation diagnostics name
the selected place separately from an active access or pending call.

Access demand includes alias children and prepared uses. Referent regions pair an owning/input/
temporary root with its resolved field path. Prefix overlap preserves exclusivity for a parent and
its child while allowing disjoint sibling accesses. Static alias delegation concatenates paths;
evaluated-array paths follow the [selection boundary](#array-selection). Access
checking and call reservations do not resolve field names again. An unused intermediary can release
its own demand while its children retain read/update protection; remaining read demand does not
force update exclusivity forever. Delegation never upgrades read or duplicates exclusivity.
Scope/root survival reads the resource ledger directly: there is no second alive-root authority.
Alternative worlds preserve mutually exclusive branch states and converge over finite identities,
relations, resource holders and lexical/evaluation frame shape, without a dynamic history or
iteration cutoff.

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

Complete assignment through an initialized edit alias uses `State::replace_referent` and one
`Step::ReplaceReferent` containing the alias-local identity, relative fields and replacement owner.
Data mismatch rejects transactionally before publishing any owner/frame/step change. The alias
retains no ownership, and the enclosing owner's initialized state stays unchanged. Evaluator and
MIR consumers dispose/replace the exact referent through its saved address. They never reinterpret
a conservative conflict footprint as a selected resource path or repeat index evaluation.

`resources::assembly` transfers complete listed component obligations into one structural value atomically.
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

Fallible schemas participate in the logical ledger with one selected payload, including when
success data is itself `error`. A guarded partial acquisition records the active variant in its
missing descendant path. Remainder cleanup follows that variant and never invents an opposite
payload. Opposite variant paths continue to overlap physically; no global disjointness rule is
used to repair cleanup. These logical obligations are separate from constructing or executing
an owned error graph, which the evaluator does not yet admit.

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

`admission` validates known body result/input/local/expression data before a typed query freezes,
including callable constraints and capture environments. Array-bearing operations follow the
[construction](#array-construction), [selection](#array-selection) and
[resource](#concrete-array-resource-frontiers) owners below. A declaration schema alone does not
qualify an executable operation; zero count does not substitute tuple or scalar behavior.

Source-admitted payload evidence consumes the declaration authority's exact Boolean/integer/unit and opaque owned-error
snapshot contracts, plus qualified optional/array/record/tuple compositions and exact
capture-free named-function witnesses. Scalars/unit/named functions have trivial disposal; error owns graph cleanup; aggregates
retain their own schema cleanup rather than becoming scalars.
Empty external D/S for those aggregates follows the exact component contracts, not Copy, ownership,
or a missing outside R. Named-function evidence follows its exact declaration schema; a formal callback does not inherit
Copy, trivial cleanup or empty D/S. Owning formals retain a symbolic environment cleanup operation
tied to their witness. The producer domain also admits the bounded owned-environment schema described below;
general retained footprints and concrete executable witness specialization remain separate obligations. Dependent aggregates,
user destructors, capabilities, general captured environments, nested callback contracts, generics,
fallible construction/propagation/recovery, async and regions require their own later schemas/validation.

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

## Optional Values

`outcomes` owns contextual `none` and presence construction, preserving exact data forwarding
before adding optional layers. Payload readiness and Copy use the declaration authority's
positive optional edge; the current tag does not grant Copy. Whole-value postfix `!` supplies
the carrier first, checks presence, then selects its payload through ordinary Copy/transfer and
remaining-container cleanup. Empty completion and bare return supply the ordinary unit value
through that same result boundary, including present optional layers. A conditional without else
first supplies unit, then applies its enclosing value context; it cannot return branch absence.
Value postfix `?` expands into ordinary binding, a stored-tag condition and a conditional whose
absence branch returns the declared outer optional absence. The supplied carrier has one local
owner and the selected answer is protected before its lexical cleanup. Existing return flow
cancels enclosing prepared calls and retires scopes; no separate propagation cleanup protocol is
introduced. A closure uses its own result contract. Initializers and non-optional callable results
cannot propagate absence. Direct read/edit operands and scoped bindings request an existing payload
place for postfix `!` or `?`. `Scoped` forms its source once and forwards the place selected by
its projection/control. Assertion and propagation retain the carrier, with the payload selector
outside that receipt. `GuardView` checks the tag
and retains its stability. Ordinary selection, argument reservation and alias binding then use the
original payload storage. Scoped propagation returns absence through the same callable return
protocol without transferring the carrier. Its tag condition consumes a resolved place, including
an already saved indexed address, and reads no payload data. The access
world retains tag evidence with dependent places, including read reservations whose neutral
intermediates can be released. A tag pin grants no update authority and prevents carrier replacement
or retirement while allowing otherwise authorized disjoint payload accesses. It is compilation
state, with no runtime owner or lock table. Named aliases cannot keep a produced temporary alive.
`otherwise` uses the same carrier acquisition and ordinary conditional. Value recovery supplies
the carrier once, selects its present payload or evaluates the fallback only on absence, and
protects the answer before lexical cleanup. Nested payload data is preserved exactly. Scoped
recovery admits terminating paths and normal fallbacks designating compatible surviving places.
Its place-demand boundary validates data and update authority without activating a grant during
call preparation. Ordinary control forwards the selected alias before handler-local cleanup;
an outside referent can survive a handler-local alias, while a handler-owned value cannot escape.
One typed projection traversal enumerates normal provenance alternatives for authority and
conservative producer analysis. Initialization validates each selected source before forwarding;
access analysis retains the actual selected root through cleanup and use, without re-reading
unchosen source slots. A unique root/path query never guesses among alternatives, including
an existing place versus owned temporary backing.

A direct read/edit input demand can also lend a fresh owned fallback, applying ordinary value
construction before lending when contextual wrappers are needed. An existing place must already
have compatible data and authority. `TemporaryPlace` aliases
the actual owning expression's payload, keeping its existing ledger entry and owned slot separate
from the alias slot. Scope-exiting `Forward` protects that owner in the parent evaluation frame;
ordinary alias forwarding does not extend its lifetime. Earlier-frame owners retain their
original registration and cleanup order. The selected branch uses ordinary neutral reservations
and complete-bundle validation. Normal completion, cancellation and frame exit check retained
access before destroying its backing. Handler-local destruction still precedes delivery; an
explicitly acquired local result can become the bounded temporary. Strict named-alias demands
cannot create such backing or upgrade a read-only existing branch. These facts add no runtime
owner table or physical payload move. Fallible recovery remains outside this qualified path.

Implicit optional input wrappers retain the original direct-transfer reservation. Complete bundle
validation precedes wrapper materialization and original-source retirement; later reads and
cancellation therefore observe the original owner. Explicit nested value constructions keep their
ordinary preparation timing. No wrapper changes the input operation or repairs a direct edit-place
mismatch. Shared required/native source covers nested absence/presence, void and narrow payloads,
local/static/container storage, captured calls, read/edit/take handoffs and indexed transfers.

## Constant-Domain Acceptance

Private `const func` declarations retain the same body, typed operations, selected callees and
resource ledger as ordinary functions. The authored header capability alone is not body evidence;
typed-only queries remain incomplete. Whole-unit acceptance additionally checks every const
body's operation domain, including unused definitions, dead branches and interrupted call inputs.
Direct and exact named-value calls require the selected declaration's authored const capability. Recursive const
headers close without executing bodies or proving termination; every definition must independently
pass ordinary typing, initialization, access and resource validation.

`value_access::constant` owns this bounded domain check. It admits primitive Boolean/integer/unit
operations, optional construction/assertion and finite record/tuple/array payload compositions of supported data, local values, read/edit aliases,
updates, structured control, explicit transfer, exact named values and eligible owned captured calls. Aggregate admission is independent of Copy and
memoized by data identity rather than expanded through shared fields. `evaluation::types` is the
single shape projection used by const-domain validation, final-data admission and closed-plan
construction; homogeneous arrays retain an element ID and u64 count, never expanded type lists.
Zero-length element edges are linked after the positive-containment walk. They retain source
identity without requiring an element payload, including recursive nominal graphs and otherwise
unsupported element data. An element declaration can still be unready during required evaluation;
that identity-only edge does not request its payload header or manufacture a ready schema. Final
Unit closure still validates every independent declaration. `Type::Unavailable` retains such an
identity without accepting a value; positive occurrences of unsupported data remain outside the
evaluator domain. The projection derives temporary-operation support and final serialization separately
from these same shapes. Exact named witnesses retain their code identity with a unit environment;
selected read/edit/once views transport one existing payload through ordinary resource transitions.
Value calls omit only that empty callable prefix from the declaration's explicit inputs, retaining
original argument positions and preparation effects. Known factories, array-selected named values,
scoped views and edited ordinary inputs use the same executor and runtime body. Dormant runtime-only
named values may be formed and destroyed, but every authored invocation still requires const evidence,
including dead source. Callable-bearing tuples and nonempty arrays are temporary data without final
constant-publication permission. Exact owned captured code derives invocation availability from its
ordinary checked operations, input shapes and actual cleanup. `constant::captured` closes transitive
captured-call facts and retains local failure causes; recursion supplies no missing fact. This does
not infer const for a named runtime helper. Formation/destruction of an eligible environment is
independent of body availability: a dormant runtime-only body can be formed and discarded. A called
unavailable body reports E0421 at its actual call, with causal locations in its own lexical code.
The same definition/body and inferred func/mut/once contract serve runtime and required evaluation.

The private source corpus qualifies empty and multiple explicit owned captures with nested captured/named values,
read/edit/consuming bodies, weaker read/edit/once views, independent Copy snapshots, explicit edited
inputs, factory-only results and per-instance state. The shared native source compares embedded
answers with ordinary execution. Arithmetic, bounds, work and call-depth failures retain actual
captured-body locations. Captured calls can also supply local/header array counts; supplier body
changes invalidate header reuse while unrelated body edits do not. General contextual captures,
outside R/D/S dependencies, formal callable substitution and opaque error-data operations remain
outside this evaluator domain. A named actual argument cannot stand in for an unclosed
formal input representation, even when unused. Domain checking reports the actual input before
projection; interrupted preparation never fabricates a completed call.

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
[construction dependency query](../nocter-dependency-query/README.md) for header, typed-body and
required-value requests. `dependencies` owns those
disjoint key/fact roles and consuming publication; there is no parallel typed-body cache or active
dependency stack. The common query records successful reads/resolutions, including cache hits. Header
facts acknowledge the declaration owner's ready contracts without containing another program or
type store. The coordinator accepts either a closed context or the exact-current
declaration preparation. A missing callable/record contract advances the ready header view; a
missing constant requests its value only after selecting its declaration identity. An unpublished
draft stops at either request and retries through the same checker after its dependency is ready.
Successful typed bodies become completed facts in that same query. No provisional value, syntax
dependency scanner, second source interpreter or completed-body retyping supplies a fact.
Header requests use the query's iterative progress entry. Lowering exposes declaration-owned
required-record identities; checking schedules them without inspecting fields or repeating
containment/Copy analysis. Header, body and value dependencies share iterative query-owned frames
and memoization. All remaining contracts, including unused records, pass through this entry
before source-unit closure. Header failures retain the facade's declaration-versus-checking
classification through body and value requests; a record failure never acquires a fictitious
callable owner. Lowering projects its owning field. Completed independent values survive failed
header work, but final Unit closure cannot admit any invalid unused declaration. Raw type recipes
load as requested through lowering; unknown or unsupported types in an unrelated header do not
block independent values. Source header steps can also request computed lengths; completed
values are read from this query before the declaration authority decides payload dependencies.
Constant references become validated primitive literals or immutable frozen-aggregate expressions in
ordinary expression checking,
before conditional completion, reachable breaks, capture invocation inference or control freezing.
All authored source remains structurally checked, including unevaluated branches. Selected const
callees are prepared through authored call edges and concrete targets revealed by result resolution;
callable-only recursion remains a graph, while a constant reference requests a value and exposes an
active declaration cycle. `accepted::Resolved` owns canonical typing, result and control closure
before effect-sensitive admission. Preparation tracks full lexical code with two demands: definition
type/witness suppliers and invocation. A dormant named value needs no body; an uninvoked captured
body can still need a callable-result supplier to finish its definition. Those type suppliers do
not make their scalar helpers execution dependencies.

If result closure reveals another target, preparation requests its ordinary body fact and resolves
the expanded set through that same authority. Raw completed body facts remain unchanged. Only code
needed for the required invocation closure proceeds to flow/resource/const admission and plans at
that stage. Whole-unit acceptance subsequently validates every definition, including dormant code;
an early valid value cannot publish an invalid Unit. One parent edge per needed code reconstructs
diagnostic paths on demand, distinguishing construction/type dependencies from invoked captured
code. Demand only grows from types to invocation, without an iteration cutoff. Source cases qualify
factory-only results, recursive/transitive bodies, independent counts, prefix inference, failure
causes and dormant-code cycles through this common path.


Header, body and value requests advance through explicit dependency groups, without host recursion
through constant chains. A body draft requests its missing contract/value or static prerequisite;
a value draft requests missing bodies in its authored call closure. Execution starts only after
that closure is complete and requests no new checking. Query-owned failure unwinding projects
each outgoing source cause through a consumer hook. The coordinator retains only those pending
source causes, not another active path or completed-fact authority. Successful retries discard
the prior pending cause. All roles share memoization, exact cycles and failure unwinding without
reopening the current key. The configured source depth budget diagnoses deep value dependencies
independently of the host call stack.

Required values can execute with unrelated callable/record contracts still unready. Every selected
const body retains its origin header view until canonical replay and contributes its exact body
bytes to source dependencies. A body becomes complete only after its static-value requirements;
re-entry into an unfinished body/value path reports the cycle at the responsible value reference.
Already completed dependencies survive a failed request, while its active branches unwind without
publishing unfinished facts. Callable-only recursion still closes through ready contracts and the
ordinary selected call graph. The same pass also checks ordinary and
unused definitions. Whole-unit closure requires every header, body and required value; selected
const-closure acceptance never grants complete Unit admission. Header-dependent normalization
requests these same values before publishing a closed array shape. Pure finite-payload header
cycles retain declaration-owned rejection; mixed header/body/value cycles report E0324 at the
responsible source request, including known but unready declaration names.
The [registered anonymous header roots](../nocter-declaration-lowering/README.md#exact-source-evidence)
also enter this same body/value path with a declared usize result and no formal inputs or source
return target. Selected counts can check and execute before their enclosing contract is ready;
ordinary typing, dead authored dependencies, const eligibility and evaluation errors retain their
real count-expression causes. Successful count execution alone does not publish its enclosing
contract. The explicit header request normalizes and validates it afterward; whole-Unit closure
still needs every header, body and required value. Header-only work does not add source call depth,
and nested type/value requests share the same execution/storage budgets.

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

Final publication admits bool, the intrinsic integer catalog, unit and finite closed optional/record/tuple/fixed-array
compositions under the shared final-data policy. Publication uses an explicit final-data policy over the shared schema traversal, independently of temporary
execution support and Copy. The admitted aggregate schema has no user destructors or retained
resources. Each constant reference creates a fresh ordinary value without granting its type Copy.
Constant names remain non-addressable. Constant member selection resolves fields through the
ordinary field authority, then selects established frozen components; scalar selections become
ordinary literals before control facts and flow admission. No runtime initializer call results.
Fixed-array values and computed source-header lengths use this path. Enums, generic conditions,
general outcomes, unresolved formal callbacks and outside-dependent payload evaluation still require
integration. Draft preparation can repeat work before a missing dependency resolves;
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
The completed query is consumed rather than cloned. `dependencies::finish` projects its successful
prerequisites into one source-only graph, eliminating source-free facts and sharing single-child
conjunctions. Bodies and captures select their own roots over that graph; no global set attaches
unrelated initializers to every body. Transitive constants and every authored const callee remain
requirements, including cache hits and unexecuted branches. A standalone typed/flow binding rejects
changes to those suppliers while allowing unrelated initializer edits; whole-Unit binding still
validates all definitions. The header-root projection is compacted to selected guards for reusable
declaration publication. This is source evidence, not a second dependency scheduler or value cache. Declaration lowering's
[exact source evidence](../nocter-declaration-lowering/README.md#exact-source-evidence) owns
that verification mechanism for both evaluator products and callable-result evidence. Whole-unit binding still verifies every
body. Ordinary editing calls have no result-only cache that could erase writes.

## Owned Capture Bodies

`closures` checks explicit owned captures under the private source contract. Formation evaluates
entries in source order through ordinary value supply and assembles one complete environment.
An empty capture set uses unit, one entry uses its payload, and multiple entries use the existing
structural tuple authority. A separate body scope receives one environment input and flat explicit
inputs. All local names resolve to interned places: captured names designate environment components,
not additional owning locals or an alias registry. Member/index selection, scoped access, update,
transfer and component disposal retain those same paths. Names outside the scope cannot become
implicit captures. The body returns
ordinary closed data and can construct nested closed-data environments under those same rules.
The body type authority reserves lexical IDs before descending into nested producers; sibling and
descendant code cannot reuse a pending parent's slot. Canonical program binding, result closure and
flow admission cover every nested body through the common scope. Exact named/captured callable
values can themselves form an owned environment, using the declaration owner's admission predicate.
Unmarked formation still needs source-context Copy; explicit transfer retires even a Copy function
value. Selected mut/once views survive capture. Result/application components follow the closure
boundary below; unbound formal payloads and inferred outside access remain outside this qualified scope.
Named factory calls inside the body use
the same callable-result closure as root bodies. Their applications retain actual lexical code,
not only the shared source/type root. A shared `flow::scope` traversal indexes captured controls
for result solving and caller-write evidence; distinct code never shares an operation arena.
Result projection seals each affected body and its dependency evidence before flow admission,
including a closure whose enclosing body had no pending result. Tests cover sibling/root calls,
distinct callees, incompatible witnesses and factory-body reuse invalidation. A shared native source
also invokes both thin returned functions and returned captured environments inside closures.
Nested source/native cases qualify three-level and sibling code, Copy snapshots, persistent updates,
consuming formation, returned environments and deep factory calls. Error-handle source tests retain
ordinary ownership handoff without repeated disposal; these do not execute native owned error graphs.
Callable-value source/native cases qualify independent mutable snapshots, nested state updates,
consuming calls, Copy of once-view named functions and recursive cleanup without invocation of a
dormant trapping body. No signature-wide Copy/cleanup assumption or extra callable registry is used.
The enclosing header remains its source-reuse owner, while lexical owner/slot identifies its code.

### Result-Resolved Owned Environments

Owned capture can retain valid pending result/application components, including callable-bearing
arrays/tuples and nested captured values. Formation freezes source operations first: unmarked
acquisition records its Copy obligation, and postfix transfer retires the source even if the final
environment is Copy. Source operations also determine the body's func/mut/once requirement;
actual payload properties cannot strengthen a formal contract or repair an unmarked consumption.
An environment awaiting a payload witness blocks raw flow admission even when its body never
reads that environment.

A received captured result retains its exact originating application as a relational input premise.
`results::producers` carries that premise through ordinary forwarding and prepared arguments.
The shared result solver closes it against actual producer relations; an uncommitted missing origin
starts as an empty equation, never an anchor manufactured from its signature. Root and nested code
retain their own operation/application identities throughout. The declaration owner's common type
projection resolves environment slots and checks positive containment before source admission.
Each capture constructor then requires an established owned environment. Still-unbound formal or
absent payloads reject locally rather than reaching resource/layout execution.

The shared `result-owned-captures.nct` source compares required answers with ordinary native calls
for result captures, nested/returned environments, callable-bearing arrays, independent snapshots,
persistent updates, explicit consuming bodies and repeated reads of non-Copy environments. Copy
once views keep snapshot behavior while explicit retirement remains visible. Runtime source also
forwards a captured result through a taking formal and preserves its exact witness. This does not
qualify general formal payload storage or formal substitution in required evaluation. Source tests
reject unmarked non-Copy acquisition/consumption, retired-source reuse, unknown capture payloads
and result-induced positive environment cycles. No new syntax, runtime dispatch or environment
ownership registry is introduced.

### Contextual Anonymous Contracts

A selected callable contract can supply omitted closure parameter data, input operations and result
data. `annotations` represents either ordinary data or that static callable constraint; the same
expression context reaches annotated values, selected call operands, declared callable results and
conditional result positions. It does not assert that a formal witness is the concrete expression's
payload identity. Ordinary constraint matching and invocation adaptation still close that identity.

`closures` checks parameter count and conflicting explicit parameter data/operations or result data
before body checking. An unmarked anonymous input inherits the selected operation even when its body
uses less authority. With no expected callable contract it remains read, and this bounded scope
requires explicit parameter/result data. Later uses do not repair missing context. Named declaration
operations remain their own contracts. Environment invocation is inferred separately from reached
capture operations; an expected view cannot grant a stronger invocation capability.

The shared contextual-closure source qualifies annotation/call/result completion, nested conditional
result positions, edited caller state, taking non-Copy inputs and mut/once adapters through required
and native execution. Formal callable invocation remains outside the evaluator domain; the fixture's
required paths invoke exact local/factory witnesses, while native source also calls selected formals.
General standalone inference, outside-reference capture and symbolic dependency contracts remain
separate realization obligations.

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
source, including Copy data. Component destruction retires only that component; consuming invocation
cleans the live remainder in reverse construction order. Source cases cover empty environments,
independent multiple-capture copies, nested transfer, disjoint components and partial disposal through
required evaluation and native execution. Error-handle cases establish logical disposal counts only.
OwnedCapture disposal retains the schema's environment cleanup obligation. A consuming body owns
and cleans its remainder; a read/edit body receives scoped input authority. Adapters preserve the
code/environment identity. Result equations accept only actually produced closure witnesses.
Unit admission validates every nested body, even an uninvoked or unreachable producer, before
publishing its closure-body catalog. Capture/body edits invalidate the enclosing source recipe.

## Body-Local Required Expressions

`required` captures a local array annotation or repetition's count request at its ordinary checking site.
It pairs the source owner's registered `ValueExpression` root with the names of active enclosing
bindings. The catalog supplies source identity, not lexical availability. A fresh ordinary
`Checker` checks the selected expression with an expected usize result, no formal inputs or
environment, and its own control targets. The current private implementation does not capture
enclosing let/var/read/edit bindings or callable parameters as compile-time inputs; their use
reports E0421 locally. Their names still participate in shadowing checks, including inside nested
count requests and count-local closures. This is an implementation limit, not a rule forbidding
future proven compile-time lexical values. Expression-local bindings, scoped updates, loops,
global constants and admitted const callees use ordinary checking and execution.

The common header/body/value coordinator keys body and value requests by complete `ValueCodeRoot`.
Its expression registry holds only paired request inputs and lexical exclusions; completed bodies,
computed values, dependencies and cycle frames remain in the one dependency query. Retrying a
parent reads the completed count before constructing the declaration-owned body array type.
Nested counts retain separate code roots, type suffixes and source causes. Required expressions
cannot return, break or continue into their parent. A count's enclosing callable result type,
parameters and const capability are not inherited.

Source reuse retains the whole enclosing body guard and selected supplier evidence. Typed call
reuse depends on the callee contract; executing that call also depends on its body. The completed
value request records that additional edge. A normalized type consuming the value must retain
those value prerequisites, rather than treating the typed call's proof as a computed-value proof.
Cycle diagnostics distinguish expression bodies/values from enclosing declarations; execution
failures retain actual expression/helper sites and the shared work, depth and storage limits.

Qualification covers source annotations and repetition counts, nested requests, surrounding lexical/control exclusions,
supplier changes, dead authored calls and mixed body/value cycles. Successful count computation
does not by itself qualify a value operation. Complete Unit publication still requires every
declared body and required value; partially completed count facts cannot publish a Unit. Count
values are not exposed as named constants or runtime-callable bodies. Listed construction and
whole-value handoffs use the [array construction path](#array-construction), and
[read selection](#array-selection) retains evaluated element addresses. Repetition uses the same
count authority and a compact value operation. Count completion precedes ordinary seed preparation,
including at zero. Indexed replacement and partial-state admission follow the
[selection owner](#array-selection); the linked native consumer qualifies its own execution scope.

## Closed Caller Write Effects

`flow::call_writes` closes exact named/captured-code evidence after callable results and supply, before numerical,
initialization and access analysis. One catalog retains exact typed-body owners in the shared final
program. Its summary solver computes finite may-write sets over environment and explicit edit-input roots; unknown
effects are the top element. Recursive input permutations and transitive calls use one selected-input
projection in `bindings`. Owned local updates have no external root. The shared `flow::aliases`
index follows lexical read/edit aliases through their typed binding initializers, preserving the original
explicit input position or owned-local root. It supplies no numeric path, address or lifetime
evidence; projected input writes remain conservative over that input root. Alias chains are
followed iteratively with cycle rejection. Replacement additionally checks the old edited
referent's cleanup type. Exact owned-closure cleanup follows its declaration-owned
environment schema recursively, including aggregate payloads. Destruction releases that environment
without invoking the captured body; body invocation effects do not become destruction effects.
The shared lexical scope supplies captured bodies to that same solver. An edited environment has
its own input root; a consuming body owns its received environment instead. Closure environments
also participate in cleanup eligibility, including read/edit bodies driven through taking adapters.
Unbound formal cleanup, unavailable callees and unsupported alias provenance remain unknown. Earlier
standalone type suffixes cannot supply cross-body evidence without the shared final program.

`call_writes::requests` substitutes the already frozen actual callable bindings into effect-only
requests keyed by lexical code and known witness/view bindings. Runtime environment contents do not
enter those keys. The existing checked body supplies every operation; no body is retyped and no
new callable type or capability is inferred. The finite monotone solver discovers reached requests,
including recursive forwarding, and resolves formal invocation and cleanup only when those actual
bindings supply evidence. Unbound templates retain their conservative effects. Required-expression
frames use that same catalog as roots, without becoming callable targets.

At each reached commit, input roots project to conservative caller write footprints. Projection
uses `CallKind::explicit_arguments`, so a prepared callable-value environment never
stands in for the first ordinary input. This remains true for zero-byte named values and mut/once
views. `CallKind::environment_argument` separately projects environment effects. A taking view edits
its received copied/transferred environment, without writing the original source again. Tests reject a named mutable callback that resets a prefix counter; the captured-callback
native case also writes a later explicit input.
Caller-side lexical aliases use that same binding resolver. A frozen evaluated alias widens to its existing
containing static place, so invalidation never reloads the old selector's now-changed index.
Unaliased operands preserve their selected footprint. These summaries do not change invocation
addresses or establish a definite replacement. Editing a
transferred/fresh temporary has no further write into the caller's remaining storage; the original
transfer still occurs. Current scalar facts and prefix histories survive disjoint known targets.
Overlapping may-writes invalidate facts without claiming a new array epoch; only a definite
containing replacement/end establishes that reset. An initialized edit preserves an already empty
extraction history, but cannot repair a prefix or an unproved history. Unknown aliases/callees remain conservative.
Argument transfer, access activation, availability and original bounds checks remain obligations;
a consumed counter is still unavailable. A signature or `const` promise supplies no write proof,
and this evidence does not promise allocation freedom or termination.

Required source closure and final whole-unit acceptance consume the same catalog path. Standalone
body queries have only their available closed definitions. Effect projection and signature-only-data
validation use the graph's shared structural reachability; detached call-completion points furnish
no effects or input bindings. Tests qualify recursive/transitive/root-permutation closure, selective
invalidation, interrupted calls, formal cleanup, missing callees and exact body-owner rejection.
Shared required/native source qualifies disjoint caller-field edits and private local writes
through alias chains beside a prefix loop. A negative case rejects reuse of a progression after
a callee resets the counter through an alias, including an alias supplied by the caller. Further
cases execute caller alias chains on disjoint fields and retain the original element address
after changing its index variable.
Header-count execution consumes the evidence before final whole-unit closure. A native source
case preserves a prefix loop through a call that constructs and drops a captured non-Copy array;
its dormant body would trap if incorrectly invoked. This qualifies checking and native cleanup,
not evaluator closure construction. Separate native source qualifies prefix loops through exact
read/mutating captured code, nested callable environments, later explicit edit inputs, copied once
views and consuming bodies, including maximum-count zero-byte geometry. Source tests reject
explicit counter resets and preserve transitive body/value reuse guards. Known actual callbacks close effects without strengthening an unbound formal contract. Source
cases retain real explicit-input counter writes and separate invocation-body guards from dormant
cleanup. General alias/root precision, unbound formal invocation and user destruction need further
closure.

Flow-query rebinding verifies the reached callee bodies and their typed value/count and result
prerequisites through one shared recursive dependency graph. A typed call may survive a callee-only
body edit while its old flow proof cannot. Unrelated bodies are not dependencies. Dependency edges retain the bound request, so callbacks selected by other callers do not become
this caller's dependencies. Each lexical source guard is checked once per verification, including
recursive requests; dormant callable cleanup adds no invocation-body dependency. The graph retains
existing source guards rather than body IR or a second source fingerprint. Whole-unit rebinding already
verifies every body before publication and does not repeat this traversal for each body.

## Array Construction

`construction::arrays` checks listed/repeated construction through the ordinary expression and
value-supply boundaries and the ordinary body/resource/evaluator path. An authored array context
supplies its element data and exact count. Inference checks each listed component independently and requires
equal data identities; the first element supplies no coercion context to the rest. Empty lists
need contextual element data. Listed expressions retain source-ordered `ValueField::Array` components
in the existing aggregate expression product and use the same body type authority, including in
nested arrays and tuples. Pending callable results follow the [result closure](#callable-result-closure)
before construction can acquire the declaration-owned `owned_payload_supported` evidence used by
owned environments. Structural formal contracts alone supply no such evidence. Native source qualifies independent mutable
captured state through tuple components and dynamic array selection; the evaluator's narrower
callable domain is defined above.

Repetition closes its count through the common query, then prepares its seed even at zero count.
The seed must be supplied as an ordinary value; zero does not turn a non-Copy read into an owner
or suppress a required constant dependency. Counts above one require the declaration authority's
Copy evidence. Typing a repetition does not execute a runtime seed. The typed repeat retains one
seed expression; its result's array schema owns the count. Execution evaluates that seed once.
At zero, the ordinary ledger disposes it before assembling the empty array; at one, ordinary
assembly moves it into the single element. Larger counts snapshot Copy data into the evaluator's
[compact repeated storage](../nocter-constant-evaluation/README.md#evaluated-array-addresses), then
retire the seed and construct the array obligation. No per-element list or second ownership ledger
is generated for repetition. Interruption creates no array value or parent resource obligation.

The internal assembly ledger validates listed array cardinality and each component through the
same declaration projection used for records/tuples. Distinct complete component owners transfer
atomically into a fresh destination; duplicate positions/sources, wrong data/families/counts and
an occupied destination cannot publish partial changes. Whole-array read/edit/take inputs,
snapshots, explicit moves, replacement, source-ordered component effects and interrupted construction
use the existing aggregate/control/initialization/access/resource products. No parallel array
ledger or source checker is added. Required evaluation freezes admitted listed arrays through
the same evaluator shape/value/publication path, including arrays constructed inside required
length bodies. Source tests cover nested/empty/zero-sized data, non-Copy records, independent
snapshots, editing calls and reverse cleanup of interrupted component prefixes.

Source repetition qualification covers zero/one/many and maximum u64 counts, non-Copy zero/one
seeds, source effects, interrupted seeds, seed disposal order, independent whole-value snapshots,
compact frozen constants, nested selection, seed-supplier invalidation and a small live-storage
budget. A scalar result of maximum-count repetition reaches the common native consumer.
This closes listed/repeated construction and whole-value handoffs in the private Unit. Element
reads and element transfers use the [selection path](#array-selection). The [native path](../nocter-machine/README.md#private-array-execution)
qualifies its own stored array scope.

## Array Selection

`selection::arrays` prepares primitive bracket selection through ordinary expression/value-supply
checking. Existing array storage identifies an evaluated referent; a computed array supplies a Copy
component as a produced value. An existing non-Copy element can therefore initialize a scoped read
alias, or an edit alias with writable source authority; a computed component cannot. A produced Copy
component may lend its own editable temporary; an existing element cannot silently become such a
temporary. Named constants construct values, while immutable statics retain their existing-place
identity.

The index receives the ordinary expected usize boundary. Signed variables, booleans, unavailable
names and out-of-range literal magnitudes keep their local diagnostics. `postfix` owns one iterative
member/index/call/transfer preparation spine. A leading direct call retains its selected direct
operation, and subsequent value calls use the same argument/bundle preparation. An existing static
member prefix remains one ordinary place; consecutive computed members become one selected path,
without intermediate record/array snapshots. Graph scheduling consumes the same selected operations.
Long invalid chains reach their actual operand/call failure without recursive spine preparation or
a source-level length ceiling.

Exact callable elements can be invoked directly. Required/native source qualifies named functions,
independent mutable captured elements, consuming non-Copy elements, Copy once views, mixed
member/call/index results, factory-returned selections and interrupted inputs. The callee's index
is prepared before explicit inputs: later index-variable mutation cannot retarget its environment.
Read/update invocation retains the selected address; taking a Copy invocation view snapshots its
environment and permits otherwise independent later replacement. Explicit element transfer still
retires that element, including Copy data; subsequent use and overlapping mutation reject locally.
General user index protocols remain a separate gate.

`Referent` is one typed-body operand for either a static selected place or an evaluated access
expression, including member projections from an evaluated access. Read reservations, Copy supply,
alias binding, assignment targets, liveness and evaluator projection consume that operand without
synthetic locals or a second owner map. A read input reserves the selected address; a taking Copy input snapshots its
payload at preparation and holds no later read reservation. The graph evaluates the base and index
once, in order, before publishing the selection. The evaluator retains the numeric address, rather
than reloading the user's index variable; computed Copy results use the same bounds/address
operation with a data destination.

Edit inputs, scoped edit bindings and direct assignment resolve writable source authority through
`Referent::static_base`, using the same declared binding authority as static places. The [shared
access model](#shared-flow-access-and-resource-validation) separates retained projections from
active read/edit grants. Edit inputs activate at bundle commit, while a scoped edit binding
activates at its declaration. Later call operands may read before activation but cannot invalidate a
reserved array. Captured-element edits use the same base resolution to select invocation mode.

Direct assignment and compound update prepare the RHS before evaluating the target. The target's
base and indices execute once in the same evaluation frame; a failing or exiting RHS skips them,
and a target exit cleans the prepared replacement without publishing a write. Compound updates read
the old selected value after target preparation. Static and evaluated targets share `Referent`;
proven owning targets retain their ordinary sparse replacement transition. Unproved/scoped
targets use the exact-address `ReplaceReferent` handoff without interpreting a conflict footprint
as an ownership key; the partial-transfer rules below define restoration and completeness.

`flow::Query::numeric_at` reports normal checked usize difference equalities at an existing graph
operation. The analysis distinguishes current owned scalar locals, scalar read inputs, computed
results and frozen prepared operands. Read-input cells describe values under their existing
access grant; they create no owner or update authority. Unknown writes forget their current facts
as well. Assignment preserves relationships between older snapshots while changing
the current cell; joins keep common exact differences and reach a fixed point without an
array-cardinality-dependent iteration cutoff. Selected builtin equality branches refine those
relationships. Whole-local read/edit alias chains resolve to the same existing scalar cell. Binding
or ending such an alias creates or retires no numerical owner; copied operands remain frozen across
later aliased writes. Exact whole-local writes also preserve separation from distinct owning arrays.
Fields and evaluated selections receive no scalar identity from that lexical root evidence.
Unsupported operations, unresolved alias writes and calls without a closed write summary
remain conservative. Impossible normal arithmetic supplies no proof. This report neither grants
access to an unavailable value nor reclassifies a source bounds check. Numerical history
partitions distinguish a complete epoch, one point taken from completeness, a proven prefix and
unproved coverage; fixed body cells retain the last committed index. Restoring the sole point
to its saved index recovers completeness and clears that epoch's source provenance. A prefix
endpoint of zero is also one missing element. Longer prefixes and generic unproved coverage do
not acquire this reset. `NumericFacts::extends_prefix` reports a zero start or
unit-stride continuation for the current transfer, including ordered transfers in an atomic call
bundle. Preparation/cancellation create no event; containing replacement starts a new epoch.
Ordinary element replacement and compound update preserve an already complete owning-array epoch,
observed after RHS effects. They cannot erase other holes or reset a partial prefix; compound update
cannot restore an absent old value. Other overlapping partial writes invalidate the prefix, and
distinct containing arrays remain separate.
No prefix history crosses a call without a closed write summary. May-executed transfer sites
form a finite provenance set alongside the difference facts; binding, containing replacement
and scope exit forget the affected epoch. `NumericFacts::prefix_frontier` selects a prefix
lifecycle when a proved site repeats, or its prefix crosses a backedge within the site's own loop.
The latter includes a condition that permits just one transfer: initialization joins still need the
stable prefix relation. A backedge does not enroll a one-time move outside that loop or a disjoint
containing array. Epoch resets still forget their sites. Finite co-executed-site relationships connect initial and later transfers; merely sharing an array local does not connect
an earlier point-restoration epoch to a later prefix loop after replacement. Straight-line moves,
point restoration and epochs ended or fully reset before the backedge retain their point realization.
An initial seed and later repeated selectors for the same concrete parent share one body-scoped
`ArrayFrontier` key. Its representative identifies backing storage, never the current number to
capture. Parent projection consumes the same typed structural traversal and exact array schema.
The numerical report creates no resource backing or holes. Its source consumer below independently
checks containing storage and uses the existing access/resource transition. A prepared whole-element
argument can use the same history to establish that its evaluated index is strictly beyond every
committed endpoint; preparation does not extend the prefix. At normal call commit, marked prefix arguments use that operation's ordered extension proofs
after whole-bundle validation. Function-view adaptation retains the original payload type in its
existing source-expression owner; intermediate adapters use their own expression owners and the
final view enters the prepared argument owner. Each selected payload is protected before endpoint replacement,
then follows the ordinary taking or read/edit temporary handoff. No numerical prefix crosses the
invocation without a closed write summary.
Within one direct marked bundle, ordered extension proofs for distinct selectors of the same
prefix establish distinct consecutive elements. Bundle validation consumes these closed proofs
before exempting its own neutral frozen-place nodes from per-source checks. Existing access grants,
external aliases and outer-call reservations remain checked. General unknown index relations and
retained aliases remain conservative; a source key alone supplies no numerical identity.

Access regions retain `ValuePath<IndexFact>` under the same owning/input/temporary root. Known
numeric results select concrete indices; unknown indices remain unknown even when their recipe
keys or representations compare equal. The [shared structural relation](../nocter-model/README.md#structural-projection-relations)
can prove disjoint known elements or later record/tuple/array components after an unknown index.
It never appends element fields directly to the containing array prefix or uses byte offsets as
logical identity. This permits independent field access for both equal and distinct runtime indices,
including zero-byte data, without an index-name or loop-generation proof.

`paths` owns `IndexFact`, concrete/symbolic path conversion and the relation consumed by both
initialization and access analysis. `Referent::projection_path` supplies one structural traversal,
preserving the static member prefix and every array/field suffix. `flow::numeric::selection`
refines unknown indices with the current operation's proved in-range constants for availability,
new access regions and resource paths. Each consumer replays numerical state once through a block;
it does not store a numerical state for every action. A created region or missing path freezes
that constant, so later changes to the source index cannot retarget it. Constant holes need no
saved-index owner; unproved indices retain their existing snapshot obligation. The original
source bounds disposition is unchanged. Unknown indices stay unknown even when recipe keys match.
Outside referent identity remains access analysis's responsibility; a local availability path
does not retarget an alias.

Place formation requires a present containing place; the selected child slot may be missing.
Value supply, read/edit reservations and scalar
snapshots require the selected complete payload at the point of use, after preceding operand
effects. Sparse availability queries conservatively include every possibly overlapping hole;
later disjoint fields can exclude a hole without inventing a concrete unknown index. Concrete
and symbolic availability use the same query, with joins solved before reporting. Exhaustive
small-domain enumeration compares symbolic queries with all possible concrete selections,
including missing parents, partial children, symbolic holes and maybe-initialized state. Missing
unknown selectors never become equal merely because their syntax or availability-map key matches.
Proven containing replacement clears their descendants; unproved overlap cannot choose an old-value
cleanup branch. Base identity stays protected
during index evaluation, and saved addresses remain the exact execution authority. Checking freezes
`ProvenInBounds` when an established normal-result singleton is below the normalized array count,
and `ProvenTrap` for an empty array or an established singleton at or above that count; other
indices retain `Required`. This uses the [one disposition role](../nocter-model/README.md#structural-projection-relations),
not a new bounds evaluator in later stages. The complete base/index evaluation and effects remain,
including zero-byte selections. A guaranteed failure schedules `IndexFailure` only after both
operands complete and terminates without a selected address, supplied result, Copy obligation,
call commit or unwinding cleanup. Prior effects, including zero-repetition seed disposal, remain.
No payload header is requested for an element that is never produced; a later authored member
selection still requires its structural definition. Empty local annotations retain element identity
without requiring its payload header. The closed evaluator checks actual values for ordinary
selection and executes explicit guaranteed failure at its exact source site; native lowering may
omit only a checking-proven check. Access disjointness alone does not prove a missing element's
identity, restoration or cleanup interval.

Postfix transfer carries the same `Referent` as ordinary selection and assignment. Proven
numeric singleton paths can retire an exact element or nested field. Evaluated nested selectors
can retire an element or subplace using one owned retained usize snapshot for each unknown index
along that path. The shared `Referent::projection_path` traversal supplies both conservative access facts
and the resource recipe; an unknown conflict footprint never becomes an owning key.
For a partial selected element, remainder cleanup descends into that element between the higher
and lower intact spans. It cleans its surviving fields in reverse order, ends its container and
keeps the snapshot owned until every dependent cleanup finishes. No per-element metadata is added.

The graph evaluates base/index effects and the original bounds operation before immediate transfer.
A direct call instead prepares its selected address and delays the snapshot/source retirement until
full bundle commit. Cancellation releases reservations and creates neither snapshot nor hole.
Read/edit sources cannot surrender their referents through a marker. A guaranteed-trap source
produces no moved value. Source availability retains symbolic holes conservatively, while the
resource ledger retains the actual snapshot identity. Existing reads, writes and repeated extraction
must exclude every possible hole; an unproved selector cannot restore one by key equality.
The source corpus also qualifies flow-proved constant holes, later disjoint reads/transfers,
frozen read arguments and aliases, exact restoration and multi-element call bundles. Logical
tests require no saved-index owners for those constant paths and reject overlapping/unknown
holes and invalidation of an older frozen alias.

Known owning replacements can restore proven missing elements. Replacing a containing array cleans
its old remainder, ends the old snapshot and permits reuse of that selection recipe on a backedge.
The same rule applies beneath a concrete record/array prefix. Unknown or possibly overlapping
selected replacement still requires complete storage. Partial fields can remain available when
structural disjointness excludes every hole.

One retained whole-element hole beneath a concrete parent can also be restored without a constant
index. The existing committed-index history must prove equality with the current target and
identify one retirement source on every reached history. Availability checks the actual unavailable
slot's causes and present array; resources require the matching `Missing::Value` and live snapshot.
Prefix histories and unproved index equality provide no single-origin restoration proof. The replacement
uses the old retained path, installs its evaluated value, then retires unused backing. Original
target evaluation, bounds checks and access conflicts remain in force. Restored backedges may
reuse that snapshot recipe for a newly evaluated number after its prior lifetime ends.
Shared source qualifies changed user indices, repeated alternating indices, direct call retirement,
restoration on both branches and maximum-count zero-byte arrays. A logical owned-error case checks
that missing old data is not destroyed and snapshot retirement follows installation; actual native
owned-error construction remains unqualified. Partial-field restoration and general relations
among multiple retained holes remain separate obligations.
Source also qualifies a later prefix loop after restoring the sole point, without replacing the
containing array. Independent negative checks retain earlier field holes and longer-prefix holes.

Conditional restoration additionally supports complete-or-single-point histories at one assignment.
Equality with the saved index is required only on the histories that contain a point; complete
histories supply no invented snapshot. The proof retains every possible retirement source.
Availability closes only whole-element constraints beneath that array whose causes all match
those sources; its parent must be present and any other overlapping unavailable coverage rejects
the proof. Concrete and symbolic constraints may describe different branches of that same point.
Prefixes and general unproved histories do not acquire this conditional proof.
Resources select the actual unique missing-point path from each ownership alternative, preserving
constant paths whose saved index was elided. An alternative without a missing point must have a
complete array and uses ordinary initialized referent replacement. No numerical-equality predicate selects a backing lifetime, and inactive backing is
never read. Shared required/native source covers absent holes, distinct retirement sites with equal
indices, changed user indices and maximum-count zero-byte arrays. Logical error data checks the
initialized-replacement versus saved-point alternatives; it does not qualify native error graphs.
Cases replace elements with different values so an omitted or wrongly addressed write changes
the result, including a flow-refined constant whose snapshot is elided.

The shared retained-source corpus executes in required evaluation and through the
[native consumer](../nocter-machine/README.md#private-array-execution). It covers normal and nested
concrete parents, later-argument/original-index mutation, take/read/edit handoffs, ownership
alternatives at one selection, different snapshots capturing equal numbers across a join,
containing replacement on loop backedges and maximum-count zero-byte cleanup. Independent holes
below disjoint concrete parents share one owner: replacing one parent retires only its snapshot
while the other survives backedges and conditional/final cleanup, including physically co-located
zero-byte arrays. Source cancellation
has no retained steps; zero-byte out-of-range transfers fail before capture. Existing cases also
cover concrete field holes/restoration, disjoint indexed edits and consuming captures.
Dynamic partial-element cases cover record fields, constant inner-array selections, later index
mutation, distinct conditional holes, whole replacement and cancellation, including maximum-count
zero-byte elements. Logical error-field tests independently qualify remaining-field cleanup and
exact snapshot retirement; these do not establish source construction/native execution of error graphs.
Nested dynamic cases freeze each index independently, including mutation of the outer variable
while evaluating the inner index, and qualify direct call commit and two-dimensional maximum-count
zero-byte cleanup. A later capture failure rolls back the entire staged transfer. Restoration and
cleanup end every snapshot attached to the one missing path.

Unit-stride repeated immediate extraction without containing replacement uses the selected prefix
lifecycle. The first transfer starts at zero and every later transfer advances by one under the
closed difference proof. The selected payload is delivered through its actual frozen alias before
the ordinary saved endpoint owner is replaced. One sparse missing-prefix region replaces individual
element holes; exact remainder cleanup uses that endpoint even on an early return before the user's
counter update. Shared required/native cases cover a concrete nested parent beside another missing
row, maximum-count zero-byte arrays, and cancellation after reserving an initialized tail element.
That pending transfer creates no snapshot, element move or endpoint update.
Normal direct calls after a prefix loop qualify taking and marked read/edit handoffs, including
mutation of the user's counter in a later argument. Source transition tests require payload
protection, endpoint replacement and region consolidation before the invocation handoff; a stale
element remains a local flow rejection.
Ordered prefix bundles also qualify two distinct payloads, later counter mutation and cancellation
after two reserved elements, including maximum-count zero-byte arrays. Each payload uses its own
frozen address and ordinary argument owner before the shared endpoint changes again.
The [closed caller-write evidence](#closed-caller-write-effects) also qualifies taking calls inside
the prefix loop, including transitive mutually recursive callees. The shared source corpus executes
that path in required evaluation and native code; no source effect annotation is added.
Selective-call cases update a disjoint field beside the partially moved array and edit transferred
element temporaries while retaining the correct prefix and exact remainder cleanup.

Multiple dynamic holes also compose when structural field disjointness excludes overlap, including
equal, ascending and descending actual saved indices. Each transfer still requires an initialized
selected payload; a recipe key or cleanup predicate cannot prove distinctness for the same field.
General conditional/multi-hole restoration, non-prefix repeated extraction without proved
intervening restoration, broader numeric relations and unclosed call writes still require the
[evaluated-selection closure](../../../design/compiler-realization.md#evaluated-array-selection-closure).
Repeated recipe keys alone never prove equal numbers across a backedge.

The shared adapted-element source qualifies atomic view changes while draining a prefix, paired
arguments, independent read/edit environments, a non-Copy capture, zero-byte named function values
and cancellation before commit. Local adapted views also compute required answers through the
same ledger/evaluator operations; deferred formal calls execute through native instances. General
formal callable evaluator substitution remains outside the [constant domain](#constant-domain-acceptance).
Overlapping argument sources and callbacks that really reset the counter still reject. The one-
iteration regression also uses ordinary unit elements, independently of callable representation.

Source tests qualify saved aliases after index mutation, loop re-execution, read reservation versus
Copy snapshots, non-Copy element/field reads, structural field views, alternating member/index chains,
once-in-order index effects, parent/index-side invalidation, interrupted preparation, computed-base
alias rejection, zero-sized bounds effects and local operand failures. Indexed edit-input tests
qualify listed/repeated and non-Copy data, edit parameters/aliases, saved indices, read-only
rejection, conservative overlap, preparation ordering, interruption and captured-environment
invocation. Scoped indexed aliases additionally qualify exact record/field replacement, saved
indices, Copy snapshots, read-child exclusivity, read-only/computed rejection, bounded error cleanup
responsibility and deep update-demand propagation. Direct writes additionally qualify RHS/index
order, compound reads after RHS effects, mixed record fields, non-Copy replacement, interrupted
RHS/target cleanup, loop re-execution, maximum-count sparse Copy snapshots, zero/out-of-range
bounds and read-only/overlapping target rejection. Structural-region tests cover known disjoint
inputs, unknown indices separated by later fields/elements, read siblings, reserved-input effects,
conditional singleton results with effects/exits, unproved overlap, whole-parent invalidation and
zero-byte logical identity. Disposition cases cover literals, named constants, conditional
singletons, interrupted indices, unknown inputs and maximum-size bounds. Required bounds failure reports E0325 without
publishing a value. Compile-time selection also reaches the [common native
consumer](../../../architecture/pipeline/value-access-declarations.md#common-machine-body-construction)
when its frozen result is scalar. The [native array consumer](../nocter-machine/README.md#private-array-execution)
separately qualifies stored values and ABI; checking alone grants no physical-layout proof.

## Concrete Array Resource Frontiers

The ordinary resource ledger accepts closed array payload descriptors for internal ownership
transitions. Positive lengths validate the element schema once; zero lengths carry no element
resource edge. `Disposal::Aggregate` retains the structural container obligation, while the
declaration schema selects record/tuple or homogeneous cleanup. No element-sized metadata list,
parallel array ownership map or new subarray type is constructed.

`resources::Path` separates constant numeric indices from descriptors for retained checked-index
backing. It has no unknown-index variant: an access footprint cannot become an ownership key.
The descriptor names reusable body-local storage and its original bound, not equal numbers across
activations; it alone creates neither a snapshot nor ownership. Internally, a transfer stages its
evaluated usize operands from the outermost selector inward into `Holder::IndexSnapshot`, validating
each containing array through already captured outer indices, then records one retained hole. Each ordinary scalar owner remains in
the existing ledger and branch conditions after its operand frame ends. Restoring the same hole
retires its backing after the final use; whole-container cleanup/replacement does the same after
its remaining spans and container end. Live backing cannot be overwritten by recipe reuse, and
failed transfer/install leaves both state and steps unchanged. Snapshot owners cannot be constructed
or consumed through the generic value APIs. The [selection owner](#array-selection) defines source
admission; evaluator and MIR project the retained identity to an actual scalar slot without
substituting a number for its descriptor.

Concrete element paths use the [declaration-owned projection](../nocter-declarations/README.md#future-value-declaration-boundary).
The existing prefix-free missing-coverage set records transferred elements, partial element fields
and proven array prefixes.
An intact array remains one whole-payload disposal. For an incomplete array, `resources::payload`
derives the affected indices from those paths and emits `DisposeArrayElements` for each nonempty
intact interval, interleaved with partial-element cleanup in reverse index order. Its half-open
u64 bounds and array payload survive payload projection unchanged; they do not become host
ordinals or a guessed tuple schema. `ReleaseContainer` ends the array after its remainder,
including when every element has moved. Restoration, initialized replacement and absence of the
parent use the same atomic transfer/replacement rules as other structural places.

For one retained hole, the same effect carries `ArrayRange::After(index)` and
`Before(index)` around the selected child's remaining cleanup, recursively for nested arrays.
These spans consume the original checked bound and backing;
either side can be empty. No descriptor ordering is interpreted as numeric ordering. Cleanup and
restoration reject a concrete path which may intersect a missing ancestor: possible overlap is
not proof of a contained hole. A proven prefix instead carries one `Missing::ArrayPrefix` with its
concrete array base and `Holder::ArrayFrontier` scalar owner. These are coverage and backing roles,
not a mutable element address or an immutable selected-point snapshot. `MoveArrayElement` delivers
the actual selected payload first; the same staged transition then replaces the endpoint, records
the region and consolidates coverage. Cleanup emits only `After(frontier)`, then the container end,
then backing retirement. Generic value operations cannot create or consume either backing role.
When several retained paths affect the same array, `DisposeArrayRemainder` freezes that owner's
existing missing coverage and ends the container after remaining cleanup. Point paths and prefix
regions keep their distinct backing/coverage roles. It does not sort descriptor
keys as numbers or create another ownership map. Equal runtime indices combine their field
exclusions. All backing stays owned through the operation, and initialized source/container checks
remain separate obligations. The constant and native consumers own their execution strategies.
Mixed cases combine a prefix loop below one concrete field with a dynamic move from a disjoint
field, including zero-trip exits, equal numeric backing values and maximum-count zero-byte storage.
Independent ledger tests qualify finite owner counts, ordering and atomic rejection of malformed
operands, occupied destinations, point/prefix mixing and foreign body descriptors. General numeric
relations between independently selected holes still need further proof.

Ledger qualification covers listed component assembly, maximum u64 cardinality, first/last and out-of-bounds positions,
empty arrays, zero-sized elements, partial record elements, replacement and container retirement.
These tests start with declaration-backed logical owners; the [selection owner](#array-selection)
separately defines source qualification. Distinct dynamic recipe keys are never disjoint numeric
positions. `DisposeArrayElements` reaches the evaluator's metered retirement frontier and private
MIR without element expansion; each consumer owns its physical/execution qualification.

## Frozen Body Input Channels

A typed query's code identity selects its named or captured body. Its derived source owner selects
only the enclosing source reuse guard; these are separate roles, not interchangeable lookup keys.
The typed query retains the source owner's paired `BodySource`. Current source binding and
input-layout admission use one proof that executable code and this independently retained source
owner agree.
The accepted Unit exposes exact code-to-body selection without a name or header/body fallback.
Checked-body catalogs, type replay, result applications and evaluator projection retain complete
code roots. Admission verifies a catalog key against its body's code before replaying any types.
Only declaration roots contribute declared callable-result equations; an expression cannot
inherit its source parent's result contract or const capability. Evaluator diagnostics select
the actual root's paired source from that same checked catalog, including for nested call sites.
The public declared-body iterator projects declaration-root entries from the closed body catalog;
source ownership never selects an executable body. A checked required expression has no explicit
or environment inputs and a usize result. Unit/target runtime selection excludes that code role;
its private checking and evaluation follow the [required-expression path](#body-local-required-expressions).

`body_inputs` owns coverage and contract validation of the typed body's input layout: one optional
environment channel and an authored-order sequence of explicit input slots and operations. Named
parameters retain their exact declaration identities; captured and anonymous inputs retain their
body-local roles. Every input local occurs exactly once, with a matching slot kind and contract.
Control construction consumes this order for root scope cleanup. Capture demand freezes the
environment operation with its local authority before final contract validation and flow admission.
No downstream consumer infers input position or an environment channel from local allocation order,
body use, source names or argument count. Mutation tests reject reordered/duplicate/missing slots,
changed operations, substituted environments and an enclosing source owner used as closure code.
