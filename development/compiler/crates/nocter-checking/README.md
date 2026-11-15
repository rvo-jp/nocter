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

Individual body queries also consume declaration-lowering's physical module-source views.
Function/constant/static selection, local collision checks, annotation leaves and record construction
use the context's exact lookup scope through the shared declaration namespace. Same-spelling names
in independent sources retain distinct identities and types; only directly seen authored names are
visible. Body checking, body-local required checking and typed-query binding select the actual
owner through that same retained source binding. A query can open from a sibling source view while
its syntax, names and origins still come from its own physical body; it does not search again for
a same-spelling callee. Source guards verify all selected owners before reuse. Required-evaluation
origin projection can likewise identify an operation in any bound physical source.
`accepted::ModuleSourceUnit` consumes explicit v2 `CompileUnitInput` through topology, source-role,
pairing and scoped reservation before using that same source coordinator. `SourceHeaders` supplies
single-source or module header advancement, current views and cycle rejection; body/value requests,
const-domain checking, actual execution, source dependency guards and final Unit admission remain
one implementation. Every physical count supplier executes in its own source scope before repeated
headers agree. The final declarations are an immutable current `ModuleContext`, and every selected
source is checked, including unused helpers and records. Empty libraries need no fabricated body.
Unselected primitives reject at their authored declaration. Explicit compile-input locators resolve
exact logical-module declarations through the shared toolchain source owner; the existing value
signature validator alone promotes selected roles to checked facts. Role snapshots follow the same
progressive header views and sealed context. Bodyless primitive headers have source positions but
no fabricated checked body; direct and function-value calls consume the same checked role evidence. Sealing checks effective root audiences and outward nominal signature exposure through
that same declaration binding; hidden representation payloads remain private. Top-level selected
record-type imports and root re-export chains preserve their original nominal identity through
ordinary header/body checking. Field selection and record construction also require the physical
source's declaration-lowering-owned view: visible root fields do not grant hidden fields or an
exhaustive structural construction contract. Both computed member values and existing-place
selections use the same field check. Complete schemas remain available to compiler-owned layout
and cleanup. Qualified type annotations, function values/calls, constants, static places and
record heads resolve through the same source namespace. The shared prefix query selects the
declaration once before ordinary record/tuple member selection; uninterned member suffixes do
not become namespace errors. Named and qualified references share value/static materialization.
Intermediate namespace audiences retain the original lookup caller, and a namespace is neither a
value nor an owning transfer source. Required evaluation uses the selected concrete targets and
actual physical bodies. Body-local imports use the same immutable name authority and the lowering
owner's exact occurrence-to-lexical-scope projection. Function/closure bodies, annotations and
body-local required lengths therefore resolve names in their own enclosing block without a second
checker-side namespace or a mutable name-scope snapshot. Imported aliases cannot shadow enclosing
runtime bindings. Leaving a block does not expose its imports to siblings or later statements.
Selected source preparation retains the actual header-value source guards in the lowering owner's
`ModuleQuery`. Rebinding equal headers into fresh source identities uses that same semantic
program; existing body queries require their independent current-body and dependency guards.
Changed runtime bodies can preserve headers, while changed count suppliers, import bindings or
audiences invalidate affected reuse. Current primitive selections must match retained checked roles;
new source input cannot acquire an earlier profile's authority. The proof currently requires equal physical inventory and
closed name/slot layout. Ordinary package/CLI/editor integration and finer incremental reuse remain
work; this bounded entry does not qualify them.

All source annotations consume lowering's one recursive `TypeSyntax`, including parenthesized
callable contracts. Parentheses do not create another data or invocation identity. Local callable
annotations resolve nested input/result signatures into declaration-owned `FunctionPattern`
constraints. They check an existing named/formal/result witness recursively, preserve its identity
and ordinary formal bindings, and report the mismatching input/result path at the supplied value.
Only the outer invocation can adapt; a nested signature cannot alter the operation or invocation
contract declared by the selected function. Scoped aliases retain the ordinary adapter restriction.
An anonymous implementation receives available data and result constraints directly; nested
callable results close from the checked returned values. Anonymous callable inputs with ordinary
signature data publish their own lexical formal identities before body checking. Explicit and
contextual signatures share this path; they never inherit another declaration's formal identity.
Deeper callable positions within those input signatures still reject locally. Callable data in unsupported carrier/header positions still rejects
through the local/header diagnostic owner. Parsing a constant/static callable annotation does not
grant final serialization or runtime initialization.

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

The same interned producer nodes retain original constructed outcome tags. Missing facts remain
distinct from a described unknown value; an unknown child or weak update cannot preserve another
path's known tag as a certificate. Joins keep a tag only when the incoming descriptions agree.
Outcome retagging preserves that fact, and strong updates retain earlier immutable snapshots.
`ProducerShape::primary_at` exposes it at an original creation/component path. It proves neither
an initialized value nor a represented callable domain, code witness, cleanup or const eligibility.
The positive producer equations do not use an empty witness set as a tag fact.

`Unit::instantiate_control` selects continuations in the original control graph for one complete
code frame. A canonical outcome key whose primary child is uninhabited can exclude its positive
continuation; unresolved metadata retains the original branch. The query preserves the original
actions, including carrier evaluation and tag reads. Its read-only `InstanceControl` retains the
frame and selected original terminators; it proves neither an initialized value nor source access.
Required dependency discovery and bound const clauses, ordinary value/preparation maps, evaluator
plans and private native instance/lifecycle/MIR projection consume this same selection. Bound
clause results retain the control map so a missing call edge is accepted only with its original
unreachable commit. Dormant source contracts keep their separate definition-time checks.
Representation-based branch pruning does not infer a tag from an unknown domain or from the
absence of positive producer candidates. Optional recovery with an early-return fallback is
qualified for take/read/edit inputs, including effectful input creation. Take/read/edit assertions
also retain their original check while excluding an impossible normal continuation. Definition
and instance refinement share `flow::assertion_source`; value checks resolve their original
operand snapshot, while read/edit checks resolve the selected referent. Required execution reports
the original `!` as an inactive-variant trap, and native execution preserves the safety trap and
inhabited counterparts. Recovery/result joins and partially active const selections remain
separate qualification obligations.

`received-outcome-propagation.nct` qualifies optional propagation through take/read/edit inputs,
including absent and positive applications of the same body in required/native execution.
`received-failure-propagation.nct` qualifies runtime failure propagation and recovery with actual
owned error graphs and captured callable environments. Propagation stages the absence/error
before ordinary cleanup; a projected fallible `?` still rejects at its source because read/edit
access cannot transfer the caller's error.
`received-compound-outcome-control.nct` qualifies the same continuation selection at a tuple
component through take/read/edit inputs. A neighboring live callable and scalar field retain their
independent domains and offsets. Only the selected empty primary continuation is excluded; the
ordinary scalar condition and inhabited counterpart keep their branches. Distinct fallback/callback
answers, required execution and native error-owning environments qualify remaining-container cleanup.
The same source qualifies optional propagation and assertions of the selected component for all
three input operations. Propagation stages absence before cleaning remaining components; failed
assertions retain the original component-check diagnostic and native safety trap rather than
requiring an impossible callable or running recoverable cleanup.
`received-nested-outcome-control.nct` qualifies two optional layers through take/read/edit
inputs. Outer absence and present-inner-absence share one canonical received representation, but
their original outer tag test remains intact. Only the inner impossible callable continuation is
excluded; the inhabited counterpart keeps its branch. Required/native execution distinguishes both
absence layers and the live callback, including native environments owning error graphs.

`received-outcome-recovery-merge.nct` qualifies a recovery result that rejoins normal execution
with the same received callable witness. A homogeneous optional-callable array supplies both
positions; distinct captured numbers select observably different payloads on the present and
fallback paths. Required/native execution preserves partial transfers and remaining-owner
cleanup, including native environments owning errors. This does not admit joining unrelated
formal witnesses or construct a homogeneous callable array from unmatched element contexts.

Each original invocation input retains its positive alternatives and a separate inactive-callable
fact in one `InputProducers` summary. Matching compound input paths propagate a constructed
inactive outcome to its callable descendants. Joins require every observed source to exclude the
leaf; an unobserved accumulator is distinct from a described unknown with no positive candidates.
The original call recipe remains shared by all result leaves. Descriptor-only applications retain
unknowns rather than inventing constructor facts. Positive result solving does not consume the
exclusion as a witness. A separate proof over the original return/application equations may use it
to exclude an active callable; it does not prove an empty physical representation.

Symbolic result scope admits a callable parameter owned by the current declaration or retained
by the current closure's declared environment dependencies. Capture scope preserves the original
parameter identity and may weaken its invocation view; an uncaptured sibling, foreign parameter or
stronger view cannot enter through a matching signature. This scope check supplies neither an
actual environment nor an initialized value: those still require the original per-instance inputs.

`results::applications` classifies each selected call result once. A newly reserved callable result
application must match its caller/call key, the selected callee's result template and declared result
invocation. A callable result of a named, anonymous or returned callee gets its own application;
a structural carrier transports its existing leaf relations. It cannot borrow an unrelated
application. Committed calls propagate actual returned callable leaves and symbolic references
through the same sparse producer facts. Missing source applications remain empty premises.
`results::relations` owns one finite positive-equation solver for unions and selected applications.
Return-summary terms pair the named or anonymous code identity with a callable-result leaf ordinal;
application terms pair that ordinal with the original whole call. Applications consume the actual
callee's matching leaf summary and frozen inputs, never a first-result fallback. A callee returned
by another application resolves through those same equations. Result templates never supply return
producers. Recursive substitution uses formal IDs and finite application nodes rather than
expanding witness names. Unanchored recursion and
heterogeneous return relations reject.
Original staged return facts select each declared callable component through the declaration-owned
result path. Independent component equations share the call's original invocation recipe; they
retain empty producer sets and recursive leaf relations without expanding homogeneous arrays.
An empty array's element descriptor does not establish an element return producer. Source result
publication, original produced facts and concrete payload realization remain separate obligations.
The established summaries then instantiate every typed call application through its frozen
actual bindings, retaining known types even on interruption. A missing producer for an application
without a reachable commit closes as explicit Unproduced evidence, preserving the structural
contract under the existing application identity. A reachable application still requires a witness.
Prepared value flow establishes return anchors; declared argument types alone cannot do so.

The exact accepted Unit retains the original reachable source-flow equations as `ProducerTerm`
and `ProducerRelation`, before code/type projection reduces their selected witnesses. Return nodes
keep their original unions. `InvocationRecipe` owns each reached committed call's original
callee alternatives and formal-to-producer alternatives, including nested application terms,
independently of whether its result is callable. Value-call receivers use the prepared value's
source-flow producers, including reassignment and control alternatives, adapted to the frozen
invocation view; the type's original witness does not replace those facts.
Producer flow retains aggregate field paths in one analysis-local, structurally interned fact
graph. Equal callable leaf types do not merge different tuple/record component occurrences.
Direct field replacement updates that component; control joins preserve alternatives at each
component, and evaluated writes remain conservative. Arrays retain one element-summary shape,
including its aggregate fields, independently of element count. Shared subshapes and iterative
traversal avoid expanding the logical value tree. These facts describe possible producer terms,
not numeric-index equality, path correlation, access permission or initialized storage.
Lexical read/edit aliases use the existing `flow::aliases` provenance owner to read and update
the current underlying local or bounded temporary's facts, rather than keeping an independent producer snapshot.
Producer paths erase numeric selectors; array writes remain conservative summaries even when
only one lexical path is found. This traversal grants no saved-address or bounds proof.

Closure creation snapshots retain that same structural graph after its fixed point; the
construction index is discarded and creations in one body share the immutable nodes.
`Unit::creation_recipe` pairs each checking-reachable formation with its exact lexical code,
expression and original environment schema. Later updates cannot rewrite an earlier snapshot;
repeated formation joins possible inputs rather than selecting one iteration. Dead formation
creates no record. The original schema remains in the same canonical type authority even when
result projection normalizes the closure's environment. Component queries return producer sets,
not path validity or bounds proofs. Complete-Unit rebinding verifies source bodies before exposing
these records; equal header types do not permit reuse after changed capture effects. Concrete
captured-frame realization and occurrence-sensitive executable data remain separate obligations.

`Unit::value_sources` retains a sparse source value-definition graph for each reached body,
after canonical type projection and receiving-action resolution. Those stages preserve the original
control/action identities and value relationships.
Definitions refer to earlier values or incoming merge nodes; a later local update cannot rewrite a
previous formation or prepared argument. One incoming node per control point and value slot links
to predecessor definitions, including backedges, without unrolling iterations. Source slots share
their identity vocabulary with the producer analysis. Both consume the same selected control graph
and lexical alias authority; this graph does not repeat typing, ownership or initialization checks.

`ValueSources::flow` retains sparse writes and actual operand reads at original actions.
`ValueSourceBoundary` counts completed source actions in one control point; it is not a resource
phase. Reaching definitions and original predecessor exits share one selection authority with the
SSA merge builder, replacing its separate block-output state table. `flow.operand` preserves the
actual selected read, including inserted receiving adapters and alias-root selection; an alias's
stored binding is not a later read of its referent. Arithmetic operand preparation, condition tests,
discarded values and compound-assignment inputs retain their own occurrence reads. A later operand
write cannot change the earlier scalar preparation's source. Staged returns retain their own source slot.
Definitions survive retirement as metadata and do not make retired owners accessible.
`flow.completions` selects reached original normal-return points after their complete action
sequences. Early and ordinary returns use those same sparse boundaries; a diverging path creates
none. These endpoints can select final input definitions without substituting argument-preparation
snapshots. They prove no actual completion, cleanup execution, ownership or resource phase.
`flow.channels` is a derived view of the original typed input order, including its environment
prefix. Each `ValueSourceInput` retains only the corresponding entry source and input operation;
the source node owns its local, role and schema. Static/global entry definitions are excluded.
Construction uses the typed input authority directly rather than reconstructing order from names,
arity or a sorted local map. This view grants no argument value or new input/access receipt.
The occurrence iterators expose these reads and the final write per slot in each original action;
multiple writes within a commit do not become new source boundaries or resource phases.
`flow.referent` retains the value selected by an assignment target before its write, through the
same lexical alias authority. A repeated alias write therefore refers to the current source root,
not the value at alias formation. An uninitialized target's metadata grants no previous payload.
`flow.root` retains each selected referent before its authored place-field suffix. Lexical alias
prefixes remain part of that root; an evaluated referent is already a complete selected root. The
suffix must match the common resolved path before it is separated. Root occurrences are sparse
original-action entries and follow the current source through writes, not an alias binding's old
snapshot. They identify reference-root metadata, not the enclosing physical allocation, initialized
storage, saved numbers or a resource phase. Changing-layout proofs remain a separate obligation.
`flow.root_carriers` retains each original containing source slot, its current source and the path
selecting the virtual root. `flow.root_carriers_after` retains the separately selected completion
state. Both are collected by the same lexical selector as the virtual root; alternative sources
stay paired with their paths. An external input can itself refer into a larger allocation, so
these selections do not certify physical backing. Erased array indices establish no numeric
address or bounds.
`Unit::instantiate_referent_carrier_data` resolves the recorded before/completion selections on
demand, retaining each original slot/source/path with its complete contextual data key. Source
slots have one typed-schema projection shared by source writes, ordinary definitions and these
queries. Each selected contract spends the shared metadata budget, including independently closed
schemas. An absent occurrence fails before resolving any contract; failure publishes no partial
pair and a new request can retry. Ordinary leaf metadata does not eagerly require every containing
context to close. Proving compatible placement remains a separate consumer obligation.
Argument preparation records its original selected roots; committed read/edit address bindings
record their current roots before call writes. This does not replace the prepared argument values.
Call writes retain whether the lexical target is ambiguous or selects an erased array element.
Those weak writes cannot replace the previous context with one selected element's new context.
Tag observations record their referent roots while retaining opaque Boolean value semantics.
`flow.root_after` separately records roots on normal completion of an original assignment or call.
Call roots include read inputs: a separate edit projection can change their containing carrier
without writing the read projection. Both sides use the same lexical selector and source state;
the original before-root is retained. These are source contracts, not evidence that a call returns,
that the selected location was modified or that a resource phase has executed.
`Unit::instantiate_referent_root_data` validates that original occurrence before selecting either
independently closed root-contract data or its actual source key. A place root uses the local's
complete schema; an evaluated root uses the selected referent's schema.
`Unit::instantiate_referent_root_data_after` selects the separately recorded normal-completion
source through that same schema and data resolver. A preparation entry cannot stand in for a
completion entry. `ValueData` freezes both contracts as one occurrence pair, separately from
produced payload receipts under the shared metadata quota. Missing sites,
ambiguous context and exhaustion supply no partial result. Contract metadata does not assert that
the root is initialized, or describe old/new resource phases. Native storage interns both keys;
source-place address projection uses the original before-root. Native scoped replacement retains
the matched pair for virtual-root selection. Evaluator planning retains both virtual roots and
their enclosing carrier contracts as constraints at the original source entry and completion
boundaries. Reference-view registration remains strict; a binding-time snapshot cannot authorize
a changed referent. These constraints allocate metadata only, not initialized storage or stable
enclosing allocation geometry.
Guarded carrier views likewise retain their actual referent source before payload selection. A tag
assertion does not replace the carrier with its payload, choose an alternative or prove availability.

`Unit::instantiate_stored_data` resolves a selected source slot/component at one such boundary
through the existing budgeted fixed point. Backedges remain sparse dependencies. Different carrier
contexts at a join remain unresolved while an independently identical component can still close.
Invalid boundaries, absent inputs, ambiguity and work exhaustion publish no partial data. This
boundary describes source definitions; pairing old cleanup, receipt and post-call updates with the
existing ordered resource transitions remains required before lifecycle admission can widen.

`results::channels` owns body input/result metadata selection for both evaluator contracts and
target-program ABI projection. `Unit::instantiate_input_data` validates the original input local
before using either an independently closed schema or its actual entry source, including the full
capture input. `Unit::instantiate_result_contract` selects independently closed result metadata or
the actual return producers. This does not assert that a diverging body returns; the separate
`instantiate_result_data` query still requires those producers. The evaluator's contracts, input and
return slots, and plan result use this authority under the shared plan-work quota. Exhaustion during
initial contract preparation retains the ordinary plan-work diagnostic rather than becoming an
integrity failure.

`Unit::instantiate_completed_input_data` selects an original input's final metadata through every
retained normal-completion boundary in the same budgeted fixed point as stored values. Input roles
and schemas share validation with the entry query. Different final contexts remain unresolved;
there is no first-return, entry-frame or preparation-snapshot fallback. A body with no normal
completion supplies no final-input key, even for an independently closed schema. Failed work
publishes no partial result and can retry. Internally, known absence of normal completion contributes
an empty fact, separately from an unavailable context. Source identities can survive retirement as metadata,
so this grants no initialized input, caller write, actual return, const permission or resource phase.
Caller writes consume those contexts only through the qualified correspondence described below.

`Unit::instantiate_value_data_map` owns ordinary operand and stored-definition metadata. It reads
the original source graph's operand selections and final local/expression writes at each reachable
action, using typed schemas and the shared source-value query under one metadata quota. Intermediate
writes within one action do not become new phases. Unproduced contracts supply no payload. Failure
identifies the original action and publishes no partial map; a fresh request can retry. The read-only
One batch retains the source-value fixed point across its operand, definition and root queries;
prepared-argument batches do the same. The immutable program/evidence and complete frame/source/
path/type keys bound reuse, so equal signatures cannot merge distinct occurrences or environments.
Every request charges work, including a solved hit. An interrupted solve discards its draft facts
and pending work before retry; no partial map or executable permission escapes.
`ValueData` retains actual instantiated payload contracts without adding owners or granting source
access, materialization or executable-body permission. Target-program occurrence projection consumes
this map directly. Evaluator preparation registers its local/expression definitions alongside the
ordered resource metadata through the same slot interner. One slot must have a coherent represented
shape; conflicting contexts reject rather than selecting the first. Residual independently closed
metadata needs no occurrence binding and cannot fall back to frame-wide type substitution. Metadata
preparation spends plan work and allocates no runtime cells or owners.

`Unit::instantiate_prepared_data` freezes preparation-time data from the original argument-slot
definitions under one metadata quota. The typed call owns each ordinal and schema. Equal repeated
preparations share a key; incompatible contexts reject without publishing a partial map. The
read-only result selects only original preparation actions, even when the argument slot also has a
post-call definition, and includes successfully prepared prefixes whose invocation never commits.
Evaluator prepared/temporary slots and target-program payload projection consume these same keys, preserving
source effects and adapter timing. The metadata creates no callee, value, access or materialization
permission. Materialized edit inputs have separate post-call argument definitions, while their
frozen call bundles and preparation contracts retain the entry sources. General dependent carrier
completion and changing evaluator slot shapes remain separate qualification boundaries; an
immutable preparation snapshot does not describe a changed environment.

Evaluator projection memoizes each committed call's actual source frame within one body plan.
Receiver-prefix handling, owning input data and call-result slots consume that same frame; a
frame-wide type binding cannot reselect the callee. Code instances are still interned only when
lowering an actual committed call. The original body input layout owns order and explicit
operations. A taking callable adapter may retain its environment while lending it to a read/edit
body; this does not change the explicit inputs' operations. Interrupted preparation creates no
call frame or invocation effect. The shared `const-callable-inputs.nct` corpus includes a forwarded
taking view over a reading, non-Copy captured environment and reuse of named and captured reading
callables after forwarding them as edit inputs, in required and native execution.

Input nodes retain their local role, including the complete environment input. Selections retain
component paths rather than treating a received closure as fresh creation. Formation records retain
their actual environment source. Committed calls retain complete argument bundles in source order.
Materialized inputs preserve their preparation sources; reserved read/edit references select the
actual referent at commit, independently of their earlier preparation-slot definitions. Multiple
bundles do not form a Cartesian product. Definition and bundle origins name the original
action. Aggregate fields and one repetition seed retain shared references independently of array
cardinality. Numeric selectors in alias/update paths are erased, and array replacements remain weak;
these paths prove neither equal indices, bounds nor saved-address identity.

`ValueSource::Transport` retains a distinct receiving node and its original supplied source.
Representation realization first closes that source against its own schema, then selects the
destination domain independently or from the original receiving value's consumers in the same
canonical frame. `Evidence::requires_transport` classifies the symbolic lifecycle operation;
concrete realization still checks directed compatibility against the
actual receiving instance. Consumer constraints retain their original producing paths and budgeted
fixed point; absence cannot choose a code/environment context. Callable-value
queries continue through the supplied source; an excluded alternative contributes no positive
function witness. The destination constraint never replaces the original callee's result schema.

Scalar operations are opaque to this metadata graph. A call's selected edit input records a possible
post-call write, distinguishing existing storage from an owned temporary; it does not assert an
unchanged context or infer the callee's effects. Nodes retain canonical projected schemas and are scoped to
the exact complete Unit and body. Neither graph IDs nor detached graph copies are reusable source
receipts. Transparent receiving adapters inserted after graph construction retain their source
value even when they have no independent Evaluate action. Frozen structural construction adapters
retain their actual component sources under the same rule. They cannot create an empty incoming
slot in place of the selected prepared value. Opaque numeric updates retain the typed target's
result schema; metadata opacity does not erase that type or execute its numeric operation.

`Unit::invocation_recipe` exposes each committed-call record
under their exact code/call identity. Callable application nodes share the same immutable record,
so the equation graph and invocation catalog do not maintain separate binding copies. Interrupted
or unreachable commits create no invocation record. The possible producer sets select neither
path correlations nor a joint concrete frame. The selected code summary remains a separate result of
that graph, not a replacement creation recipe. `Unit::producer_relation` exposes only immutable
metadata from that Unit. Rebinding the complete Unit verifies every accepted body before exposing
current-source views; a detached typed-body rebind does not expose this graph as a new source
receipt. These relations do not establish a concrete captured frame, runtime value, layout,
callable target, const permission or ownership. The separately qualified ordinary-source boundary
is described under [context-bound callable captures](#context-bound-callable-captures).

`Unit::instantiate_invocation` resolves a concrete metadata frame under a caller-supplied
deterministic `WorkBudget`. For matched source bodies it consumes the original sparse value graph:
complete committed bundles select the callee, its actual callable inputs and independently received
compound contexts. The declaration authority's `compound_input_contracts` selects which exact data
keys the frame requires. Every bundle must supply them before a frame is published; differing
compound contexts distinguish otherwise equal code, environment and direct callable bindings.
Legacy code-only recipes cannot invent those keys. Body entry and normal completion share the
retained original explicit-channel correspondence, excluding the validated environment prefix.
Entry component queries select the frame's own input key, not a raw-type default or an unrelated
captured occurrence. Missing/duplicate correspondence or a mismatched schema remains unresolved.
This metadata integration alone does not authorize source construction or qualify storage.
Return edges
retain selected component paths; received environment inputs project their complete canonical data
key rather than reopening lexical capture names. `Unit::instantiate_value` exposes callable metadata
for one value-definition ID in that exact frame. One request-local worklist joins value, data,
witness and frame facts to a fixed point, retaining contexts through definitions, selections,
formation and application/result edges. Opaque contextual effects and unqualified call writes
remain unresolved. A declaration-closed callable schema can identify context-free metadata, but
never proves that a value exists or can be accessed.

For a value-receiver update with dependent context, the metadata solver preserves the actual callable's code while
selecting its completed environment from that exact invocation frame. The unique original capture
input and its schema must match the closure; missing bodies or unresolved completion contexts
cannot fall back to the old environment. Declaration-closed witnesses fix their static dependencies
without asserting that runtime payloads are absent or unchanged. Disjoint component paths retain
the previous source, and containing data is rebuilt through the common component normalizer.
Weak updates join old and completed contexts. Explicit edited inputs select their final structural
data and callable components through the same completed-input query. A selection within the
written carrier follows its relative component path into the selected input; whole carriers retain
their exact source schema and canonical component contexts. Function selections use the resulting
concrete witness without discarding its environment. The original call kind distinguishes the frozen
value-call prefix; the selected body's retained source-channel order then supplies its exact input
local, operation and schema, including a closure's environment prefix. Only an actual edit input
can supply this write. Missing correspondence, mixed call kinds or differing final contexts remain
unresolved, without a caller-entry or preparation-snapshot fallback. Weak writes join only the
selected old and new components; equal siblings can resolve even when the whole carrier cannot.
Logical carrier/replacement tests do not grant source assignment or materialization permission.
General dependent source input carriers, context-changing post-call temporary transport, varying
evaluator slots and context-changing source captures remain unqualified.

`Unit::instantiate_value_data` normalizes one original value against its receiving source schema
through that same query and declaration authority. Returned carriers retain their selected callee
frame and component paths. Aggregate replacement rebuilds only the affected canonical component
keys; prior snapshots remain unchanged. Erased multi-element array writes join old and supplied
contexts rather than asserting a selected element. A direct assignment through one original path
can replace the selected context when every erased array step has exactly one element: normal
completion identifies that sole coordinate without claiming a numeric value or bounds proof.
Call-write summaries retain their independent conservative rules. Empty sources and unproduced applications publish no data key,
even if their declared scalar schema is independently closed. The key selects contextual metadata,
not initialized storage, source access, runtime layout or cleanup. Ordinary body payload and
lifecycle consumers must retain those exact selections within the qualified source scope.

The code-summary/creation-recipe resolver remains available for original metadata fixtures and
definition-contract reasoning without a matched value graph. Failure of an actual source-value
query cannot fall back to that hypothesis. A matched result-code summary may close directly only when the declaration authority proves
that concrete witness context-free; dependent environments cannot reuse the outer caller's binding
merely because the source code agrees. Invocation adaptation delegates to
`Program::adapt_witness`, preserving the complete capture context and rejecting view upgrades.

A source formation assembles its complete environment from actual value sources through
`Program::instantiate_function` and `Program::instantiate_components`, then binds that data key
directly to the selected closure. Tuple/record positions retain distinct contexts even when their
source types agree. Homogeneous array metadata requires all represented element contexts to agree;
a repetition retains one seed and an empty array supplies no element context. Numeric selector
paths provide no number, bounds or initialized-storage proof.

In code-summary reasoning, a dependent direct closure term requires its retained creation record
in that exact creator frame. The resolver pairs original and normalized environment schemas with the frozen producer
shape, visiting shared schema/shape pairs once and charging traversed components to the same
metadata quota. Actual function leaves retain complete occurrence contexts in the declaration-owned
component DAG; carrier assembly preserves different contexts at two same-source-type positions.
The completed environment binds directly to the selected closure without flattening its free
parameter names. An alternative producer set at one position must independently resolve to one
witness. Missing creation records, foreign creator frames, incompatible views and alternative
contexts remain unresolved rather than borrowing current outer formal names.
This closes metadata substitution only; per-value/component executable representation and
additional source admission still require their separate qualification.

Different concrete alternatives remain unresolved rather than selecting the first witness or
enumerating a Cartesian set of joint frames. Empty facts supply no success receipt. Anchored
recursive forwarding can close through the same positive fixed point without host-recursive
resolution. Work failure drops the request-local draft and cannot publish partial state or poison
a retry. Foreign owners and absent committed calls reject before resolution. The returned key is
metadata only: it supplies no runtime value, initialized owner, const permission, body plan,
representation or executable admission. Source creation and occurrence-sensitive transport require
their separate consumer qualification; logical context tests alone do not widen that source domain.

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
initialization over the same graph. Schema cleanup projection rejects unguarded result/application
roots; outcome-guarded obligations still require concrete or explicitly excluded component data
when consumers specialize them. These obligations supply no positive function value. Separately,
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

Tuple and array formation can also retain valid callable-formal descriptors. Each component still passes
ordinary value supply; an unmarked formal cannot acquire Copy and transferred inputs cannot be
reused. After result closure, pending producers remain rejected. Concrete component keys close
under the existing source-value and resource-data queries, preserving distinct environments at
same-typed tuple positions. An array requires one actual homogeneous element context; a formal
descriptor alone does not prove agreement among different elements. Repetition above one still
requires a source Copy proof, and selection from a temporary array retains its Copy requirement.
If required analysis cannot establish one concrete context, it reports E0323 at the original
source operation; an unresolved query is not an internal integrity failure or a choice of the
first element. Owner mismatches and missing committed-call identities remain integrity failures.
`contextual-tuple-results.nct` returns such a carrier from an anonymous body, then invokes its
selected components with different named/owned environments. Required evaluation and native
execution must both produce 42; a callee's formal descriptor cannot replace the actual result key.
`contextual-callable-arrays.nct` qualifies named and owned callable elements through taking
inputs, single-element repetition and invocation in both consumers. Zero-count repetition still
evaluates and destroys its seed, without inventing an element or a callback invocation obligation.
The separate `contextual-callable-array-errors.nct` native case constructs actual owned error
graphs and disposes them through element cleanup and zero-count seed cleanup.

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
`resources::Step<P>` owns one operation vocabulary and an exhaustive payload transform for both
checked and instantiated obligations; projection preserves selected operations and places.
`Step::try_map_payloads_at` supplies each obligation's `PayloadSite`: its selected source place or
new destination holder. Capture and invocation-view adapters retain separate environment/source
and callable/destination roles. Scoped replacement carries a retired obligation at the existing
destination and a supplied obligation at its source. Equal source schemas do not identify those
two actual values; constant cleanup requirements consume the retired obligation.
`Step::Transport` retains separate source and destination contracts for represented-value
transport. The shared reclassification kernel consumes one existing owner, checks the recorded
identities and contracts, then publishes its destination and step atomically. Function invocation
adapters reuse that same ownership transition. Independently closed taking conversions use this
record through original value definitions, ordered resource-data projection, evaluator effects and
MIR/native lowering. Destination selection validates the actual supplied instance against that
closed domain; it does not reconstruct captured contexts from an ambient frame. Occurrence-bound
destinations remain outside this qualification and reject before ownership consumes the graph.
Array transfers identify the original retained element, not the
whole array. The type-only convenience transform delegates to this same exhaustive operation
mapping. A site grants no availability or bounds proof and does not choose an operation phase.
Consumers must pair it with the retained transition order: replacing a local does not make its old
cleanup refer to the new value. Source/phase provenance and occurrence-specific lifecycle transport
remain separate integration obligations.
`Unit::project_resource_data` owns ordered payload specialization under one concrete frame and
original action. Its transition-local binding map contains data references for intermediate
materializations and transfers, not owners or new world identities. Old cleanup precedes new
binding; scoped retirement uses the original write target's actual alias-root read.
Consumers that need containing representations request `steps_with_roots` and
`after_steps_with_roots`. Each sealed `ResourceDataStep` pairs its unchanged operation with the
dependent roots of partial sources selected before that step. Aggregate source components follow
the same rule. Independently closed roots need no additional occurrence receipt. Selection uses
the same transition-local bindings and original stored-data queries as payload specialization;
it grants no owner, initialized backing, numeric index proof or stable geometry. Failure rolls
back that projection's bindings and publishes no prefix, while retaining spent metadata work.
For a dependent partial owning destination, the original assignment target must match exactly.
The matched operation must occur in that original action's checked resource report, retaining
ownership's frozen numeric projection without replaying the analysis or guessing from syntax.
Its `PartialRootContracts` retain the original whole-container before/after keys with that
resource step. They neither fuse old disposal with new receipt nor choose another source phase.
Evaluator projection registers both contexts and emits a typed component receipt plus exact
constraints; the kernel preserves the types of all unselected components. Native projection
retains the same contracts for placement and independent physical geometry validation.
`ResourceDataProjection::replacement_target` matches a scoped destination to the original update
referent, including its alias-relative fields. A different operation, owned local or destination
cannot select that reference-root pair. This metadata match grants no owner or layout proof.
`replacement_carriers` resolves that matched destination's containing contracts under the same
request budget and requires its normal-completion side. Native lifecycle retains the pair;
evaluator planning registers both sides' original containing holders alongside their virtual roots.
All use the ordinary slot/shape interners. Source-entry constraints precede every before-effect;
source-completion constraints follow every after-effect. These boundaries enclose resource phases
rather than being inferred from step names. Incompatible reference-view shapes reject without
choosing the earlier alias snapshot. This supplies no physical geometry or ownership permission.
Capture, function-view adaptation and component assembly use the declaration-owned canonical operations;
array element keys must agree without selecting an arbitrary first context. Independently closed
schemas select metadata without consulting a backend's frame-wide type cache. Original transitions
still own before/after order, availability and guards. Both evaluator preparation and native
lifecycle pass their checked post-operation steps through `ResourceDataProjection::after_steps`.
At an original committed call this invalidates entry bindings only for edited temporary inputs;
later cleanup selects their post-call argument or owned-expression sources. Interrupted preparation
and read inputs keep their entry metadata. No operation phase is inferred from a cleanup step's
name. A failed step list, including post-operation selection, restores prior data
bindings and publishes no partial list, while spent metadata work remains spent. This projection
grants no source, layout or executable receipt. Native lifecycle schedules and evaluator whole-slot
metadata consume this authority. Evaluator preparation registers selected whole roots, assembled
field sources and retained element views before lowering control points, so block allocation order
cannot choose a frame-wide type default for those slots. It allocates metadata, never cells, values
or owners, and reuses the ordinary slot interner. Canonical input contracts establish ABI-entry
slot data before this registration. An owning local can register different selected representation
contexts without changing its initial data, storage role or logical slot. This registration is not
a representation transition. Aliases, expressions, return and auxiliary holders still require a
single matching context. Previously registered original view definitions retain their exact
metadata rather than being inferred again from the backing slot's initial data.
Preparation retains the complete instantiated
before/after lists for each original action and transition through effect emission, rather than
discarding them after registering types. Original steps still own declaration/view checks; their
payload-erased operation identities must match the retained concrete steps. Concrete source
contexts are constrained before their effects, destination contexts after receipt, and replacements
retain distinct retired/supplied constraints. Whole owning-root moves emit the evaluator kernel's
contextual receipt with the actual supplied data, preserving original retirement order. These
constraints do not create values, access or owners. Publication rejects a mismatched constraint
without retaining part of the plan. General changing-holder registration, partial-carrier execution
and the enclosing source-admission boundary remain unqualified.
Place, boundary, guard, component and index
views select their metadata from the represented source slot; read/edit local bindings and forwarded
views preserve that selection. Physical component lookup has one evaluator authority and does not
establish active variants, initialized payloads or valid runtime index numbers. Empty arrays provide
no element view. General changing partial-carrier roots, context-changing slot geometry and post-update
dependent environments remain separate obligations.
`resources::Payload::instantiate` binds an existing logical schema to the declaration-owned
`DataInstanceKey`, preserving the actual callable representation and nested captured context.
`InstantiatedPayload` retains that complete key with its concrete root cleanup obligation. Its
environment projection follows the selected value's own context, rather than a caller-wide map of
raw source type IDs. `InstantiatedPayload::with_environment` rebuilds that actual callable through
the same canonical witness and payload authorities, preserving code and invocation view while
selecting a matched new environment. The old receipt remains unchanged; non-callables, code tokens
and incompatible environment schemas reject. These operations grant no source admission, initialized owner, Copy,
external-dependency independence or stored representation. The accepted Unit's `DataEvidence<P>`
uses the same payload-or-unproduced distinction at both projection stages. A dead access holder is not automatic early resource destruction. A staged
return survives evaluation/lexical cleanup, whose order is reverse actual construction/preparation. A normal return has no evaluation frames and exactly one
Return owner with the checking-closed result data type, including unit. Trapping/diverging paths do
not acquire unwind guarantees.

`admission` validates known body result/input/local/expression data before a typed query freezes,
including callable constraints and capture environments. Array-bearing operations follow the
[construction](#array-construction), [selection](#array-selection) and
[resource](#concrete-array-resource-frontiers) owners below. A declaration schema alone does not
qualify an executable operation; zero count does not substitute tuple or scalar behavior.

Source-admitted payload evidence consumes the declaration authority's exact Boolean/integer/unit,
program-extent text and opaque owned-error
snapshot contracts, plus qualified optional/array/record/tuple compositions and exact
capture-free named-function witnesses. Scalars/unit/named functions have trivial disposal; error owns graph cleanup; aggregates
retain their own schema cleanup rather than becoming scalars.
Empty external D/S for those aggregates follows the exact component contracts, not Copy, ownership,
or a missing outside R. Named-function evidence follows its exact declaration schema; a formal callback does not inherit
Copy, trivial cleanup or empty D/S. Owning formals retain a symbolic environment cleanup operation
tied to their witness. The producer domain also admits the bounded owned-environment schema described below;
general retained footprints and concrete executable witness specialization remain separate obligations. Dependent aggregates,
user destructors, capabilities, general captured environments, nested callback contracts, generics,
general error-data construction, async and regions require their own later schemas/validation.

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

Unit closure reconciles normalized source headers with actual required values before producing
the accepted product. Supplying a different count through the internal metadata API cannot certify
a Unit, even when the body happens to match that substituted array type. This covers data and
callback input/result contracts, fields and every physical occurrence in a retained module binding.
The source/type reconciliation uses the [declaration source owner](../nocter-declaration-lowering/README.md#exact-source-evidence);
it adds no evaluator, type writer or alternate accepted-body path.

The whole-unit constructor validates every definition, including unused/recursive callees, and
publishes no partial accepted value after any typing/init/access/resource failure. It owns the
same shared declaration facts, canonical type authority, closed result evidence and all body/flow
reports. It rejects an incomplete ready-header view before evaluation or acceptance. Admission
also proves exact source-body, checked bodyless-primitive and required-initializer-value coverage; a coordinator's
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

## Primitive Header Boundary

The ordinary value-access source entry has no primitive authority. It rejects a matching
bodyless declaration name rather than treating it as an intrinsic. Explicit authorized source
preparation uses the same staged contract, body and required-value coordinator with checked
[header bindings](../nocter-declaration-lowering/README.md#trusted-primitive-headers).
The complete Unit retains those immutable bindings: every source definition has a checked body,
while every primitive has an exact checked role and no body. Neither a fabricated empty body nor
an omitted ordinary definition satisfies coverage. Binding still requires the original header
authority. Runtime primitive calls use ordinary input/result and cleanup checks; required closures
reject them through the ordinary runtime-call diagnostic rather than requesting absent source.
Native constructor execution is qualified separately by the
[Machine consumer](../nocter-machine/README.md#private-runtime-error-construction).
Caller-write summaries consume only separately qualified runtime evidence through the
[closed effect catalog](#closed-caller-write-effects).

## Program-Extent Text

Plain string expressions use syntax's shared escape/multiline decoder, then enter ordinary
construction as `FrozenNode::Scalar(ConstantValue::Text)`. The admitted `str` payload describes
immutable program-extent characters, with no outside data/storage dependency or heap cleanup.
It grants no Copy evidence: existing-value supply still needs an explicit transfer, while read/edit
inputs, descriptor replacement, records/arrays, captures, constants and immutable statics use their
ordinary protocols. Neither byte indexing nor source text comparison follows from this admission.
Required evaluation qualifies exact UTF-8/embedded-zero decoding and frozen publication. Native
source qualifies descriptor storage and input/result transport; the machine data tests qualify
backing bytes and relocations. General string construction, error-accessor dependencies,
interpolation and trusted primitive consumers remain separate obligations.

## Outcome Values

`outcomes` owns contextual `none` and presence construction, preserving exact data forwarding
before adding optional/success layers. Payload readiness and Copy use the declaration authority;
fallible carriers remain non-Copy even with Copy success data. The current tag does not grant Copy.
Whole-value postfix `!` supplies
the carrier first, checks presence, then selects its payload through ordinary Copy/transfer and
remaining-container cleanup. Empty completion and bare return supply the ordinary unit value
through that same result boundary, including present optional and success layers. A conditional without else
first supplies unit, then applies its enclosing value context; it cannot return branch absence.
Value postfix `?` expands into ordinary binding, a stored-tag condition and a conditional whose
absence branch returns the declared outer optional absence. The supplied carrier has one local
owner and the selected answer is protected before its lexical cleanup. Existing return flow
cancels enclosing prepared calls and retires scopes; no separate propagation cleanup protocol is
introduced. A closure uses its own result contract. Initializers and non-optional callable results
cannot propagate absence. An enclosing success layer is preserved around propagated absence.
Fallible value propagation transfers the alternate error into the callable's unique failure layer;
outer optional layers are constructed present. `throw` uses that same ordinary result/return
path with an owned error, while ordinary return of error data selects success. No exception edge,
implicit error copy or failure-to-absence conversion is introduced. Source checking qualifies
these ownership/branch decisions. Actual runtime failure construction, propagation and owned
recovery execute through the [Machine consumer](../nocter-machine/README.md#private-runtime-error-construction);
required evaluation still has no owned-error graph domain.
Direct read/edit operands and scoped bindings request an existing payload
place for postfix `!` or `?`. `Scoped` forms its source once and forwards the place selected by
its projection/control. Assertion and propagation retain the carrier, with the payload selector
outside that receipt. `GuardView` checks the tag
and retains its stability. Ordinary selection, argument reservation and alias binding then use the
original payload storage. Scoped fallible assertion preserves the carrier; scoped fallible
propagation is rejected because it cannot implicitly extract an owned error. Scoped optional
propagation returns absence through the same callable return
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
owner table or physical payload move.

`outcomes::recovery` owns both local recovery clauses through that same carrier, branch and
scope protocol. A value `catch` selects an owned error into a fixed handler local; ordinary
result forwarding protects its answer before handler cleanup. Owning `catch _` discards the
received error before the fallback starts. Scoped `catch` binds a read/edit alias into the
existing alternate payload and retains its selected tag; scoped `catch _` does not destroy that
payload. Normal scoped fallbacks must designate compatible surviving places or caller-owned
bounded temporaries. A handler alias can forward the original carrier's error, but cannot grant
ownership, escape handler-owned backing or authorize whole-carrier replacement. Guards carry
their selected variant explicitly through the shared checking/MIR/machine path.

Implicit optional/success input wrappers retain the original direct-transfer reservation. Complete bundle
validation precedes wrapper materialization and original-source retirement; later reads and
cancellation therefore observe the original owner. Explicit nested value constructions keep their
ordinary preparation timing. No wrapper changes the input operation or repairs a direct edit-place
mismatch. Shared required/native source covers nested absence/presence, void and narrow payloads,
local/static/container storage, captured calls, read/edit/take handoffs and indexed transfers.
The fallible source fixture additionally qualifies native success construction, exact forwarding,
owned propagation, unit completion, scoped read/update, and deferred/cancelled implicit success
input transfer. Recovery source additionally qualifies success-branch laziness, original-storage
update and nested recovery order. Checking covers owned and scoped failure handlers, error-data
forwarding, rethrow and suppression obligations. The [constant domain](#constant-domain-acceptance)
and [native owned-error effects](../nocter-machine/README.md#owned-error-effects) define their
respective executed scope; inactive error alternatives alone establish no live-graph cleanup.

## Constant-Domain Acceptance

Private `const func` declarations retain the same body, typed operations, selected callees and
resource ledger as ordinary functions. The authored header capability alone is not body evidence;
typed-only queries remain incomplete. Whole-unit acceptance additionally checks every const
body's operation domain, including unused definitions, dead branches and interrupted call inputs.
Direct and exact named-value calls require the selected declaration's authored const capability. Recursive const
headers close without executing bodies or proving termination; every definition must independently
pass ordinary typing, initialization, access and resource validation.

`value_access::constant` owns this bounded domain check. It admits primitive Boolean/integer/unit
operations, optional construction/assertion, eligible fallible success values and finite record/tuple/array payload compositions of supported data, local values, read/edit aliases,
updates, structured control, explicit transfer, exact named values and eligible owned captured calls. Aggregate admission is independent of Copy and
memoized by data identity rather than expanded through shared fields. `evaluation::types` is the
single shape projection used by const-domain validation, final-data admission and closed-plan
construction; homogeneous arrays retain an element ID and u64 count, never expanded type lists.
Raw descriptor shapes and closed `DataInstanceKey` shapes have distinct cache keys in that one
projection. `get_in` describes the declaration authority's frame-bound child/environment selection;
it is not an occurrence lookup. Call-input domain checks use `supports_input` with the exact
received compound key at its explicit ordinal. A missing compound context cannot be filled from
same-typed captures; direct callable inputs retain their own witness bindings, and closed inputs
use their raw schema. The supported/unsupported decision follows the supplied input independently
of the surrounding environment. Schema disagreement rejects rather than selecting another shape.
A representable optional absence does not supply a missing contextual-input receipt.
The selected function leaf uses its actual code/environment representation so a supplied value and
its receiving formal agree on the evaluator type. Carrier identities remain intact. Instance
support cannot strengthen the raw unbound descriptor, and contextual callable-bearing carriers
retain the separate final-publication restriction. This shape integration alone does not admit
formal environments in source; native storage and executable producer/plan integration remain open.
Zero-length element edges are linked after the positive-containment walk. They retain source
identity without requiring an element payload, including recursive nominal graphs and otherwise
unsupported element data. An element declaration can still be unready during required evaluation;
that identity-only edge does not request its payload header or manufacture a ready schema. Final
Unit closure still validates every independent declaration. `Type::Unavailable` retains such an
identity without accepting a value; positive occurrences of unsupported data remain outside the
evaluator domain. Variant support follows representable cases: optional absence and fallible success
do not fabricate an inactive error payload. The shared const-outcome source qualifies success
construction, taking inputs, forwarding, assertion, success propagation, frozen constant reuse and ordinary native
execution, including absent `error?` and `(error!)?` values. Live error graphs, their destruction
and runtime primitive calls remain unavailable. Final publication also excludes representable
callable payloads; an unavailable opposite case does not erase that restriction.
Selecting an unavailable payload from a represented carrier cannot produce an evaluator value.
The domain check retains this projection through ordinary value boundaries, allowing
the lowered failure arm of `?` without creating error data. A conditionally supported carrier
retains its own bound domain obligation; admitting that carrier also covers its original payload
projections. An unbound contract alone never grants an actual payload. Execution still performs the ordinary
tag test or assertion. This is evaluator representation evidence, not source reachability;
runtime calls and explicit error destruction remain rejected even in an inactive handler.
Absent optional payloads and partial arrays of those values use ordinary sparse cleanup.
Projection facts are memoized within one domain check rather than rewalking a deep expression
chain. Nested unavailable payloads still report the original assertion site. Native source also
passes an actual failure through the same const transfer body; evaluator-only absence evidence
does not remove its runtime failure path.
The projection derives temporary-operation support and final serialization separately
from these same shapes. Exact named witnesses retain their code identity with a unit environment;
selected read/edit/once views transport one existing payload through ordinary resource transitions.
Value calls omit only that empty callable prefix from the declaration's explicit inputs, retaining
original argument positions and preparation effects. Known factories, array-selected named values,
scoped views and edited ordinary inputs use the same executor and runtime body. Dormant runtime-only
named values may be formed and destroyed, but every authored invocation still requires const evidence,
including dead source. Callable-bearing tuples and nonempty arrays are temporary data without final
constant-publication permission. Exact owned captured code derives invocation availability from its
ordinary checked operations, input shapes and actual cleanup. `constant::Catalog` derives requirements
once from each shared accepted body and closes transitive captured-call facts with local failure
causes; recursion supplies no missing fact. This does
not infer const for a named runtime helper. Formation/destruction of an eligible environment is
independent of body availability: a dormant runtime-only body can be formed and discarded. A called
unavailable body reports E0421 at its actual call, with causal locations in its own lexical code.
The same definition/body and inferred func/mut/once contract serve runtime and required evaluation.

The private source corpus qualifies empty and multiple explicit owned captures with nested captured/named values,
read/edit/consuming bodies, weaker read/edit/once views, independent Copy snapshots, explicit edited
inputs, factory-only results and per-instance state. The shared native source compares embedded
answers with ordinary execution. Arithmetic, bounds, work and call-depth failures retain actual
captured-body locations. Captured calls can also supply local/header array counts; supplier body
changes invalidate header reuse while unrelated body edits do not.

Flat direct callable inputs in named and anonymous code retain conditional invocation, data and
cleanup obligations. The template check does not grant an unbound callback unconditional const
capability. Required evaluation closes the conditions of already selected `CodeInstanceKey` contexts before any
plan executes. During plan construction, committed calls use the same original-source recipe
realization as native instances, under the shared plan-work quota. Named actual
callbacks still need their authored const capability; captured actuals retain their own local and
transitive operation causes. A callback that is received but never invoked needs its temporary
representation, not a body or invocation capability. Interrupted preparation creates no completed
callee instance. A parameter-selected invocation whose frozen argument preparation cannot
complete retains no invocation-capability obligation. Environment formation, receiver preparation,
transfers and cleanup keep their own clauses. Fixed forbidden source calls remain definition
errors even when their preparation cannot complete. The dependency coordinator discovers actual
formal-call targets after ordinary result closure and requests their existing checked drafts without
source body retyping.
Exact required-condition checks use `constant::requirements::bound` to resolve each call lazily
once for one caller frame. Invocation eligibility, receiving-input eligibility and subsequent body
traversal consume those same derived keys. Clause order remains intact; a violation discards the
request-local draft. Resolution failure retains the original operation ID for condition-work
diagnostics. Definition checks have a separate template-only entry point, and frozen dormant
contract binding remains private to condition selection. This memo supplies no final value,
authored capability, runtime owner or executable plan.
`instances::condition_call` uses original source recipes for committed calls during body discovery.
An absent commit can supply only a frozen definition-contract hypothesis: const declarations still
check dormant source operations, but that hypothesis creates no executable call or value receipt.
The strict Unit invocation resolver continues to reject the same absent commit.
`evaluation::call_support` computes which completed-call closures name only fixed code, propagating
uncertainty from parameter-selected or missing callees through a worklist. Reached source recipes
must establish one closed target; missing source evidence cannot establish independence. Fixed-code recursive
components need no unrolling. Body discovery can visit such a component once across differing
witness bindings; every representative still has complete validated metadata. This projection
never merges condition checks, payload representations, cleanup or evaluator/native instances.
`evaluation::discovery` separately factors body discovery when every completed parameter-selected
call receives no callable witnesses. In that domain, fixed callees forward independent witness
positions and selected callees have no callable-argument correlation to preserve. The worklist
propagates each new witness at one formal position through a complete declaration-validated
representative frame, instead of enumerating joint permutations. Actual source recipes must also
have singleton unary producers: unresolved application/result terms and alternative inputs retain
exact discovery. Frames with received compound contexts, or frames/received witnesses with
captured substitutions, retain exact discovery until contextual data is qualified. Existing fixed-target components
still need only one visit. A selected target receiving another callable, or a target outside the
recognized terms, retains exact joint contexts. Discovery is recomputed when required bodies
arrive; a missing body's unknown operations never establish condition or plan equivalence.
This projection executes no value operation and merges no source owner or captured environment.

`constant::Traversal` separately qualifies condition-only sharing for a fixed-target call closure.
Captured substitutions and received compound contexts retain exact traversal; a bare code or raw
environment type cannot erase those contexts. Compound frames bypass both code-only condition
sharing and independent-witness body factoring, including frames with no captured substitutions
or direct callable bindings.
It checks every actual direct callable witness through the evaluator's type authority. When all
those temporary representations are supported, the present condition clauses can observe neither
which callable code was passed nor its environment layout: invocation targets are fixed, and bound
data/cleanup clauses ask only whether that data is supported. One complete representative can
therefore discharge that code's conditions and outgoing edges. An unsupported actual retains its
exact frame and local failure; parameter-invoking and incomplete call closures retain exact frames.
This invariant includes transitive fixed-code recursion and forwarding through normalized views.
It grants no shared evaluator/native plan, representation, serialization or body-generation receipt.
Extending the condition vocabulary requires rechecking this equivalence proof.

Direct callable data leaves select their actual named or captured representation without mutating
the unbound template's type identity; distinct environments and returned callbacks remain distinct.
The shared source corpus covers read/edit/take calls, forwarding, recursion, anonymous formal
inputs, per-instance mutable state and returned taking callbacks through required and native
execution. Context-changing captures, outside R/D/S dependencies, dependent callable carriers,
deeper callable inputs and opaque error-data operations remain outside this evaluator domain.
`constant::Requirements` retains once-derived operation references and sparse cleanup obligations,
sharing the exact immutable `flow::Query` with the unit's body catalog. Calls, input contracts and
types remain owned by that body and its `Program`; the condition product adds no alternate target
or type authority. `Unit::const_requirements` exposes the product for that exact code generation.
Unit clones share both products and bodies. A changed body cannot reuse the old source receipt.
Required checks apply concrete bindings to the stored clauses and call edges rather than walking
source bodies or rebuilding resource obligations.
Expression-domain clauses retain original operand and result-definition sources from the shared
value-source flow. Bound checks test each selected concrete data context, preserving the original
expression diagnostic and metadata-work failure location. A live contextual value cannot be
recovered from a frame-wide type default. Dormant expressions retain their definition-contract
checks; the existing uninhabited-projection rule remains separate from producing an unsupported
value. These selections supply metadata, not execution or source-access permission.
Definition-only data hypotheses use `Types::supports_contract` over the same canonical shape
graph. Callable parameters remain conditions; aggregate and captured-environment composition
preserves them without making the raw shape supported or serializable. Known unsupported data,
including an owned error environment, still fails that hypothesis. Exact-instance checks consume
the original value contexts and ordinary supported-shape predicate; a conditional descriptor
cannot manufacture a payload, invocation capability or final value.
Explicit `drop` clauses retain their original checked action locations. Bound checks query the
local's reaching data immediately before retirement through the common source-definition
normalizer, preserving the authored statement's diagnostic. A dormant statement without an
action retains its definition-contract check and supplies no fictitious source value. Independently
closed schemas need no contextual lookup; other live drops cannot use a frame-wide type default.
Each cleanup condition retains the original transition ordinal, before/after resource phase and
step ordinal behind its diagnostic site. Selection validates the original cleanup role and schema
against that immutable body. Replacement conditions select the retired obligation, independently
of a supplied value with the same raw type. No resource phase is inferred from source-action
completion. Exact-instance checks specialize these selections through the same ordered
resource-data projection used by execution consumers, then test the selected concrete data key.
A request-local cache shares a complete before/after projection for each original action and
transition; failed or partial projections are never cached. Metadata-work failures retain the
original action location, and unsupported cleanup retains its original diagnostic type and cause.
Unbound definition-contract checks remain distinct from this concrete selection. These metadata
checks grant no source admission, initialized storage or physical-layout receipt; general
context-changing carrier transport and cleanup still require consumer qualification.

Each invocation clause derives its identity-frame call edge once from the frozen operation.
Interrupted preparation stores no edge; unresolved producer/view metadata stays an unavailable
edge rather than strengthening an otherwise valid runtime definition. The clause owns the edge
and original operation reference together, without a parallel call index.
`Requirements::template_calls` composes these retained edges through `CodeTemplateKey` frames.
A `CallTemplate::Code` keeps the callee's formal positions bound to concrete or outer-parameter
witnesses; `CallTemplate::Parameter` retains the selected unresolved witness and invocation view.
The original operation ID and shared body retain the remaining call metadata. Named and lexical
closure formals preserve their own identities across forwarding. Interrupted preparation creates
no completed template frame. Known-code edges compose by one-step outer substitution through
the declaration authority. Required closure closes the same retained edges to concrete frames;
parameter-selected edges retain their frozen argument metadata and the common call binder.
A bound failure retains its original witness position. `accepted::const_admission::provenance`
follows that position through each frozen argument binding, annotating only the argument chain
that supplied the failed witness, alongside the ordinary call path. It uses verified body/source
receipts and never searches other callbacks for a favorable capability.
Each internal edge reference borrows its matched checked body and rejects a different caller code.
This is symbolic edge metadata, not a solved const condition or an executable-callee proof.

`constant::Conditions` derives a finite symbolic conjunction of invocation, direct callable-data
representation and actual-cleanup requirements from
those same clauses and retained edges. A worklist substitutes callee witness positions into the
caller, including recursive components, without unrolling concrete combinations. Outer parameters
remain symbolic. An already satisfied authored named-capability atom is pruned while the callee's
own requirements still propagate. Missing producer/view facts and outside captures are not
invented substitutions. A closed false invocation condition is rejected in an uninvoked authored
const definition; ordinary runtime callers remain valid. Captured availability consumes the same
requirements, so an indirectly unavailable body can be formed and discarded, but not invoked.
Causal nodes refer only to earlier nodes in one arena. Definition failures project the original
operation, supplying argument chain and call path through verified body receipts without a
recursive cause graph or source retyping.

Callable representation and cleanup remain distinct predicates even where this private domain
uses the same shape-admission check. Their original expression, explicit-drop or sparse-ledger
sites remain distinct causes. An unused read input acquires no owned cleanup; an unused taking
input retains its actual destruction condition through forwarding. None of these data predicates
requires invocation availability or final serialization of the callable.

Direct callable receiving-input conditions derive from the declaration authority's same
`function_contract` used by ordinary const input checking. Each retains its receiving read/edit/take
operation independently of the function value's invocation view. A completed known-code call
substitutes the callee's formal data witness through its stored frame and keeps the actual argument
as its cause. Interrupted preparation contributes no callee input or body-owned cleanup. Receiving
an input does not require its invocation and does not invent Copy, an owner or a retirement.

`Requirements::clause_coverage` separately checks whether every current clause is represented
in the finite conjunction. A parameter-selected call additionally retains a selected-body
obligation, distinct from its authored invocation capability. Its currently covered receiving
boundary has supported noncallable data and no callable argument bindings. Unrepresented data,
missing producer/view metadata, foreign captures and missing/uncovered fixed callees prevent
coverage; incompleteness propagates back to callers. Coverage alone supplies no eligibility.

For root-closed named functions and closures with supported owned environments,
`Conditions::prove_closed_root` discharges the retained predicates and recursively preflights each
selected body's complete conjunction. Selected bodies
must have no callable formals requiring a further concrete frame. Unknown or uncovered obligations
return to exact checks before consuming condition work. A successful proof rebinds every reachable
checked body through the matched source/type receipt and charges its completed source edges,
including parameter-selected edges. This avoids enumerating joint witness permutations without
using the body-discovery projection as eligibility evidence. It relies on coverage of the current
retained predicates and declaration-owned signature/view normalization. More general contextual
shape projection does not extend that predicate coverage; new dependent carriers still require
revisiting this proof. Closure invocation uses the catalog's existing conditional
availability, separately from environment representation and the selected body's complete
conjunction. An unused supported closure needs no invocation capability. Unresolved/free callable
captures, unsupported environments and callable receiving dependencies retain the exact-frame path.
The proof neither executes nor memoizes source results, merges layouts,
nor supplies a plan or final-publication exception.

Outside this covered scope, absence of a failure in the finite conjunctive product is not a complete
eligibility certificate. Compound dependent data, value/resource-state guards and general symbolic
composition remain integration work. Ordinary exact required checks still apply. Module publication
requires separate qualification; the products alone grant no exported unconditional const guarantee
or evaluator implementation.

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

Concrete call-frame discovery and required condition closure share a separate condition-work
quota, with the configured work limit, across one outer request and its nested dependencies.
Each completed instance-call edge spends a unit before selecting its binding frame. Body discovery
also charges original-recipe fixed-point resolution to that quota. Cache-driven
reanalysis therefore spends analysis work rather than source execution operations. The quota
resets with outer work; it neither adds source call depth nor resets live storage. Exhaustion
reports E0322 with `ConditionWorkLimit` at the matched source call, distinct from evaluator
`StepLimit` and from a failed const capability. It publishes no value or false eligibility fact.
A failed request can retry through the same dependency coordinator with a replenished quota;
unused callback values still do not demand invocation bodies. This bounds the expanding direct
witness closure, not all compiler process memory or general symbolic solving.

Concrete evaluator plans remain separate. `evaluation::projection::Source` realizes a body only
when execution enters its exact instance.
It retains the matched checked-body products, shared type projector and complete witness interner.
The type projector selects contextual components by validated source fields and follows callable
environments through their separate environment edge. Resolved structural shapes share runtime
identities when their actual child representations agree; nominal identity and callable code and
environments remain distinct. Source descriptors and canonical frame keys retain their own identities.
An empty array retains raw element classification in the source-key table, while its executable
`Type::EmptyArray` contains no element identity or payload. It grants no element access.
Evaluator slots identified as the body's environment input or its retained backing use that frame's
complete environment key. Input layout identifies this role; sharing a TypeId does not make an
ordinary local an environment. The source input schema must match before selecting the shape.
Component-context qualification includes a mixed carrier whose same-source callable leaves retain
different environments. Its evaluator shape remains distinct from both uniform carriers, and this
metadata construction does not strengthen the raw body's support or final-publication eligibility.
Each realized body links its immediate call boundaries without recursively realizing their bodies;
no lookup, type checking or const-capability inference happens in the executor. The evaluator
validates each append-only batch against those boundaries before executing it. Actual callable
environments and executable instances remain distinct even when their conditions can share.
Construction charges an independent plan-work quota before each body is built and throughout
original-recipe call-frame resolution, configured from the same request work limit. Receiver and
input preparation still inspect frozen contracts without creating a committed callee instance;
interrupted calls need no invocation recipe. Exhaustion in either body loading or frame resolution
reports E0322 as plan preparation, without spending source execution steps, publishing a partial
value or manufacturing a const-ineligibility fact. Nested dependencies share the quota, and a
new outer request resets it alongside condition and execution work.

Final publication admits bool, the intrinsic integer catalog, unit, program-extent text, eligible
fallible success data and finite closed optional/record/tuple/fixed-array
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
values can themselves form an owned environment, using declaration-owned descriptor dependencies
and the original checked value-supply operations.
Unmarked formation still needs source-context Copy; explicit transfer retires even a Copy function
value. Selected mut/once views survive capture. Result/application components follow the closure
boundary below. Context-bound callable inputs follow their separate boundary; inferred outside
access remains outside this qualified scope.
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
Each capture constructor retains its checked value-supply operation and valid descriptor
dependencies. Pending or absent result payloads reject locally rather than reaching resource/layout
execution; valid callable formals follow the context-bound boundary below.

The shared `result-owned-captures.nct` source compares required answers with ordinary native calls
for result captures, nested/returned environments, callable-bearing arrays, independent snapshots,
persistent updates, explicit consuming bodies and repeated reads of non-Copy environments. Copy
once views keep snapshot behavior while explicit retirement remains visible. Runtime source also
forwards a captured result through a taking formal and preserves its exact witness. This does not
qualify general formal payload storage or formal substitution in required evaluation. Source tests
reject unmarked non-Copy acquisition/consumption, retired-source reuse, unknown capture payloads
and result-induced positive environment cycles. No new syntax, runtime dispatch or environment
ownership registry is introduced.

`recreated-captured-callables.nct` exercises the same loop-local owners repeatedly through a
factory result, a taking forwarder, a tuple component transfer and a consuming call. A saved sibling
view remains readable after that transfer. Required evaluation and the actual native process both
produce 42 from the shared source. This qualifies recreation of established environment shapes,
not changing formal capture representations or native geometry.

### Context-Bound Callable Captures

Declared compound inputs retain a distinct formal contract at each callable leaf. Input matching
checks the whole structural contract while preserving the supplied value's semantic identity.
Original argument sources close the received `CodeInstanceKey.inputs`; forwarding reprojects each
matched component into the next input's descriptor rather than flattening it into direct callable
bindings. Binding first closes the original supplied schema, then uses canonical reprojection to
retain its exact positive contexts and excluded alternatives under the formal descriptor. Selected
places and slot merges retain their original typed contracts; a receiving type cannot fill in a
missing source contract. Completed input reprojection uses the same canonical operation.
At a fresh transport into a matching formal input constraint, reprojection preserves the supplied
domain instead of guessing a positive destination context. Fixed code/environment destinations
retain their independent receiving-instance proof.
Header reuse reconciles component contracts under their original reserved input owner.
`contextual-compound-inputs.nct` qualifies reading and taking tuples, forwarding, two independently
selected callable leaves, singleton arrays and zero-count arrays through required/native execution.
It distinguishes nested callable contexts and repeated calls with the same context but different
values. The matching error fixture executes actual received-container and discarded-seed cleanup.
Missing transfers, moved-source reuse, runtime-only invocation and changed header contracts reject.
`contextual-compound-input-results.nct` qualifies owning callable results selected from received
tuples and arrays, including direct/function-value calls, anchored recursive forwarding,
independent same-signature tuple leaves and different
received callback contexts at the same array-selection function. Symbolic producer equations match
declaration-owned component identities against the original structural producer facts; type-only
applications retain descriptor equations without manufacturing committed calls or input values.
Exact result realization still requires the original received compound context. Missing context
and exhausted work reject independently, followed by healthy retry. Required/native execution
agrees, and the matching error fixture executes remaining-container and returned-owner cleanup.
Higher-order component signatures and callable record fields remain separate admission boundaries.
`received-excluded-outcome-inputs.nct` qualifies separately received absent and positive callable
domains through taking inputs, whole-outcome return/forwarding, nested calls, original resource
projection, required evaluation and native execution. A missing input context remains unresolved.
Per-call empty-result proof substitutes actual nonempty producer relations into the formal result
summary and checks the original result equations. Reached applications reuse the original commit
recipe; only dormant type applications construct a descriptor-only recipe. An empty input set alone
proves nothing. This
does not feed substituted calls back into the global positive result summaries. Only an original
named result's outcome guard admits this excluded leaf; ordinary callable results still require a
positive producer. The matching error fixture executes actual forwarded failure and capture-owner
cleanup. `constructed-outcome-inputs.nct` qualifies fresh `none` inputs for function and compound
primary constraints. Only a matched foreign input component without a selected positive context can
choose an excluded domain for that explicit inactive alternative. Construction checks its original
source contract even when the requested target is already excluded; different function signatures
cannot be retyped through absence. Existing input domains and positive supplied fields keep their
own evidence. `lifted-callable-outcome-inputs.nct` qualifies supplied functions lifted through
optional, nested optional and success layers, including tuple-component construction. Shared
constraint selection checks the callable contract and retains the supplied code identity before
building the outcome layers; required evaluation and native invocation agree. A different callable
signature rejects. `constructed-outcome-results.nct` qualifies explicit constructor exclusion through
original return/application equations, nested forwarding, required evaluation, per-call input/result
keys, resource projection and native cleanup. Each original result/application has one
`CallableEvidence` classification. `Excluded` retains active-value exclusion separately from
`Uninhabited`: closed-domain queries still reject without concrete instance metadata. Original
equations distinguish constructed exclusion from missing or empty unknown inputs. Outcome control may prune a proven inactive primary
continuation, preserving the original carrier evaluation and tag/trap operation. Dormant const
requirements use value exclusion only when the original graph has no received occurrence or
committed call; actual values still require their concrete representation. Recovery through owned,
read and edit access, propagation and original assertion traps are covered through required/native
execution. Symbolic transport classification may use the exclusion, while concrete source/destination queries must still close
and validate their own keys. Exact canonical reprojection can preserve a supplied excluded domain
under the receiving result descriptor; otherwise the original consumer constraints remain required.
Distinct calls retain absent and positive actual inputs/results independently.
`constructed-outcome-environments.nct` extends the result path through positive inactive callable
environments. Original returned sources first close under their own typed descriptors; the selected
callee's declaration/result path then qualifies receiving result-family metadata through the
canonical `reproject_result` authority. The same lexical owned-capture body can retain different
nested callable environments without erasing their positive represented domains when the actual
outcome is absent. The same source qualifies tuple and two-element tuple-array results and
optional payloads inside a success/failure carrier. Tests retain each original inactive-callable
input fact separately from its positive physical domain, compare distinct contexts at the same
lexical result descriptor, read adjacent scalar fields, transfer/invoke active callable components
and clean only the remaining container. Native error variants retain and dispose actual owned
error environments while inactive alternatives create no owner. Mixed tuple results preserve two
independent input families, including a recursive call with a constructed inactive first input and
a positive captured second input. Both result leaves share one original invocation recipe; swapped
actual environments remain distinct through extraction, invocation and disposal. A success/failure
carrier around a two-element tuple array retains those two families and homogeneous per-leaf
contexts even when the second row is absent. Scalar offsets, partial extraction, original resource
phases and remaining cleanup use the same source in required/native execution. Generic signature
matching still cannot create a result context. Instance-specific received primary continuations
retain separate qualification obligations.
Declared compound-result contracts have declaration-owned result ordinals. A diverging body can
retain that known signature without establishing any returned value, concrete environment or
storage representation; publishing its header is not a consumer-realization proof.
`contextual-compound-results.nct` qualifies independently witnessed tuple results, positive-length
callable-array results and nested tuple/array results through direct/function-value calls, taking
inputs, anchored recursive forwarding and partial extraction. Different nested callback contexts
retain their actual original return sources. Receiving descriptors can be reprojected into the
caller's concrete result shape only through actual component evidence; a matching signature cannot
substitute another static callable. Required/native execution agrees, and the matching error fixture
executes returned-owner and remaining-container cleanup. Missing transfer/input context, retired
source use, wrong concrete targets and exhausted work reject; healthy retry remains possible.
`contextual-excluded-result-components.nct` qualifies a returned tuple with an absence-only
callable alternative beside a captured callable. Exact descriptor reception preserves the supplied
component instances rather than reconstructing positive environments from the target descriptor.
Forwarding, two distinct captured callbacks, partial extraction, remaining-container cleanup and
required/native execution retain those separate contexts. This does not widen a represented outcome.
`empty-compound-results.nct` qualifies standalone and mixed empty-array results, declaration-to-
declaration forwarding, preserved result-ordinal gaps and adjacent context-bound callable values.
Empty-array element signatures are checked without adding producer equations or element values.
Required/native execution preserves the once-only evaluation and cleanup of a zero-repeat seed;
the matching error fixture executes seed and retained returned-owner cleanup. Wrong signatures and
missing seed transfers reject.
`contextual-outcome-results.nct` qualifies uniquely witnessed optional/fallible callable results,
exact forwarding before outcome injection, nested optional layers and optional tuple components.
Present values retain their actual nested callback contexts; absent alternatives inherit only
representation constraints through original forwarding, aggregate, merge and return channels in
the same canonical frame. No type-wide default or positive producer is inferred from absence.
Ordered outcome assembly consumes this shared definition metadata while separately checking the
actually supplied resource fields. Required/native execution agrees. The matching error fixture
executes discarded captured-owner cleanup, returned callable cleanup and actual failure propagation.
`producerless-outcome-results.nct` qualifies absence-only callable results, declaration-to-
declaration forwarding and nested optional construction through required/native execution and
ordered resource projection. An empty positive solution is accepted only beneath an outcome
and with complete equations and known call targets; an opaque target or unresolved root callable
cannot establish an empty domain. An uncommitted application remains distinct from a completed
carrier whose callable alternative is excluded. The evaluator shares one explicit `Uninhabited`
shape, distinct from unresolved `Unavailable` descriptors. The matching error fixture qualifies
failure-only results, exact forwarding and disposal of actual owned error graphs.
Extracting an excluded callable and application-specific emptiness through received inputs remain
separate obligations. After outcome refinement, `results::admission` checks reachable taking,
forwarding and return boundaries. Taking conversions emit an explicit transport; both supplied and
receiving domains may depend on the actual committed callback inputs.
Other known widening boundaries report `UnsupportedData` at
the supplied expression. Unknown metadata establishes no conversion. Direct destination construction
of `none` remains supported without moving an existing smaller representation.
`widened-outcome-results.nct` qualifies absence-only forwarding into an inhabited callable result.
`widened-outcome-carriers.nct` extends the same operation to tuple and array carriers, preserved
component values, partial extraction and differing element strides. Required evaluation and native
execution agree. `widened-outcome-results-errors.nct` qualifies native failure forwarding, actual
owned error graphs, successful captured-owner cleanup and named recovery. Both representations and
original tags survive the single-owner transition; matching signatures or equal geometry do not
replace it. `contextual-widened-outcome-results.nct` qualifies the same lexical receiving closure
with distinct callback environments in separate applications, required evaluation, exact resource
projection and native layouts. The matching error fixture qualifies failure forwarding, retired
input captures and returned capture cleanup. `contextual-widened-outcome-carriers.nct` extends
transport to supplied tuples and tuple-array carriers with adjacent positive callback contexts,
partial extraction and differing field offsets/strides. The error fixture also qualifies failure
carriers with supplied captured owners, inactive success payloads and remaining-container cleanup.
Different positive environments cannot be widened to agree; a receiving constraint never rewrites
the original supplied schema. Changing external/aliased storage remains a separate boundary.
`producerless-outcome-guards.nct` qualifies value/read/edit recovery and value propagation when
the primary domain is empty. Checking refines only the impossible outgoing edge of the original
tag condition; every source action, tag read and action identity remains intact. Original value
sources are then derived from that same graph. Required body discovery follows its reached call
channels instead of demanding code for a dormant excluded invocation. Const requirements omit
only an excluded target with no reached call, or an empty-domain data operation with no operand
or definition occurrence; fixed forbidden operations in dormant source remain checked. A positive
producer with the same signature keeps its ordinary call and result. The matching error fixture
qualifies value propagation, named/discarded recovery and read recovery with actual error-graph
cleanup. Missing transfers and use of a dropped carrier still reject. Representation widening
retains its separate qualification boundary.
`flow::outcome_domains` also refines an assertion's impossible continuation. Initial control
construction gives each owning/scoped variant check a separate continuation point. Domain closure
can mark that existing outgoing edge unreachable while preserving the original check, its action
identity, preparation effects and non-unwinding trap. Source actions are never truncated or moved
to new identities during refinement. Required execution of excluded callable assertions reports
the original inactive-variant failure and source span for value/read/edit forms. Native qualification
covers those assertions for absence and actual owned failure graphs, with prior exits preserved;
inhabited counterparts retain their ordinary result.
`contextual-compound-edits.nct` additionally qualifies scalar updates through edit tuple inputs,
forwarded updates and caller reads after completion while the callable context remains unchanged.
The matching error fixture executes actual cleanup after both updates. Receiving/caller descriptors
are reconciled from original completed-input components, preserving may-write footprints and weak
joins. An abstract signature match cannot substitute another static callable. Immutable storage
and overlapping still-used references continue to reject.
`contextual-mutable-array-inputs.nct` qualifies evaluated element receivers through edit array
inputs and forwarded calls, with persistent independent state in two homogeneous elements.
A singleton array also changes its nested callback context between calls to the same receiving
function; the moved-out callback retains its earlier state independently of its replacement.
Required/native execution agrees, the matching error fixture executes remaining-owner cleanup,
and out-of-range selection preserves its original failure/trap. These cases do not qualify
context-changing assignment inside a generic receiving body or general external carrier geometry.
The edit tuple and mutable-array fixtures also qualify freshly constructed compound arguments;
the tuple fixture additionally lends an explicitly transferred owner and rejects subsequent source
reuse. Their temporaries retain completed input metadata through post-call cleanup. The tuple
error fixture exercises interruption during later argument preparation: the already constructed
callable graph is cleaned while the receiving body remains uninvoked. This does not establish
context-changing writes inside those temporary environments.

An owned callable input can enter an explicit captured environment without a declaration-closed
representation. Its descriptor retains callable dependencies; the checked transfer supplies the
value. After result closure, pending producers and absent applications still reject. Each concrete
invocation then selects its actual canonical code/environment key through the shared original-value
queries. A matching signature or another instance of the same lexical closure cannot supply that key.

The formal contract does not acquire Copy or invocation capabilities from one concrete argument.
Unmarked acquisition still requires a source-level Copy proof, explicit transfer retires its source,
and a runtime-only callback cannot become a required const invocation. Definition-only domain
hypotheses remain separate from supported payloads, final values and exact-instance conditions.
Dormant expressions and explicit drops use those unbound hypotheses rather than an unrelated
execution-frame type default; receiving and executed operations retain their own exact checks.

`formal-captured-callbacks.nct` qualifies one lexical capture with named and owned environments,
forwarding through taking inputs/results, tuple formation and read aliases, read/edit/take
invocation, persistent state and required/native agreement at 42.
`formal-captured-errors.nct` executes nested owned error graph disposal through consuming adapters
and explicit destruction in a real native process. Owned error payloads remain outside required
evaluation. `contextual-component-transfers.nct` qualifies transfer from a local tuple or array
whose callable descriptor is context-bound, retaining a disjoint sibling view and the original
container representation through required and native execution. `contextual-component-errors.nct`
executes selected-callback and remaining-component error graph cleanup through that native path.
These cases do not qualify changing container geometry or general partial destination writes.
`contextual-callable-replacement.nct` qualifies sequential whole-local replacement of the same
lexical wrapper with different captured callable contexts. Calls before and after replacement,
explicit transfer into a saved owner and restoration retain their exact instances. Empty named
environments and owning captured environments also exercise growth and shrinkage when no
reference survives the replacement. A still-used read reference prevents the overwrite.
`contextual-callable-replacement-errors.nct` executes actual retired, received and saved error
graph cleanup across those changes. These whole-owner cases do not establish stable offsets
for retained component references, partial writes, joins or changing backedges.
`contextual-component-replacement.nct` additionally qualifies same-geometry partial replacement
and restoration while a disjoint sibling reference survives, including repeated writes through
one local edit alias. Original carrier selections match its actual containing owner. The evaluator
retains the alias's entry view and changes its current referent contract only with an actual checked
receipt, validating both the owning container and the alias without adding ownership state.
This projection requires one owning local carrier and established component coordinates. Calls and a saved transferred
owner retain their actual contexts in required and native execution. The matching error fixture executes
retired, saved and received graph cleanup. A native negative case changes the initial environment
size and must reject shifted sibling geometry before an executable can be published. General
relocation and changing offsets remain unqualified.
`contextual-indexed-replacement.nct` qualifies the same transport through a singleton array nested
in a tuple, using a usize input as the evaluated index. Required/native execution preserves the
sibling reference and saved callable, while a matching error fixture exercises actual cleanup.
The first replacement uses a retained edit alias after its original index variable is changed;
its saved address still selects the original element. A one-element carrier has one coordinate
on normal continuation, using the same selection rule as numeric/resource projection. The
original bounds check executes when the alias is formed, before any replacement can occur.
An independent native negative case grows the captured element representation and rejects changed
sibling geometry before publishing an executable.
The normal-continuation coordinate proof is shared by numerical projection and resource paths;
it never rewrites the operand value or original bounds disposition. Out-of-range input still
fails in required execution and traps natively, and mixed multi-element contexts remain unresolved.
`contextual-indexed-same-context.nct` separately qualifies a multi-element update whose original
numeric analysis establishes its local index and whose element contexts remain homogeneous.
Additional dependent carriers, context-changing carrier writes, surviving
partial references, inferred captures and external dependencies still require separate qualification.

### Contextual Anonymous Contracts

A selected callable contract can supply omitted closure parameter data, input operations and result
data. `annotations` represents either ordinary data or that static callable constraint; the same
expression context reaches annotated values, selected call operands, declared callable results and
conditional result positions. It does not assert that a formal witness is the concrete expression's
payload identity. Ordinary constraint matching and invocation adaptation still close that identity.

`closures` checks parameter count and conflicting explicit parameter data/operations or result data
before body checking. An unmarked anonymous input inherits the selected operation even when its body
uses less authority. With no expected callable contract it remains read, and this bounded scope
requires explicit parameter data. An omitted result can close from the closure's own checked
returns and normal completion when their data identities agree or the existing result solver
proves their pending witnesses equal. Each value first passes ordinary
owning supply; inference does not turn a read alias into ownership or omit a required transfer.
`closures::results` consumes those existing typed operations and closes the one result slot before
typed-body publication. An unresolved slot has no placeholder `TypeId`. Empty completion and an
explicit empty return supply unit; a wholly diverging body with no result data needs an annotation.
Conflicting data and result-dependent optional/failure operations without enough context report
at the local source. Declared or expected results still supply ordinary contextual checking before
the body. Explicit callable result annotations and expected nested callable results use that same
`Annotation`/`FunctionPattern` constraint model. It supplies inner anonymous input data and operations
before checking each inner body, without selecting concrete returned code from the signature.
Each explicit or block-final return passes ordinary value supply and any permitted invocation
adapter. Only established result data replaces the construction-only constraint before publication.
Known data results keep their existing contextual checking. Later uses do not repair missing context. Named declaration
operations remain their own contracts. Environment invocation is inferred separately from reached capture operations; an
expected view cannot grant a stronger invocation capability.

Anonymous callable inputs with ordinary signature data use declaration-owned formal publication
before the body is checked. Input operations remain explicit or contextual; body use cannot turn a
read input into take. The input's lexical closure and ordinal identify its witness, independently
of another closure with the same signature. Ordinary call preparation freezes formal-to-actual
bindings, including forwarding between named and anonymous bodies. A taking anonymous input may
return its environment through the same caller-owned application relation. A result revealed by a
factory inside that anonymous body retains its dependency until the common equations bind the
caller's actual input. `Program::function_result` independently projects the closed declared
result through those frozen bindings to validate normal call-result agreement. No body is retyped
and no callee-owned formal is published as a caller value.
Additional callable input carriers, deeper callable signatures within those inputs and inferred capture
footprints remain unqualified. The [constant-domain owner](#constant-domain-acceptance) defines
the separately qualified formal-substitution scope.
The shared anonymous-callable-application source covers deferred input returns, calls through
returned higher-order code, result invocation adapters and interrupted inputs. Its required path
executes a returned concrete callback. The logical-module primitive source also carries a real owned-error environment through
a deferred anonymous return and executes its invocation and cleanup.

The shared contextual-closure source qualifies annotation/call/result completion, nested conditional
result positions, edited caller state, taking non-Copy inputs and mut/once adapters through required
and native execution. This fixture's required paths invoke exact local/factory witnesses, while
native source also calls selected formals; required formal calls follow the constant-domain owner.
The shared inferred-result source checks tail/early/explicit returns, unit, record/tuple/array
results, owned captures, nested lexical bodies and unchanged read/edit/take inputs through required
and native execution. The shared logical-module primitive source also returns a real owned error
from an inferred closure and executes its ordinary cleanup. Inferred results may also retain an
existing named function, a nested owned closure or an application of a declared callable factory.
The same canonical result projection closes pending leaves in the result contract; no extra
producer table or signature-based erasure is introduced. Returned environments pass ordinary owning
supply and cleanup. The shared inferred-callable-result source covers these paths in required and
native execution. The shared anonymous-result-contract source additionally checks explicit/expected
nested results, contextual take inputs, early returns and mut/once result adapters. Returning an
existing higher-order named function retains its already declared formal inputs; the annotation
does not turn a result constraint into an anonymous input witness. Distinct result
applications with matching structure retain obligations in the existing witnessed-result solver;
only proof of the actual common code can close them. Normally supplied results select the
inferred identity before interrupted expressions. Known interrupted shapes still undergo contract
checks and cannot claim a produced value; without a normal candidate their static shape remains
available, independently of execution and witness-production evidence. General parameter/formal-witness inference, outside-reference capture and symbolic
dependency contracts remain separate realization obligations.

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
Optional cleanup checks its payload; fallible cleanup checks both the success payload and the
builtin error contract. A currently selected tag cannot certify an otherwise unknown cleanup.
Caller-write evidence grants no constant-evaluation capability or error-graph evaluator operation.
The shared lexical scope supplies captured bodies to that same solver. An edited environment has
its own input root; a consuming body owns its received environment instead. Closure environments
also participate in cleanup eligibility, including read/edit bodies driven through taking adapters.
Unbound formal cleanup, unavailable callees and unsupported alias provenance remain unknown. Earlier
standalone type suffixes cannot supply cross-body evidence without the shared final program.

Checked bodyless primitive bindings may supply a terminal no-caller-write fact only when the
runtime role independently certifies caller-storage preservation. The current error snapshot
constructors carry that evidence; input modes and the absence of a source body do not imply it.
Whole-unit closure pairs those receipts with the exact declaration context. Standalone catalogs
without that authority remain conservative. Recursive/transitive closure joins the primitive fact
with ordinary body writes, so a wrapper that edits an input still invalidates that caller root.
Reuse retains the original header authority and checked role bindings alongside the existing body
dependency graph; an equal separately prepared program cannot substitute its roles or bodies.

`call_writes::requests` uses the declaration-owned `FunctionWitness::substitute` for already frozen
actual bindings, preserving the selected invocation view. Missing bindings leave effect targets
unknown; an incompatible supplied view is an inconsistent product. Executable instance selection
uses the same substitution rule with its stronger closed-target boundary. `CallKind::invocation`
decodes the callable environment operation from the explicit direct/value role, independently of
ordinary argument operations or count. Effect requests are keyed by lexical code and known
witness/view bindings. Runtime environment contents do not
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

With an authored outcome-element context, listed absence-only constructions wait until the other
supplied entries establish the common element data, then are checked once using that data. This
covers literal `none` and complete conditionals whose branch results recursively construct only
literal absence. It does not infer tags from named values, inspect called bodies or omit condition
and branch effects. An all-absent list keeps its authored element contract without inventing a
callable. Each value-bearing entry is checked against the original context; no first-element
coercion is introduced. Final aggregate fields retain source order, including effectful entries and prefixes
prepared before a later argument exits. `contextual-absent-callable-arrays.nct` qualifies named and
captured functions, distinct nested environments at the same lexical code, all-absent arrays,
partial transfers and actual owned-error cleanup through required/native consumers. Unknown
contexts, incompatible signatures and distinct positive witnesses remain separate from absence.

Conditional elements pass construction context into both branches without turning a receiving
callable signature into an exact supplied witness. Branches still check their declarations and
signatures, including dormant branches; only normally producing branches supply the result's
identity. A statically selected branch retains its own application descriptor. The shared source
also qualifies runtime-selected captured environments and an early return during element formation.
Exact local and parameter witnesses remain binding constraints rather than construction hints.
After the branch identities agree, explicit value boundaries give continuing branch values the
conditional result's common receiving descriptor. Equivalent represented domains still retain
that schema transition through the existing single-owner transport operation; physical equivalence
does not leave branch-specific descriptors attached to one merged result slot. Schema-only
reclassification remains ordinary adaptation in the original value graph, retaining its result-family
receipt. Representation-changing transport separately records both original carrier domains.

When one contextual conditional branch recursively constructs only literal absence, check the
value-bearing branch against the authored constraint first, then use its known outcome descriptor
as the absence-only branch's construction context. Each branch body is checked once; the original
graph retains its condition and source-ordered statements. No foreign input witness is equated to
the callable merely to type the absence. Interrupted branches retain their lack of normal completion.
A condition that cannot complete drops exact outgoing value boundaries but retains construction
shape for dormant branch checks; it supplies no runtime payload or callable receipt.
`construction::absence` owns the
memoized syntax-only query shared by arrays and conditional results, discarded with the body builder. It neither
infers named-value tags nor replaces producer evidence. The shared source covers either branch order,
nested absence-only branches, source-ordered updates, `else if` results and earlier returns in one or
both branches. Already prepared array prefixes retain their ordinary cleanup, including native
environments owning errors. Incompatible dormant signatures still reject.

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
For a singleton array, any normally completed selection identifies coordinate zero. The shared
projection may use that coordinate for ownership and metadata even when the operand is unknown;
the operand evaluation and original bounds disposition remain unchanged; a required check still
executes. This proves neither that an
out-of-range operand equals zero nor that different recipes have equal numeric values.

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
that missing old data is not destroyed and snapshot retirement follows installation; the
[native effect corpus](../nocter-machine/README.md#owned-error-effects) additionally executes
owned-error construction and release through these restorations.

A single concrete descendant of a retained element can also be restored. The numeric history
keeps its selected field path distinct from a whole-element point and from a whole-element
prefix. Restoration requires the same suffix, proved numeric equality, the actual missing path
and matching retirement causes. Availability and resources then use the original snapshot; a
write to one child cannot close a missing whole element or another field. The shared projected-field
source qualifies unchanged indices, saved equal indices after mutation and repeated restoration
across alternating loop indices through required/native execution. Partial transfers never extend
a whole-element prefix. A completed replacement disjoint from the missing descendant preserves
its single-point evidence, using the shared structural path relation even when the indices are
unknown. It does not treat another transfer or an unknown call as a harmless replacement.
Intervening call effects and general relations among multiple retained holes remain separate
obligations.
Source also qualifies a later prefix loop after restoring the sole point, without replacing the
containing array. Independent negative checks retain earlier field holes and longer-prefix holes.

Conditional restoration additionally supports complete-or-single-point histories at one assignment.
Equality with the saved index is required only on the histories that contain a point; complete
histories supply no invented snapshot. The proof retains every possible retirement source.
Whole elements and concrete descendants use the same conditional proof, with the exact suffix
validated across every retirement source and the target. Availability closes only matching
constraints beneath that array whose causes all match those sources; its parent must be present and any other overlapping unavailable coverage rejects
the proof. Concrete and symbolic constraints may describe different branches of that same point.
Prefixes and general unproved histories do not acquire this conditional proof.
Resources select the actual unique missing-point path from each ownership alternative, preserving
constant paths whose saved index was elided. An alternative without a missing point must have a
complete array and uses ordinary initialized referent replacement. No numerical-equality predicate selects a backing lifetime, and inactive backing is
never read. Shared required/native source covers absent holes, distinct retirement sites with equal
indices, changed user indices and maximum-count zero-byte arrays. Projected-field cases also use
a different target number on the initialized route; that route reads no missing-point backing.
Logical error data checks the initialized-replacement versus saved-point alternatives; the
[native effect corpus](../nocter-machine/README.md#owned-error-effects) owns actual graph qualification.
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
dependent formal substitution still needs qualification under the [constant domain](#constant-domain-acceptance).
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
