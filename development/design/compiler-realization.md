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
non-Copy value capture still needs explicit formation permission. General nested captures and
factory-result closure bodies need code-scoped application identities, not their parent's domain.

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
Keep effect reuse distinct from typed-call reuse: a callee-only body edit may preserve the call's
type but invalidate its write proof. Verify transitive reached bodies and their value/count/result
prerequisites through existing source guards. Share the dependency graph, handle recursive components
without repeated visits, and preserve reuse after unrelated body edits. Whole-unit publication can
use its collective body verification instead of repeating transitive checks for every body.

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
