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
array-length/constant-argument calls need the owning dependency protocol: the current header
stratum cannot reopen ordinary body checking or publish fabricated type/constant placeholders.

Early structural requests must start from the declaration owner's
[closed reservation domain](../compiler/crates/nocter-declarations/README.md#future-value-declaration-boundary),
before type-dependent definition publication. The existing coordinator records distinct header,
typed-body and required-value keys in one dependency query; extend source type normalization to
request its values through that authority. A checked body may consume
only closed contracts for its selected references; if one is unavailable, suspend its unpublished
work and require that contract rather than treating an unknown length as zero, unit or a fresh
opaque type. Compute a required length through the ordinary checked usize/evaluator path, then
intern the resulting array identity in the same type authority. Publication still requires every
declaration and every definition to close.

Use the already-reserved input/field identities when publishing a ready contract; normalization
order cannot allocate new slots or renumber a formal witness. The current
[staged source path](../compiler/crates/nocter-checking/README.md#bounded-future-value-binding-typing)
can retain bodies over earlier ready declaration views and replay their type suffixes after later
contracts close. Requested contracts publish transitive record schemas on demand, while raw source
data references still require a complete first stratum. The same
readiness/value coordinator executes selected const closures while unrelated headers remain
unready. Extend that boundary with deferred source-reference
normalization and header-dependent required-value requests through the one
dependency protocol. A reservation alone cannot type-check an unfinished signature. Qualify actual
array source through the combined path rather than treating ready-view/slot tests as array conformance.

Keep type/value cycles distinct from callable recursion. A request that needs its own unfinished
normalized type or value is a dependency error, including an authored requirement in dead source.
Callable-only recursion instead closes one checked graph and remains subject to execution budgets.
Body bytes used by a header value must participate in header reuse invalidation; the current
body-independent header-surface equality cannot certify an evaluated length after a supplying const
body changes. Preserve exact source recipes and causal edges across those requests. This integration,
not reservation closure alone, is the remaining gate for array type lengths.

### Structural Type Request Closure

This is the remaining construction design, not implemented array admission. Retain source type
recipes until their selected declaration is requested rather than eagerly resolving every header.
The closed name/input/field domain stays fixed. Normalization can then return a typed dependency
request without publishing a provisional contract. Source lowering owns loading the requested
recipe; declarations own validation and normalized identity; the existing checking/value
coordinator owns ordinary body acceptance and required execution. A length expression needs its
own semantic code identity and exact source recipe. Do not manufacture a named function, source
text or a separate expression checker to evaluate it. Code identity and source-reuse ownership
must cover expressions in record fields as well as callable headers. The current callable-only
source-owner projection cannot represent a record-owned length by relabeling it as a function.

Keep deferred header normalization on the
[query-owned iterative progress path](../compiler/crates/nocter-dependency-query/README.md#invariants).
Introducing raw type/value dependencies must preserve deep-schema preparation without host
recursion, exact dependency causes and the shared active/completed authority. Do not replace this
proof with a source depth or array-length ceiling, or charge header traversal as source
execution-call depth.

Separate nominal name identity from payload-schema dependencies. In particular, the
[array containment contract](../spec/language/tuples-and-arrays.md#scope-and-identity) requires
evaluating the count before deciding which element-payload edges need closure. Requesting every
element's complete record schema before normalizing the count would incorrectly turn a valid
zero-length containment break into a cycle. A known nominal leaf does not itself prove payload
validity or Copy; every independent declaration must still validate before whole-unit publication.
Finite containment, structural Copy and ready-schema publication must consume one declaration-owned
dependency model and policy; separate eager and deferred validators cannot maintain competing
containment or Copy rules.

When extending raw-definition loading, preserve the existing
[declaration rejection boundary](../compiler/crates/nocter-declarations/README.md#future-value-declaration-boundary)
and its source projection. A known contract needing a recipe, a rejected definition and an unknown
declaration remain distinct. Loading a recipe cannot invent a valid default or discard the exact
declaration/header-position cause while a body or value request is active.

Qualify the boundary with computed lengths in fields and function signatures, forward helpers,
dead authored dependencies, zero/positive containment, type/value cycles and supplying-body edits.
Then qualify local annotations, construction/repetition, selected indexing, partial initialization,
cleanup and native execution against the same normalized array identity. A test of early scalar
evaluation or of a reserved array key cannot stand in for those source paths.

The evaluator must preserve edit-input writes, aliases, partial state and once-only cleanup.
Compare repeated calls with equal scalar input values but different mutable places, selected enum
payloads, zero-count repetition and temporary escape. Result-only memoization must not erase
state transitions. Eligibility, arithmetic failure, evaluator budgets and final serialization need
distinct source causes. The current [initializer boundary](../compiler/crates/nocter-checking/README.md#required-private-initializers)
does not establish conditional generic eligibility or publication of unadmitted aggregate/static forms.

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
