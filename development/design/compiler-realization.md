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

Extend this path to local annotations, repetition counts and later constant-generic arguments
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

Array values need their own combined source proofs over these same schemas: contextual/inferred
construction, once-only seed evaluation even at zero repetition, Copy requirements, selected
indexing, bounds checks, partial initialization/restoration, aliases, access reservations and
remaining-element cleanup. Dynamic selected indices retain their evaluated identity and checked
path conditions. Neither a constant-index restriction nor a language-level count ceiling may
replace those proofs. Large cardinalities must retain homogeneous/sparse representations rather
than expanding the payload into tuple-like metadata or compiler state.

Then qualify evaluator array data/publication and native layout, zero-sized element addressing,
input/result channels and destruction against those accepted source operations. Compare repeated
calls with equal scalar inputs but different editable places; result-only memoization must not
erase state transitions. Unsupported operations, arithmetic errors, dependency cycles, budgets
and final serialization retain distinct source causes. The existing native header cases prove
that unused/computed array types coexist with scalar execution, not array value or ABI support.

### Body-Owned Required Expressions

Local type lengths and repetition counts need a body-owned expression-code domain before source
admission. Do not append these roots to the closed header reservation arena: adding an expression
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

Carry the complete expression-code identity through header/body/value requests, unpublished body
type branches, selected applications/closures, evaluator plans and failure projection. Sharing a
source owner cannot make two expression bodies share a type suffix or application key. Reuse the
[body structural type authority](../compiler/crates/nocter-declarations/README.md#future-value-declaration-boundary)
and its canonical array/tuple replay; neither a second type store nor a literal-only evaluator is
needed. Completed required values remain immutable facts in the common dependency query, with
their ordinary work/storage charges and transitive source requirements.

Qualify the source connection with local annotations and repetition, independent sibling-body
edits, a supplier-body change, nested structural data, dead authored requirements, actual runtime
inputs and type/value cycles. Keep array body/resource/storage admission separate until their
combined proofs close. Header-count tests alone cannot establish body-local identity or scope.

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
