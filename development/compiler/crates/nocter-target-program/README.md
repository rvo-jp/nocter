# nocter-target-program

## Responsibility

Validate one checked program against a selected toolchain target, then close one executable or test
root into a deterministic concrete reachable program.

## Contract

`TargetProgram` owns target/toolchain acceptance for `check`, `build`, and `run`.
`ExecutableProgram` owns entry-driven monomorphization, concrete callable closure, frozen dispatch,
reachable declared-constant values, and executable type representations consumed by MIR. The crate
does not inspect syntax or repeat checking decisions.

## Internal Responsibilities

- target capabilities, primitive completeness, trusted target-service ABI validation, and
  compiler-owned runtime-storage validation
- package target and entry validation
- executable and test root selection
- concrete instance, closure, and drop reachability
- monomorphized item and representation closure
- private value-binding instance closure under `value_access`, consuming a complete accepted Unit
  and explicit root without granting released/native target admission
- concrete logical lifecycle schedules under `value_access::lifecycle`, retaining checked
  ownership conditions, operation phases and normal adapter exits before result receipt
- activation-local packed initialization tests and writes under `value_access::lifecycle::flags`
- private stored-shape and body-input projection under `value_access::storage`

## Invariants

- Released target validation runs once before executable specialization.
- Target validation consumes only `CheckedProgram`; this crate has no source-projection dependency.
  The checking-owned output maps that semantic component into `TargetProgram` and preserves or
  restores the exact projection pair outside this layer.
- Every concrete dispatch comes from checking's selected semantic authority.
- A target-service dispatch retains the catalog-owned target, calling convention, import, and ABI
  classes; executable closure never derives them from a declaration name.
- Error-constructor signature validation consumes the runtime contract's shared value signature.
  Its published adapter expresses read/edit inputs as the published access-qualified type and
  taking inputs as values. Published exposure, target and provenance checks remain here;
  the adapter does not authorize a v2 bodyless source declaration.
- Runtime storage is accepted only from a toolchain-selected, fieldless, unique nominal declaration
  in the standard package; its physical layout remains outside semantic representation.
- Runtime-storage bindings are validated before primitive signatures that refer to them. An invalid
  prerequisite therefore cannot be reported as a downstream primitive mismatch.
- Runtime primitive generic slots are explicitly type-domain contracts. A constant-domain
  parameter is rejected during target validation rather than being silently discarded by MIR.
- Every primitive whose closed signature returns `future T` must carry runtime-contract evidence
  whose typed produced-computation classification certifies nonblocking drive and cancellation;
  certification on a non-future role is rejected as the same contract inconsistency.
- Executable specialization owns key construction; a consumer cannot pair a semantic identity with
  an unrelated specialization type store.
- Reachability uses semantic identities, never runtime symbol spelling.
- A concrete closure type enters the runtime type table only with the representation emitted by
  its reachable executable item. Closure types from unselected bodies remain semantic facts and
  cannot require runtime layout or body access.
- Checked constant references remain identities until executable closure freezes exactly the
  reached values; MIR cannot reopen declaration storage.
- MIR receives no unresolved requirement, interface implementation, or generic lookup.
- Private future function-witness keys have no released representation or runtime-type
  mapping through released executable closure. That boundary rejects them rather than guessing a
  closure/function-pointer layout; the private storage product uses its selected static witnesses.

The cross-stage contract is documented in
[Target, Executable, and MIR Program Design](../../../architecture/pipeline/target-program.md).

## Private Value Instance Boundary

`value_access::InstanceProgram` owns the future private product's root-driven static instances.
It retains the exact accepted Unit and its body/type authority; no released CheckedProgram or
TargetProgram adapter exists. Roots requiring externally supplied callback witnesses reject as
unbound. Named and anonymous call instances use the same owner-scoped formal binding keys;
an anonymous instance requires exactly its own callable inputs, independently of environment size
or the positions in another closure. Constant/static initialization thunks reject as roots and never become runtime instances through
constant/static references; checking has already frozen their initializers. Static place views
retain their global identity and ordinary data projection without adding runtime code instances. Scalar/aggregate
runtime inputs remain ordinary data rather than specialization arguments.
This product grants no ABI, layout, executable cleanup or backend capability.
Body-local required-expression code has no callable origin and cannot enter native root/callee
selection. A declaration-owned metadata key alone supplies no runtime body; a source owner cannot
substitute for the expression's code identity.

`InstanceKey` and `ConcreteWitness` reexport declaration-owned metadata; this crate defines no
competing binding key or constructor. `build` interns those keys for one named/captured code identity
and its own formal-witness bindings plus captured substitutions. A concrete witness retains its
code, supplied invocation view and complete captured callable context. The code instance itself
is shared across views. Actual runtime environment values do not enter its key. The builder reserves
before following recursive edges and freezes only after every reachable instance is defined.
Only checking-reachable invocation commits create call edges. A value reference, uninvoked binding
or interrupted argument does not force a code instance. Construction indexes are discarded; one
immutable instance arena is the published authority.

Reached call frames come from the exact accepted Unit's original prepared value sources through
its [budgeted fixed-point resolver](../nocter-checking/README.md#shared-flow-access-and-resource-validation).
Code-summary/definition hypotheses do not replace missing actual invocation evidence.
`build` consumes checked invocation commits and never reconstructs a creation context from the
projected call signature or code name. `prepare_with_budget` shares one deterministic metadata-work
quota across all reached calls; default preparation uses the shared compiler work policy. Failure
retains the requesting code/call or original action site, publishes no partial instance graph and leaves the Unit
reusable. Runtime execution work and call-depth limits are not spent or inferred here.

The declaration-owned `Program::bind_instance` still owns complete formal/capture coverage,
contract matching and view validation. Source permissions, original body invocation and explicit
input operations stay frozen. Declaration-owned normalization remains shared by source-value
queries and body projection. Actual [instantiated payload obligations](../nocter-checking/README.md#shared-flow-access-and-resource-validation)
retain their complete `DataInstanceKey`, including captured environments and component context.
`Instance::payloads` exposes the original occurrence, preparation and body-channel metadata; it
does not supply runtime values or access permission. `Instance::metadata` contains only independently
closed storage roots requested by reached schemas. Its construction takes no frame and cannot
resolve or choose a contextual occurrence. Storage interns both these sets and the lifecycle's
additional actual keys through one canonical authority. No per-frame source-type default table is
present. `Prepared::instance` also requires that exact key for lookup; the storage table cannot
reconstruct an occurrence from a code frame and raw source type. Transitive components remain in
the canonical data graph rather than a flattened inventory.
The declaration authority's `DataInstanceKey::has_same_representation` checks explicit input and
returned-result channels against their supplying payloads without equating their source descriptors.
It follows actual logical components and preserves nominal and selected callable code/environment
identity. Empty-array element metadata supplies no represented element. Native storage interns
resolved structural tuples, positive arrays and outcome carriers by their actual child identities;
it keeps every original source/context key as a lookup alias and does not create semantic types.
Unproduced source classification stays with the accepted Unit; it supplies no storage, payload or
code instance. Source-selected moves, reservations and snapshots remain unchanged by projection.

`Instance::operand` and `Instance::definition` retain checking's read-only `ValueData` from
[`Unit::instantiate_value_data_map`](../nocter-checking/README.md#shared-flow-access-and-resource-validation).
This layer does not traverse source definitions or select their schemas again. Storage interns the
actual keys without changing source ownership or granting materialization.
`Instance::referent_root` and `Instance::referent_root_after` retain checking's paired original
reference-root contracts. Storage interns their complete keys separately from independently closed
schema probes. These contracts identify
virtual reference roots at source actions, not enclosing allocations, initialized owners or the
old/new sides of a resource step.

`Instance::input` retains the checking-selected payload for each existing input local; the typed
body's input layout remains the sole owner of order and operations. `Instance::result` retains its
selected payload or the checked unproduced contract. Checking's
[body-channel metadata queries](../nocter-checking/README.md#shared-flow-access-and-resource-validation)
own source selection and the distinction between contract metadata and an actually produced return.
Evaluator contracts consume those same queries; this layer adds no alternate resolution rule.

Frozen body edges compare against those channels, and storage signatures and MIR input/return
roots consume them directly. Additional closed storage metadata cannot change an existing bound
ABI channel. Input/result metadata queries share the instance work quota and identify the
exact code/input or result on failure. MIR expression production and local binding consume the
original occurrence definitions, and operand projection uses the matching action's actual reads.
Detached declaration-closed metadata can still select scalar snapshots; a missing contextual
occurrence cannot fall back to a frame-wide type entry. Partial carrier roots and general post-update
representation still require integration before extending the qualified
[context-bound capture scope](../nocter-checking/README.md#context-bound-callable-captures).

`projection::requirements` collects schema requests for the frozen input/result ABI, checked
reachable operations and their resource steps. Independently closed requests contribute metadata;
contextual representation must come from actual value/channel or ordered resource obligations.
Skipping a non-closed schema probe supplies no positive representation receipt. Beginning an evaluation does not
produce its result; a proved index failure requires its base/index data without an element carrier.
The typed body's referent-base projection retains complete storage-root data for reached accesses,
including static/member views, without resolving an index or granting access. Positive schema
edges of required payloads still demand their component representations; empty arrays have no
element-payload edge. Source-checked absent shapes remain metadata, and Unit/Program retain the
complete source types. Unneeded positive metadata cannot demand a callable binding or layout.
Required carriers still reject missing bindings; frozen source permissions and ABI slots stay intact.

The shared `empty-captured-callable-array.nct` case qualifies a generic factory instantiated with
both empty named code and a non-Copy captured environment. Its returned closure contains only an
empty array and has no callable binding for an element that cannot exist. Reached factory inputs
and seed disposal retain their real bindings, while the body preserves its original bounds trap
through native execution. Detached function metadata cannot force a phantom callee or carrier.

`Instance::prepared` retains the actual metadata from each reached argument-preparation receipt,
even when a later operand prevents the invocation commit. Checking's
`Unit::instantiate_prepared_data` owns source selection and repeated-context agreement; this layer
only instantiates the resulting payload obligations. The original typed layout retains operations.
Each committed edge verifies its complete payload list against those preparation receipts.
MIR argument, prepared and temporary roots consume these keys rather than rereading the original
expression's TypeId default. Storage includes their actual geometry without creating an uncommitted
callee, invocation effect or adapter-completion schedule. Metadata for a reserved adaptation does
not assert that its runtime materialization has happened.

`invocation` binds checking's original committed argument bundle to the selected ordered inputs.
Argument payloads, including the callable-value carrier, normalize the bundle's actual source
value IDs under the caller frame and the same deterministic work quota. They do not use the
frame's default payload for a matching TypeId. Result ABI metadata can use an independently closed
schema; a contextual result follows the original post-commit expression definition through its
actual callee frame. Multiple retained bundles must yield the same complete edge rather than
mixing callee, environment and argument alternatives. Source-selected construction adapters retain
their value provenance even without an independent Evaluate action, and opaque numeric updates
retain their typed result schema without evaluating their numbers in this graph.
An edge retains the concrete body instance or checked primitive role, direct/value kind, explicit argument-to-input map,
environment channel and concrete input/result obligations. A named value supplies a code token without a
body environment; a captured value supplies its exact carrier and environment payloads to the declared body
slot. Original body invocation and supplied adapter view remain distinct. Normal completion either
retains the environment, leaves remaining-payload cleanup with a consuming body, or disposes an
adapter-owned environment before result delivery. Copy supply refers to its prepared snapshot,
not its original source owner. No unwind/cancellation guarantee is inferred from this normal exit
recipe. Explicit bindings retain their ordinal, operation and payload; body-local identities remain
with the body's input layout instead of being duplicated in every caller edge.
Binding retains the exact selected callee frame instead of reducing it to code identity. An
adapter-owned captured environment has separate entry payloads and normal-exit carrier/environment
contracts. Exit metadata uses the original body's environment schema: independently closed
metadata needs no completion value, while a dependent schema uses checking's completed-input
query. The canonical payload authority rebuilds the full callable with that environment.
`DisposeBeforeResult` and its lifecycle step retain both completed contracts; storage interns
both. This selects no new owner or source permission, and dependent environments with no qualified
normal completion remain unresolved.
Lifecycle forwards each checked transition's post-operation phase to checking's common resource
data projection. Edited temporary cleanup therefore selects completed argument definitions instead
of the preparation snapshot. Preparation contracts, before-operation transfers and interrupted
prefixes retain their entry sources; this does not qualify changing storage geometry.
Final freeze verifies each edge's environment, input and result payloads against its selected
concrete body or the runtime contract's sole primitive value signature. Compound returns preserve
the body's and caller's original contracts while comparing their actual component representations;
equal callable signatures or geometry cannot substitute another code/environment context.
Input locals are relative to the selected body and a fresh invocation activation; sharing a
static instance never shares runtime environment values, owner state or storage between calls.

Checked bodyless primitives create no source instance, local slot, lifecycle body or synthetic
callee environment. Named primitive witnesses can pass through ordinary callback instances and
retain their normal code-token adapter cleanup. A bodyless declaration cannot be a source-body
root. This static closure supplies neither the primitive's physical runtime ABI nor native
execution authority; downstream consumers must qualify that operation explicitly.

Each instance retains its exact accepted body and these frozen edges/data maps. Consumers cannot
recheck a body, select by names, infer an environment offset from arity, recover source-level Copy
from concrete payloads, or reinterpret a closure's source owner as its code body. The graph retains
owned-capture producers and nested/factory-result closure bodies through that same code authority.
Physical input/environment realization, executable cleanup, ABI and backend admission belong to
downstream consumers and their qualified scopes. The
[private cross-stage contract](../../../architecture/pipeline/value-access-declarations.md) owns
the handoff; the [current realization plan](../../../design/compiler-realization.md) owns remaining
admission obligations.

## Private Lifecycle Schedules

`value_access::lifecycle::Program` freezes derived schedules over one retained InstanceProgram;
it has no parts constructor. `lower` enumerates reachable checked graph locations and merges only
identical guard/schedule pairs. A projected condition admitting different schedules rejects
explicitly, rather than selecting by report order. No second CFG or runtime-state store is built.

`lifecycle::payloads` consumes checking's
[ordered resource-data projection](../nocter-checking/README.md#shared-flow-access-and-resource-validation)
and retains each sealed operation/root pair in the native lifecycle vocabulary. It adds instance/action
failure context, not another source-value substitution model or owner registry. The original
transition still supplies before/after phases and ownership conditions. Scoped destinations are
matched by checking before data projection can change transition-local bindings. A match retains
the original operation and the instance's frozen before/after root contracts in one sealed
`ScopedReplacement`; other operations retain their resource form. MIR uses the independent retired
and supplied obligations with their respective virtual roots for disposal and transfer. The sealed
replacement also retains checking's complete original containing contracts, including source slots
and selection paths, on both sides. Additional storage roots include both virtual roots and those
containing contracts and dependent partial-source roots through the same canonical interner.
Root receipts stay attached to their original resource step; no frame-wide type map or later
phase guess supplies a local container. A partial owning move also retains its matched original
destination's whole-container before/after contracts; both enter the shared storage interner.
Enclosing allocation geometry and adapter completion remain
separate responsibilities; these contracts grant no mutation or layout proof.

`Program::prepare_with_budget` bounds contextual metadata work across the complete lifecycle
request. Failures retain the exact instance/action and instantiation cause and publish no draft;
the retained InstanceProgram remains reusable for a healthy retry. Ordinary body slots, invocation
channels, post-update dependent environments and storage must complete their separate occurrence
integration before that context-bound source scope can widen.
`completion` expands the frozen EnvironmentExit recipe only for reached calls; it does not
reclassify the supplied invocation or the body's environment authority.

`flags` selects facts that distinguish remaining ownership alternatives, shares those facts
across graph locations, and packs their tests/writes into word recipes. It skips redundant
facts and bodies with no choices. A bounded guard simplifier shares equal operation/write
sequences and removes complementary bit tests without changing those operations. The construction
index is discarded after emission. Changes
come from checked entry/intermediate/final snapshots, not an independent initialization solver.
Draft snapshots are consumed before the public body is frozen. Public alternatives retain emitted
bit guards; the checking report remains the owner of source ownership conditions. There is no
temporarily empty flag recipe in an accepted lifecycle program.

The [lifecycle handoff](../../../architecture/pipeline/value-access-declarations.md#private-lifecycle-schedule-handoff)
owns cross-stage ordering, answer protection and physical-consumer obligations. This private
product grants no released ExecutableProgram conversion, payload layout, ABI or native admission.

## Private Storage Projection

`value_access::storage` materializes declaration-owned `DataInstanceKey` graphs for an explicitly
selected runtime ABI. Its `Prepared` product keeps the closed RuntimeEnvironment and the matching
contextual interner together until MIR finishes consuming them. Context-free shapes retain direct
source identities; contextual carriers and closure environments receive local `RuntimeDataId`
identities. Function leaves use the canonical selected function representation, so a supplied value
and the receiving formal reuse one physical identity without relabelling their surrounding carrier.

`storage::prepare` receives the instance data roots and explicit additional `DataInstanceKey` roots
through that same interner. MIR supplies `lifecycle::Program::payload_instances`, including actual
resource payloads and adapter-owned environments. A frame's default TypeId entry cannot replace
those occurrence-selected keys. Requests for the supplied value and its normalized function
representation reuse one physical identity. Geometry-only consumers can provide no additional
roots without acquiring a dependency on lifecycle permission. Additional roots supply neither a
source value nor an executable code instance, and a failed/missing request cannot mutate an older
Prepared product.

The iterative graph walk selects every positive child by its declaration-validated field through
the same contextual instance authority. Function environments use their explicit environment edge,
not a source tuple field. Intrinsic representation primitives are independently closed roots.
Mixed component contexts preserve distinct callable environment identities and their logical payload
cleanup metadata even when the source callable type is shared. Constructing such a carrier is metadata
qualification, not execution of a source-level formal capture or an owned error's destruction.
Complete callable environments retain that same mixed carrier key through callable representation
and payload-environment projection, without rebuilding captured substitutions from a frame-wide map.
Optional and fallible payloads request canonical tag/comparison primitives; fallible shapes also
request the built-in error identity. Aggregate representations retain exact logical component keys.
Machine owns size, alignment and member offsets. Unproduced contracts add no stored type, and empty
arrays retain only their source element metadata without binding or materializing a positive element.
Body signatures and MIR payload slots resolve through the prepared interner rather than inventing
independent substitutions. Checked payload/access/cleanup evidence remains a separate prerequisite.
No released executable adapter is introduced.

Outcome interning retains a declaration-owned uninhabited primary as `RuntimeType::Uninhabited`
metadata without inventing a static callable. Ordinary root interning still rejects empty domains
before reserving an identity; only the owning outcome edge admits that metadata child. The
interner shares one empty runtime domain while preserving every semantic key upstream. The current
fallible runtime descriptor preserves the ordinary error alternative and rejects specializations
that exclude it. This stored-shape capability does not admit producer-less source returns or
grant their lifecycle/guard proofs.

This materializer and its nested-context tests do not complete formal-environment source support.
The producer and closure-creation paths still need qualification before that source admission can open. A representation key supplies neither a
checked payload nor a new destruction or callable-publication rule.

The [private stored-representation contract](../../../architecture/pipeline/value-access-declarations.md#private-stored-representation)
owns the representation policy and remaining realization obligations.

Whole-array source values retain their homogeneous declaration schema and payload obligations
through instance-data and lifecycle projection. Positive lengths visit element obligations once;
zero lengths add no element-payload edge. This logical closure grants no stored representation or
ABI evidence. Stored-shape projection closes nonempty arrays into the shared homogeneous
`RuntimeType::FixedArray`, visiting the element once. Zero-count storage uses the payload-free
`RuntimeType::EmptyArray` with the same established element identity; it adds no element runtime
entry, layout request or representation fields. The [native consumer](../nocter-machine/README.md#private-array-execution) owns current
executable qualification; normalization alone grants no layout or executable proof.
