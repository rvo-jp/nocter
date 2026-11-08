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
- Scalar literals retain intrinsic values, while references to declarations retain `ConstantId`;
  checking never copies an evaluated declaration value into a checked body.
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


## Bounded Future Value Access Typing

The development-only private-function goal also has a separate typed-only leaf query. It opens
from one exact-current access-declaration context and uses its immutable program directly. It
resolves lexical names, checks data and structural permissions, contextualizes explicit boundaries,
selects direct callable identities once, and records primitive operations, Copy/materialization,
transfer/update actions, call argument reservations, assignment order, and structured control.
It does not construct or convert to the released `CheckedProgram`.

Its body-local semantic arenas contain no source coordinates, syntax identities, names, or symbol
IDs. A successful reusable query owns its callable, exact declaration authority, exact body bytes,
and locator recipes beside that typed body. One body projection serves checking and recipe capture.
Rebinding requires that same authority and unchanged own body, then projects current origins without
retyping or repeating dispatch. Body-independent header reuse does not imply body-result reuse.

Structural eligibility is separate from named-slot initialization, source liveness, escape/origin
proofs, access conflicts and validated cleanup. The shared flow query below supplies those facts
for the exact first-fragment data contracts; whole-unit acceptance closes their callee assumptions;
even a typed-only body with an uninitialized read or escaping local cannot enter execution. Primitive
arithmetic retains checked operation kinds, and short-circuit/control interruptions remain explicit.
The future product has no compatibility adapter, source-version flag, editor recovery product, or
native consumer. The [adjacent boundary](../../../architecture/pipeline/value-access-declarations.md)
owns its relationship to declarations and later validation.


## Future Body Control Flow and Slot Initialization

A shared typed-query authority produces one immutable evaluation/control graph. Graph construction
consumes typed arenas and selected argument actions without syntax, dispatch or type inference.
It preserves eager operand preparation, short-circuit/conditional edges, loop destinations,
RHS-first assignment, staged return values, scope exits and cancellation of each successfully
prepared call prefix. A direct-place reservation does not execute a normal place read or transfer.
A complete commit validates the reserved bundle before changing any ownership slot. Literal
Boolean predicates select their known edge after condition evaluation; structurally checked dead
source contributes no reached initialization use.

The graph and its fixed-point initialization report share that exact typed-query authority.
Each reached point starts with a map of named-slot states: initialized, uninitialized or maybe.
Unreached points have no entry state. Parameters start initialized; bindings initialize their
holders; explicit transfers, drops and committed X places remove ownership; complete assignment
restores X. Joins conservatively combine all incoming states, and a worklist reaches a fixed point
before reporting uses. Reaching invalidation locations remain separate source-neutral evidence.

Cleanup facts retain initialization conditions at potential owned-slot replacement, explicit drop,
and scope exit. They are not execution schedules, do not erase carried dependencies, and cannot
certify that a retained I/U root survives. Non-owning holders are released rather than destroyed as
owners. Scope exits retain reverse binding order, while a protected result is staged before exit
or cancellation cleanup. Source projection reuses the typed query's exact-current binding without
reconstructing flow or solving again.

The referent/access analysis below consumes this graph and named-slot prerequisite. Evaluation
frames also delimit temporary cleanup during normal and interrupted evaluation. An initialized
slot and an initialized referent are distinct facts. Intermediate reports cannot independently
construct accepted evidence or enter execution.


## Future Referents and Permission Demand

Backward fixed-point liveness tracks semantic carriers and their read/update demands. Carriers are
locals, computed answers and prepared arguments; witnesses retain source-neutral action locations.
Scalar operand/condition preparation consumes a read without keeping its source access for later
primitive computation. Cancellation retains its prepared prefix for cleanup, but releasing a
non-owning access does not dereference its root. Owned payload cleanup dependencies are not inferred
from that fact.

The forward analysis retains alternative worlds across control flow and private result-origin
candidates. Each world tracks named owning roots, external input roots, delegated accesses,
prepared reservations and a staged result. It never unions mutually exclusive U roots into
simultaneous permissions. Equivalent worlds converge over a finite domain of existing semantic
identities, parent relations and permissions; there is no arbitrary iteration cutoff.

A use checks its actual root and required permission. Removing an unused intermediary reparents
live children; after the last update requirement, remaining reads need only read authority. Source
types, selected operations and interface contracts stay fixed. Explicit update creation still
requires update eligibility at its authored point. A later U call/result keeps exclusivity live,
while a pending I request need not retain an otherwise unused U intermediary.

Reservations protect identity/value changes throughout preparation and cancellation. They activate
as a complete checked bundle at commit. Computed access answers are already active; duplicate X,
U/U and I/U inputs cannot be made safe merely by sequentially activating them. Result referent
candidates are alternative input roots; output accesses are reparented through ordinary cleanup
without retaining callee parameter storage. Root removal checks live accesses and staged results.

This is a referent/permission report under the selected callable contracts. A zero private access
origin bound has no normally producing access result in this closed fragment; every callee body
must independently satisfy that bound before whole-program acceptance. `NoReferent` describes R
only, not empty carried payload or storage requirements. Bodies with initialization failures are
explicitly blocked from this analysis. An analyzed report without violations alone cannot close
callee assumptions, supply native validity, or qualify an ABI.

## Future Logical Resources and Accepted Private Units

The same alternative-world solver tracks a finite logical owner map and evaluation-frame stacks.
Ownership is independent of access liveness: a dead value still owns its required destruction.
Each reached operation records before/after ownership alternatives and ordered construction,
move, callee-result receipt, disposal, invocation-transfer or caller-input replacement steps.
Named R survival reads that owner map directly; no parallel root-survival set is retained.
No dynamic resource counter or execution-history log participates in the fixed point.

Payload evidence is an exhaustive match on the admitted built-in data contracts, not a Copy or
owned-value heuristic. Scalars/unit have trivial destruction; the immutable owned-error snapshot
has private storage and an owned-graph disposal obligation. All four have empty carried D/S under
their exact normative contracts. Other data shapes cannot acquire that certificate. This is not
a general dependency solver for aggregates, closures, generics or regions.

Statement/condition/block-result evaluation frames retain fresh I-call inputs without Copy.
Successful X invocation transfers the prepared owner; cancellation destroys only its owned prefix
in reverse order. Named reservations never become early transfers. I temporaries remain caller
owned until their evaluation ends; temporary referent roots prevent a saved access from silently
keeping them alive. Forwarded/staged owners leave the cleaned frame; other temporaries are disposed
before enclosing lexical cleanup. Whole-value replacement disposes only the old owner present on
that path; a U input replacement disposes its initialized outside payload and installs the ready
replacement. Every normal return has empty evaluation frames and exactly its declared X result
owner, or no owned result for I/U. Trapping/diverging paths gain no unwinding guarantee.

The closed private-unit constructor checks every definition in the supplied declaration context,
including unreachable or recursive callees. It publishes no partial accepted value after a typing,
initialization or access rejection. The accepted unit owns the exact declaration authority and
all validated body/flow/resource reports. Its source binding validates every original body before
exposing projections; a changed sibling or unrelated equal header authority rejects reuse.
There is no public parts constructor, released-program adapter, public version switch, native
consumer, user-destructor support, or outcome/async executor. A target consumer must realize the
ordered transitions and checked primitive operations under its own layout/runtime contract.
