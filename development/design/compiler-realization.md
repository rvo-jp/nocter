# Compiler Realization

## Scope

This document owns remaining integrated verification and realization requirements for v2.0.0.
It neither repeats the [adopted language](../spec/README.md) nor defines current crate internals.
The [private access handoff](../architecture/pipeline/value-access-declarations.md) owns the
implemented cross-stage boundary and its limits. The [work plan](../TODO.md) owns priority.
These are pending obligations, not passed source fixtures or a full-language soundness proof.

## Remaining Source Supply Coverage

The [private handoff](../architecture/pipeline/value-access-declarations.md) owns the admitted
core and its component contracts. Extend its one frozen source-retirement/snapshot decision
through each newly admitted family; do not recover source intent from concrete Copy, liveness,
type size or private-body lookup. Remaining distinguishing cases include:

| Source situation | Remaining evidence |
| --- | --- |
| General generic types and opaque capabilities | Preserve explicit retirement and conditional Copy without granting a missing capability |
| Static storage and user index/current projections | Reject manufactured ownership even for Copy referents |
| Dynamic array elements, partial parents and user destructors | Exact source identity, overlap, restoration and cleanup on all exits |
| Named outcome, handler/pattern and future sources | Explicit acquisition at the owning failure/drive boundary |
| Anonymous pattern/cursor carriers | Internal handoff without a hidden move of a named source |
| Fallible scoped projection | No hidden error extraction from preserved storage |
| Marked array spread, including empty/Copy arrays | Whole-source retirement at direct outer commit |
| Marked pack capture/spread/loop | All owned components retire while original count metadata remains |
| Read/edit pack marked acquisition | No caller-component retirement |
| User source acquisition before outer failure | Preserve committed inner calls, pulls and cleanup |
| Pending cancellation during preparation versus after receipt | Release reservations versus destroy actual received state |

Derive case semantics from [explicit transfer](../spec/language/value-access.md#explicit-transfer-sources).
The source corpus and consuming tests own actual core assertions; this plan does not duplicate
completed test inventories or admit another implicit source mode.

## Dependency and State Proofs

General capabilities, dependent aggregates and callable environments require schemas that preserve
direct-place access R, carried dependencies D and external storage requirements S. Data ownership
or Copy is not proof of empty D/S. Bind symbolic public slots to actual roots without consulting
private bodies, changing a selected operation or granting a missing capability.

Check result origins separately from edited-state relations, including reachable outside targets,
not just descriptor fields. In particular test retained old dependencies, permitted carried-input
insertion, newly retained direct-input access, success/failure clear bounds, and writes surviving
the destruction of a callback or guard. Tuple/pack mappings preserve paths and actual identities.
A dependent result must survive callee, handler, adapter and evaluation cleanup. No root extension,
retargeting, hidden clone or destructor postponement repairs a failed obligation.

Contextual closure completion must retain the selected contract's input operations even when a
body reads less. Inferred read/update captures must preserve outside roots and exclusivity;
non-Copy value capture still needs explicit formation permission. Nested captures and
factory-result closure bodies must preserve code-scoped application identities, not their parent's domain.

## Integrated Source Families

| Owning subject | Remaining integrated evidence |
| --- | --- |
| Comparisons | Same-data selection, fixed interface witnesses, scalar snapshots versus read inputs, original/reversed receivers, short-circuit equality, effects/cleanup and const visibility |
| Modules and type behavior | Contract/body agreement, complete headers, audience paths, visibility and operation binding |
| Typed literals and explicit conversions | Shape/exact-target selection, pack/receiver preparation, explicit retirement, result origins, failure/cleanup and selected const capability |
| Generics, interfaces and opaque results | Symbolic evidence, associated bindings, static witnesses, finite result equations and non-retyping specialization |
| Outcomes and error information | Exact layer order, explicit failure selection, recovery cleanup, scoped projections and lost-failure diagnostics |
| Arrays, enums and aggregate patterns | Exact schema/Copy proof, named-source supply, internal decomposition, ignored remainder and partial restoration |
| Variadic inputs | Key/value order, dynamic paths/count checks, complete contributions, origins and formation/commit timing |
| Traversal and general spread | Source/cursor proof, zero-input projections, sequential versus accumulated roots, early exit and remaining cleanup |
| Temporary update loans | Direct authority versus produced-value backing, nested frames, escape, failure and cancellation |
| Effects and async | Construction/drive/cleanup facts, fresh input extents, ready-output roots and distinct cancellation envelopes |
| Remaining grammar and library/platform boundaries | Explicit contract closure before parser/checker, CTFE, layout or ABI admission |

Use the owning source rules for case semantics. Examples must contain real declarations, source
locations, selected operations and expected resource/access results. Required CTFE uses the same
checked operations and evaluator capability, not an independent source interpreter. User protocol
names do not authorize hidden effects, root stability or target primitives.

## Module and Contract Source Realization

The [module contract](../spec/language/modules.md) owns source meaning. Discovery's existing
`CompileUnitInput` remains the sole inventory and resolved physical-edge handoff. Reuse its
module/source identities and selected roots; do not add a second filesystem loader or flatten
every inventoried source into one visible namespace.

Keep declaration grammar shared across file roles. Validate placement from the selected source
kind before normalization, then select contract/definition candidates through the explicit source
graph. The existing declaration-surface/contract stages are syntax-level infrastructure, not a
conversion through the released borrow-based `DeclarationProgram`. Preserve the private v2
inventory boundary, explicit grammar selection and distinct root eligibility when extending them;
neither language product may silently accept the other's sources.

One logical declaration owns the contract identity, parameter/field slots and normalized types.
Its implementation retains its own full-header/body locators and lookup context. The complete
representation determines field slots and their authored order; a root's selected visible fields
must map to those identities without another record schema. Expression lookup belongs to its
physical header/representation source, independently of the nominal contract's publication scope.
In particular, a required array count in a private representation must not use the root's helper
scope merely because the root owns the nominal identity. Signature agreement needs both canonical source structure and resolved contract equality; a token-only
comparison must not erase meaningful jointness or header-contained expression structure. Body
editing must preserve unrelated contract evidence while pairing, visibility and inventory edits
invalidate the affected namespace products. Current-source binding must choose each source's
actual syntax rather than interpreting all locators through one tree.

Qualify missing, duplicate and mismatching implementations, direct versus transitive visibility,
source-local imports and independent helper spellings with real multi-source input. Root-role
recognition alone grants no public type representation, target primitive, executable body or
accepted Unit. Ordinary CLI/library integration follows that closed source/declaration authority.

## Conditional Const Admission

The [const contract](../spec/language/constant-evaluation.md) owns eligibility and publication.
Extend the [required initializer boundary](../compiler/crates/nocter-checking/README.md#required-private-initializers)
with fixed arrays, enums and early structural dependencies. As ordinary
generic/witness schemas are admitted, qualify condition derivation, per-conformance witness facts,
exact-generation exports and substitution over that same authority. Preserve runtime-valid
instances whose const condition is false; distinguish those from definition-time known forbidden operations and missing ordinary capabilities.

Exercise const and runtime-only implementations of the same interface, stronger const member/
callable requirements, forwarded conditions, adapter cleanup, body-change invalidation and
missing/mismatched evaluator products. Use actual source declarations, frozen selections and
required initializers; a condition algebra alone is not this evidence. Qualify recursive closure
without accepting capability cycles or mistaking budget exhaustion for nontermination. Required
constant-argument and further structural requests must use the same dependency protocol; no stage
may publish fabricated type/constant placeholders to break a cycle.

Extend captured evaluation to the adopted general capture/input dependencies, contextual outside
access captures and formal callable substitution. Preserve definition-time witness availability
and local causes without inferring const for named runtime implementations. General R/D/S and
survival must qualify transferred-view backing and answer receipt before dependent references or
nontrivial cleanup enter evaluation. Symbolic generic/witness/resource-existence conditions and
exported products use the same checked program; no new source annotations or parallel instance/type
authority can substitute for those proofs. Final callable serialization remains a separate boundary.

### Captured Callable Instances

A callable's selected source code and invocation view do not identify the substitutions captured
when its environment was created. Two values of the same lexical closure may retain different
callable inputs. Preserve that distinction in the declaration-owned instance model before
admitting formal callable values into owned environments. A global map from an outer formal to
one code cannot represent both values in a later call receiving them together.

Derive captured witness positions from the closure's canonical environment schema, including
structural components, rather than maintaining a second capture inventory. A concrete callable
witness must retain the relevant closed substitutions with its code and selected view. Nested
callable environments retain their own substitutions; copying or forwarding a witness cannot
strip that context. Keep this metadata separate from runtime payloads and their owners. An
instance key grants no representation, const capability, source receipt or executable plan.

Normalize direct callable formals and captured substitutions through the existing declaration
contract authority. Preserve exact coverage and reject missing, foreign or incompatible bindings.
Resolve frozen calls and data projections from that complete instance without reopening lookup,
operation selection or body typing. Symbolic conditions may retain free dependencies, but cannot
close them by equating signatures or by assigning every use of a lexical closure one environment.

Code-frame substitution is not value-occurrence provenance. One caller can receive two results
from the same factory and lexical closure while their captured callbacks differ. After a result
solver selects that closure's source identity, a table keyed only by `(code frame, TypeId)` cannot
recover which creation frame supplied each local, prepared argument or returned value. Preserve
matched producer/application and creation-frame evidence through result projection and ordinary
value transport. Compose it with the existing canonical witness/data instance model; do not
reconstruct a context from the final signature, code name or arbitrary first producer. The two
values must remain distinct both before and after being forwarded together. This evidence selects
representation and static callable context; initialization, access, Copy, source-generation receipts
and cleanup remain owned by their existing analyses. Qualify joins, component selection, recursive
forwarding and failed requests without an address-indexed value registry or duplicated body typing.

Received compound inputs are another independent value context: equal code, direct callable
bindings and environment do not imply equal nested callbacks in those inputs. The canonical
invocation/frame authority must retain their exact contextual data keys in frozen explicit-input
order. Original body-channel correspondence maps those ordinals to input locals; raw type IDs,
local allocation order or unrelated captured values cannot supply the entry context. An
independently closed input needs no additional context, and direct callable inputs and the
environment keep their existing canonical binding authorities instead of duplicating leaf maps.

Obtain compound-input contexts from the matched original call bundle. Preserve them through
instance interning, entry projections, normal completion and selected component queries, without
substituting a caller's later storage state for the supplied snapshot. Missing, foreign, conflicting
or unqualified inputs remain unresolved. These keys describe static specialization only; they
add no owner, initialized input, access grant, const capability or representation proof.

Dependent carrier shapes must use one contextual realization shared by evaluator eligibility,
plan projection and native storage. Preserve nominal identities, component paths, invocation
adapters and sparse cleanup; a substituted callable leaf cannot relabel its surrounding raw type
or transfer ownership twice. Pair instance use with the existing matched body/type generation.
Qualify the same closure created with distinct named callbacks, nested captured callbacks,
read/edit/take calls, forwarding and failed-request recovery through ordinary source and native
execution. Until those consumers agree, retain the explicit source rejection of unsupported
formal environments rather than accepting a body that has no realizable instance.

Context identity alone does not prove that existing storage can change representation. Replacing
one callable component can change its captured environment's size or alignment while references
to other components remain live. Select the complete old and new carrier roots at their actual
operation boundaries, in addition to the selected component data. A slot's first or only recorded
representation is not proof that an alias-relative write left its carrier unchanged.

Preserve valid saved addresses and projections of unaffected components. Reserving the maximum
root size alone does not stabilize field offsets. The representation planner must establish
compatible geometry for coexisting contexts or prove a relocation strategy that preserves every
surviving reference; metadata must not silently choose either policy. Evaluator slot interpretation,
native field placement and cleanup must consume the same selected root context. Representation
choices must also respect established size/alignment observations, required header values and
published ABI contracts; later specialization cannot invalidate those earlier results.

Qualify mixed environment shapes, including empty named callbacks and callbacks owning captured
data, inside nested carriers with a surviving sibling reference and across replacement/backedges.
Keep original bounds effects and old/new destruction observable. These are compiler realization
obligations, not a request for user annotations, a second owner registry or a permanent ban on
context-changing updates. Unsupported cases remain explicitly unqualified until those proofs close.

### Structural Type Request Closure

The [source header path](../compiler/crates/nocter-declaration-lowering/README.md#exact-source-evidence)
now connects ordinary usize execution to the declaration owner's closed array references,
finite-containment and Copy rules. Its shared query records header/body/value prerequisites and
publishes selected supplier guards with reusable headers. The
[checking owner](../compiler/crates/nocter-checking/README.md#required-private-initializers) defines
its exact admitted scope. Header qualification does not establish array value operations.

The [local annotation path](../compiler/crates/nocter-checking/README.md#body-local-required-expressions)
uses the same query for body-owned annotation/repetition counts and
[listed whole-value operations](../compiler/crates/nocter-checking/README.md#array-construction).
Extend this path to later constant-generic arguments
without reopening ordinary body checking through a competing evaluator or changing the closed
name/input/field domain. Each required expression needs its own semantic code identity and actual
source root. Body-local code must retain its real lexical/source owner and remain independent of
unrelated sibling allocation; a source body edit cannot renumber existing header slots. Reuse the
ordinary type/safety and const-domain checks before execution. Preserve source causes, exact
prefix ancestry, body-local type suffix replay and the common value/work/storage budgets.

Keep type/value cycles distinct from callable recursion. Needing an unfinished normalized type
or value is a dependency error, including authored requirements in unexecuted source. Callable-only
recursion closes one checked graph and remains subject to execution budgets. Reusable types must
retain every selected supplier's source evidence, including transitive constant initializers and
unexecuted callees that participate in ordinary validation. Header-surface equality alone cannot
certify a computed length after a supplying body changes.

Continue using the declaration owner's one type identity and payload dependency policy. Counts
must close before selecting containment edges; a zero array has no element-payload edge but still
needs valid element names, storable types and independent declaration closure. Do not introduce a
second eager containment or Copy validator, provisional zero count or fresh opaque type. Raw
missing definitions, authored rejections and unknown IDs remain distinct at source projection.

Extend the combined source proofs over these same schemas to once-only seed evaluation even at
zero repetition, Copy requirements, selected
indexing, bounds checks, partial initialization/restoration, aliases, access reservations and
remaining-element cleanup. Dynamic selected indices retain their evaluated identity and checked
path conditions. Neither a constant-index restriction nor a language-level count ceiling may
replace those proofs. Large cardinalities must retain homogeneous/sparse representations rather
than expanding the payload into tuple-like metadata or compiler state.

Close the remaining evaluator schema/repetition products and native layout, zero-sized element addressing,
input/result channels and destruction against those accepted source operations. Compare repeated
calls with equal scalar inputs but different editable places; result-only memoization must not
erase state transitions. Unsupported operations, arithmetic errors, dependency cycles, budgets
and final serialization retain distinct source causes. The existing native header cases prove
that unused/computed array types coexist with scalar execution, not array value or ABI support.

### Body-Owned Required Expressions

The [model/type owners](../compiler/crates/nocter-declarations/README.md#future-value-declaration-boundary)
and [source owner](../compiler/crates/nocter-declaration-lowering/README.md#exact-source-evidence)
provide body-owned expression roots and exact source pairing. The
[closed evaluator](../compiler/crates/nocter-constant-evaluation/README.md#stateful-value-plans)
also retains those roots independently of source ownership. The
[checked-body closure](../compiler/crates/nocter-checking/README.md#frozen-body-input-channels)
preserves them through result applications, plan projection and execution-error locations. Its
[local required-expression path](../compiler/crates/nocter-checking/README.md#body-local-required-expressions)
now supplies ordinary lexical checking and full-root dependency scheduling for local array
annotations and repetition counts, including cycle projection and supplier evidence. Array value
operations remain incomplete. Do not append these roots to the closed header reservation arena: adding an expression
in one body would otherwise renumber sibling expressions or invalidate an unrelated header query.
Pair each expression key with its actual root through its enclosing body's source authority.
An edit may invalidate that body and its expressions, while preserving header slots and unrelated
body identities. The full body guard, plus the selected supplier graph, must certify reuse;
matching the count expression's bytes alone cannot certify its lexical environment.

Keep required-expression execution distinct from callable origin and source ownership. A local
count has an expected usize result but no named-function witness, implicit formal inputs or
return target in the enclosing callable. Its enclosing declaration still owns diagnostic source
and reuse evidence. Ordinary lexical name validation must be preserved; a second global-only
lookup for count syntax would miss local shadowing and unavailable bindings. Lexical values need
their actual compile-time evidence. An unavailable runtime input cannot silently become an
expression-code argument, evaluator capture or runtime-sized type.

Capture lexical availability at the ordinary annotation-checking site and retain it as an input
to that expression's shared query request. The source-root catalog cannot reconstruct binding
decisions. In particular, a visible local cannot disappear so global lookup can find another
declaration; expression-local binding checks must retain the enclosing shadowing constraints.
Unavailable enclosing values need an explicit local diagnostic, without fabricating evaluator
inputs. The expression owns its internal control targets and cannot return, break or continue
into the enclosing callable/loop. A source pairing alone is not this lexical proof.

Carry the complete expression-code identity through header/body/value requests, unpublished body
type branches, selected applications/closures, evaluator plans and failure projection. Sharing a
source owner cannot make two expression bodies share a type suffix or application key. Reuse the
[body structural type authority](../compiler/crates/nocter-declarations/README.md#future-value-declaration-boundary)
and its canonical array/tuple replay; neither a second type store nor a literal-only evaluator is
needed. Completed required values remain immutable facts in the common dependency query, with
their ordinary work/storage charges and transitive source requirements.

Extend [listed/repeated construction and whole-array handoffs](../compiler/crates/nocter-checking/README.md#array-construction)
into general element operations and combined nested partial-data/resource proofs.
Preserve local-count source guards, scope exclusions, dead authored requirements, budget failures
and type/value cycles. Gate each unclosed operation at its owning boundary; do not reject an
independently qualified whole-value operation solely because element selection or storage is
unimplemented. Header-count tests alone cannot establish body-local identity, scope or value safety.

### Evaluated Array Selection Closure

The [array selection contract](../spec/language/tuples-and-arrays.md#primitive-component-selection)
owns source meaning; the following obligations concern its checking and execution products.
The [shared path relation](../compiler/crates/nocter-model/README.md#structural-projection-relations)
separates concrete indices from symbolic recipe keys. Connect it to one checking-owned source of
evaluated-index facts, consumed by initialization, access and the existing ownership ledger.
Unknown equality remains possible overlap. Different syntax nodes, slots or spellings must not
become an implicit disjointness proof. Builtin array structure is required for this algebra;
a user index projection can lend the same referent for different arguments.

Freeze the base evaluation and numeric index once, in order. An existing base identifies storage
without copying or requiring the whole array initialized; a computed base owns a temporary.
Preserve that category through selection, calls and cleanup. Index-side effects must be reflected
in the selected container's initialization and access checks. A normal bounds check establishes
index < N before furnishing a place/value; interruption furnishes neither. Carry that decision
to execution instead of re-proving it from a later scalar value.

An alias, prepared transfer or missing element can need the selected index after its evaluation
frame ends. Preserve the numeric value for that obligation, independently of the user's mutable
index variable. Assigning the variable must not retarget an existing alias or cleanup hole.
Keep code-site/slot identity separate from evaluated-value identity and its retained representation.
The backing and lifetime of a saved index must close with its existing access/resource obligation,
not a second ownership registry or repeated evaluation of the source expression.

When current numerical facts prove an in-range constant, use that same fact for availability,
access creation and resource selection. Freeze it in the resulting region or missing path;
never reinterpret an older alias from a recipe's later execution. Such a hole needs no saved
index owner. Preserve the original bounds disposition, and retain actual snapshot backing when
the number is unproved. A prefix region must remain a region rather than its last selected point.

Retained-index backing is an ordinary pure usize snapshot in the existing ownership ledger,
attached to the containing array obligation. Create it only after a normal checked selection and
at the transfer's established commit point. Keep its owner in branch conditions; two paths can
capture equal numbers while requiring different backing lifetimes. The implementation must not
choose a cleanup alternative from numeric equality alone or read an uninitialized alternative's
snapshot. End the retained snapshot when its associated hole/container ends, including whole
replacement, and create none on call cancellation. A recipe may name reusable storage, but an
already live snapshot owner cannot be overwritten by a later execution of that recipe.

A source access footprint can remain unknown while a resource path refers to such an actual saved
value. These are different facts: neither an unknown footprint nor a code-site key is a concrete
number. Project element data through the shared declaration schemas and the original normal
bounds evidence, without substituting index zero. Preserve that original bounds check; later
cleanup consumes its captured result and must handle empty remainder intervals and a successor
at the maximum usize count. The [resource ledger](../compiler/crates/nocter-checking/README.md#concrete-array-resource-frontiers)
owns the qualified logical backing/restore/cleanup stage. The [evaluator consumer](../compiler/crates/nocter-constant-evaluation/README.md#evaluated-array-addresses)
projects saved numeric paths and guards. The [source selection owner](../compiler/crates/nocter-checking/README.md#array-selection)
defines the retained element/partial-element and proven-prefix stages through shared availability and native
projection; multiple independent holes and more general progression relations remain separate
closure obligations.

A loop may execute the same selection recipe more than once. Equal recipe keys across a backedge
are not proof of equal values. Qualify retained indices, invalidation, joins and restoration before
using those facts; an unproved relation cannot erase a prior hole. Repeated extraction with a
checked progression needs a finite region/interval summary rather than unbounded generations,
one entry per array element or an arbitrary iteration cutoff. Preserve conservative local failure
when that proof is unavailable, without imposing a constant-index-only language rule.

For a single whole-element retained point, restoration can reuse the existing committed-index
history. Require a current index equal to that saved value on every reached history, one matching
retirement source in that array epoch, the corresponding unavailable source slot, a present
containing array and the actual matching sparse hole/snapshot. Source-site provenance alone is
not a lifetime or equality proof. A prefix must remain a region, and multiple/unproved sources
must not acquire a single-point proof. Restoration installs the already evaluated replacement
before retiring the old snapshot; a restored backedge may capture new backing only after that
obligation closes. Preserve original target evaluation and bounds checks.
Distinguish one whole point taken from a complete epoch from generic unproved coverage. Restoring
that sole point can recover a complete epoch for a later prefix loop. The zero endpoint of a
proved zero-start prefix also describes one point; restoring a later endpoint cannot erase its
earlier holes. Forget the completed epoch's source provenance so a later loop does not enroll
an unrelated earlier point in its prefix backing policy.
For conditional single holes, prove target equality on each point history and permit complete
histories without constructing fictitious backing. Retain possible retirement sites and validate
the actual unavailable slot and absence of other overlapping holes. Distinct source sites may
save equal numbers while owning different snapshots; select the existing ownership alternative,
not a numerical-equality guard. The initialized alternative must clean its old payload, while the
missing-point alternative installs without destroying absent data and then retires its backing.
Project that destination from the actual sparse missing coverage, including a constant point
whose index backing was elided. Absence of a snapshot does not imply that the element is
initialized. Any overlapping non-point or multiple-point geometry must reject this proof.

Use exact difference equalities between current usize cells for unit-stride induction. Keep
mutable local cells separate from frozen computed values and prepared operands; an assignment
changes only its target cell. Join retains only common differences. In a fixed body, every strict
join splits an equivalence class, so convergence does not depend on the array count or a widening
threshold. Checked arithmetic relationships describe normal completions only; impossible
continuations furnish no proof. Unknown writes and calls without a closed write summary must
forget potentially changed local facts. This analysis consumes typed normal-value evidence and
the existing operation graph; it does not become another source bounds classifier. Before
admitting user-defined destruction, apply its closed write effects at explicit and implicit
cleanup sites as well; today's closed payload cleanup is not a general purity assumption.
Close named caller-write evidence over the same final typed program after result/supply closure,
retaining its exact body owners. Recursion alone introduces no new write; unknown calls and cleanup
effects must propagate through dependencies. Absence of writes into existing caller storage may
preserve numerical histories, while source transfers and input permissions remain obligations.
Required execution and whole-unit acceptance must consume that same evidence. General selective
write footprints and captured/user destruction effects remain separate closure obligations.
Project parameter-relative may-writes through the selected input plan. An edit of a transferred
temporary does not additionally write the old caller region; its transfer is still a committed
resource operation. Invalidate only facts overlapping proved actual targets, keeping unknown
aliases/captures conservative. A may-write never proves a containing replacement or a fresh epoch.
Include cleanup of an edited old value, not only newly produced/owned values. These root effects
apply to primitive structural referents; capability/dereference projections require their actual
dependency/storage mapping. Detached completion points of an interrupted call supply no effects.
For an exact owned closure, classify destruction through its environment's actual payload schema,
independently of invocation. Releasing captured data must not execute the captured body; unknown
environment cleanup must remain unknown rather than inherit a named function's trivial cleanup.
Close invocation effects for exact captured code through the same lexical code scope and dependency
solver as named calls. Keep a retained environment's edit root distinct from explicit input positions.
A taking view edits its received copy/transferred environment, not the original source again;
explicit-input writes still project normally. Include possible adapter environment cleanup and
preserve unknown formal effects rather than inferring emptiness from a structural signature.
Lexical edit aliases can resolve their write root from the existing typed binding initializer.
Retain the original input position or owned-local root, and keep unresolved/escaping provenance
conservative. Root resolution does not prove a dynamic index, exact component footprint or alias
lifetime, and must not create a second authority for binding or access state.
Use that resolver for caller-side alias footprints too. Widen a frozen dynamic selection to
containing storage when exact retained-index evidence is unavailable; never re-evaluate the old
selection with a newer user index. A may-write footprint can overapproximate the invocation's
actual address and must not be used as a definite replacement or an address-producing recipe.
Share that lexical provenance with scalar numerical analysis. An exact chain of whole-local
read/edit aliases identifies the existing current scalar cell, not an additional owner or a copied
value. Alias binding and scope end must not overwrite or retire that cell; prepared operands still
freeze their value before later effects. Evaluated selectors and fields require their own evidence:
a containing may-write footprint must never become an exact scalar or definite replacement.
Keep effect reuse distinct from typed-call reuse: a callee-only body edit may preserve the call's
type but invalidate its write proof. Verify transitive reached bodies and their value/count/result
prerequisites through existing source guards. Share the dependency graph, handle recursive components
without repeated visits, and preserve reuse after unrelated body edits. Whole-unit publication can
use its collective body verification instead of repeating transitive checks for every body.

Multiple retained field holes need remainder cleanup from the ledger's existing missing set.
Different fields can be disjoint even when their saved array indices are equal or unordered;
that access proof does not establish a destruction order. Carry an immutable cleanup projection
of those holes, not a second ownership table. Native execution must visit remaining elements and
components in reverse order and combine exclusions when saved indices coincide. A finite traversal
with predicates over saved indices is valid; generated work and metadata must depend on the body,
schemas and holes rather than the array count. Intact regions and trivial cleanup can retain their
existing efficient paths. Required execution uses its closed payload domain and the same qualified
missing coverage, with metered sparse working state. A cleanup predicate cannot retrospectively
authorize an otherwise unproved read, transfer or restoration. All snapshots remain owned until
the remainder operation and container end complete.

Select a prefix realization from possible re-execution of a transfer site within the same array
epoch, rather than from a loop's syntax or the mere existence of an earlier extraction. Track
may-executed sites as finite source provenance, clearing the affected sites on epoch replacement
or end. This provenance never proves equal numbers; exact difference facts independently justify
each step. Its union join and the difference domain remain finite in the fixed body. Preserve
ordinary selected-point restoration when no such progression needs a region lifecycle.
Connect transfer sites through possible participation in the same proven epoch. Select the finite
connected sites of an actually repeated transfer, rather than enrolling every selector of the
same containing local across unrelated epochs. Replacing that array must isolate an earlier
point extraction/restoration from a later prefix loop.

The progression closure must distinguish an array epoch with no extracted prefix from one with
a proven prefix. Its first transfer proves index zero; a later transfer proves the new evaluated
index equals the last committed one plus one. Record the last committed index in the existing
resource lifecycle, separately from immutable point snapshots. Update it together with the
actual transfer, after the original normal bounds check, never at preparation or cancellation.
Represent the missing prefix as one region, and clean only the suffix after its saved endpoint.
The endpoint is the transferred index, not the user's possibly later-updated counter; early exit
therefore preserves exact cleanup. Whole containing replacement ends that epoch. Do not admit
this progression from numerical facts alone: availability, guarded resource state and every
execution consumer must agree on the region transition first.

Keep the numerical history distinct from its execution backing. An actual prefix uses one
ordinary saved usize owner in the resource ledger, and its missing coverage is one sparse
region. Deliver each selected payload using its own still-live evaluated operand before
replacing the prefix endpoint. In particular, several taking arguments cannot all project
through a repeatedly overwritten endpoint slot. Save the last committed operand and consolidate
the region within the same guarded transfer phase; before/after guards must agree with actual
coverage and backing ownership. No new element owner is introduced for each iteration.
For a direct call argument, protect the payload in its existing prepared argument owner before
the endpoint changes, then use the established invocation or scoped-temporary handoff. Whole-bundle
validation precedes these transfers; cancellation performs none of them. Calling a function still
invalidates numerical histories unless its closed write summary proves what survives.
Ordered prefix-extension proofs within one atomic bundle establish that its distinct selected
elements are consecutive and disjoint. After checking every input pair, per-source validation can
exempt the bundle's own neutral frozen-place nodes; actual access grants, external aliases and
outer reservations remain obligations. This contextual proof does not refine a retained alias
from the current contents of a reused source-expression cell. General index relations must retain
their evidence for the actual selected-address lifetime before crossing re-evaluation or backedges.

Project the resulting sparse remainder and its exact captured indices into cleanup. Excluding
one checked index i from [0, N) gives [0, i) and [i + 1, N); the same bounds fact justifies the
successor arithmetic even at the maximum usize count. Symbolic cleanup must retain that fact and
the saved value through physical lowering. It cannot substitute the current contents of the
index variable or invent a subarray type. Qualify mutations between selections, conditional
equality/disequality, reservations with later effects, partial nested elements, loop re-execution,
failure and zero-sized data through actual source before opening precise element-state and transfer
admission. Complete-value access disjointness can close separately through the
[selection path](../compiler/crates/nocter-checking/README.md#array-selection); it does not establish
these precise element-state proofs. The concrete
ledger tests and path-algebra tests do not establish these source proofs.

## Executable Lifecycle and Target Admission

Extend the [bounded common native path](../architecture/pipeline/value-access-declarations.md#common-machine-body-construction)
as new source families are admitted. In particular, qualify runtime error creation and nontrivial
record/capture destruction, prepared input receipt, protected answers, conditional cleanup and
normal adapter exits with actual native effects. Physical address channels must remain explicit
without fabricating source layout keys or reviving borrow data types. A read/edit body invoked through a consuming adapter needs its
received environment disposed before answer delivery; a consuming body must not receive a second
destruction. Static code sharing must not merge runtime activations, environments or source owners.

Preserve all frozen call targets, invocation views, input-channel mappings and result obligations.
Interrupted evaluation can carry a known type without a value, witness, environment or cleanup;
never invent an executable edge for it. Close required runtime helpers before graph freeze.
Use the [private stored-layout/input boundary](../architecture/pipeline/value-access-declarations.md#private-stored-representation)
for data geometry and call placement. Qualify their actual activation/backing, caller-to-body
transport and result use through MIR and native consumers with the same source behavior.
Keep trap/divergence, recoverable cleanup and pending/ready cancellation distinct.

## Proof Obligations and Validation

- Preservation: each transition keeps its data schema, actual initialization and one cleanup owner.
- Access soundness: active/reserved operations and dependent cleanup cannot conflict on possible overlap.
- Dependency preservation: supply, wrappers, substitution, erasure and outside writes retain required roots.
- Exit soundness: protected results and edited state survive every applicable cleanup boundary.
- Abstract-call soundness: each body refines its public contract for arbitrary eligible inputs and fresh extents.
- Analysis correspondence: finite CFG abstractions conservatively model branches, loops, partial state and frames.
- Lowering refinement: executable transformations preserve frozen decisions, failures, effects and cleanup timing.

Run direct affected production tests before required broader gates under the
[verification contract](../verification/README.md). A parser shape, manifest count, finite assumed
model or unrelated LSP/native test is not proof of the complete path. Optimization experiments
require separate correctness, target-code inspection and measurement.
