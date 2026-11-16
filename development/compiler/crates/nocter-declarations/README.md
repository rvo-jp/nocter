# nocter-declarations

## Responsibility

Own the immutable, syntax-independent declaration graph and its namespaces, visibility, callable,
requirement, standard-role, and target metadata. Define the sparse structural-constant and dense
final-value products without evaluating either one.

## Contract

The crate supplies domain types, builders, and validated immutable products. It knows semantic
identities and normalized declaration relationships but not source text, syntax trees, editor
coordinates, or checking internals.

## Internal Responsibilities

- declaration and member arenas
- the bounded future private-function declaration authority under `value_access`: canonical
  reservations, define-once callable contracts, input operations, namespace validation, and resolved
  value results, normalized callable contracts and recursive annotation constraints, without
  borrowing/source-syntax representations
- closed nominal payload schemas under `value_access::records`: define-once records, resolved
  field identities and types, local member namespaces, finite-containment validation, and
  explicit structural Copy evidence, published in that same private program/type authority
- one sparse structural-constant table and one independently validated dense declaration-value
  table, both separate from constant and static metadata
- module, import, and prelude namespaces
- callable execution, guarantees, their closed compatibility policy, canonical input/result
  provenance graphs, constant, and requirement contracts
- explicit block or expression body forms, with constant and static initializers represented by
  ordinary semantic body identities rather than embedded syntax
- canonical interface dependency paths, `Self`-inheritance closure, and effective member identities
- visibility and path contracts
- accepted/recovery admission shapes

## Invariants

- Builders reserve and define every declaration identity exactly once before metadata freeze.
  `PreparedDeclarationProgram` owns a sparse structural-constant builder; it is a construction-only
  capability and cannot cross into checking. Its consuming finish transition validates that table
  and publishes checking admission without pretending ordinary initializers are already executed.
- Graph/type integrity, structural-constant integrity, and final declaration-value integrity are
  validated independently.
- Namespace lookup consumes frozen tables rather than iterating declarations.
- A selected import is validated against the exact authored name/target pair in its target module
  namespace. Re-exported targets may retain an original declaration owner in another module; the
  import validator does not confuse physical declaration ownership with exported surface ownership.
- A declaration identity never contains a source range or rendered name.
- Authored callable execution and guarantees are declaration data; consumers do not rediscover
  modifiers from syntax or result shapes.
- One declaration-owned policy classifies incompatible callable kind, execution, and guarantee
  combinations. Checking projects that source-independent violation to diagnostics; consumers do
  not maintain parallel modifier matrices.
- Generic application validation compares one ordered parameter-domain schema with one ordered
  value sequence. Callers cannot substitute a type-only arity rule.
- Callable input provenance stores resolved receiver and parameter identities. Source spellings,
  header walk order, and body inference cannot enter that graph.
- Invalid or incomplete graphs cannot be constructed as accepted programs.
- `DeclarationProgram` contains graph/type metadata only. Declaration records never embed
  evaluated constant or static payloads. Accepted and recovery declaration branches carry only
  structural constants; `CheckedProgram` later owns the dense final value table.
- Every constant and immutable static owns exactly one expression-form body. Every callable,
  destruction, and test implementation owns a block-form body. Integrity validation proves both
  directions of each relationship before the graph can leave construction.
- An accepted immutable program may create owned checking branches without rebuilding declaration
  decisions; every branch preserves semantic IDs and the type-authority lineage.
- Interface prerequisite cycles and effective member collisions cannot cross the accepted-program
  boundary.
- Dependency-cycle validation follows every interface predicate; member inheritance follows only
  contextual `Self impl Interface` predicates.


## Future Value Declaration Boundary

The value-access product owns immutable declaration facts and their type authority for the future
private core goal. Header facts are shared by immutable body type branches and canonical replay. It is not
an adapter to `DeclarationProgram` or accepted by the released target/checking pipeline.
`Reservations` assigns code/record and input/field identities; its consuming `finish` closes
the complete name domain into a `Builder`. This construction owns define-once raw definitions;
its infallible `into_publication` transition opens `HeaderPublication` without requiring definitions
to be loaded. The writer retains that sole raw owner and permits defining remaining headers through
the same define-once methods. It cannot append reservations or replace an established definition.
This single writer validates and normalizes requested callable or record contracts and exposes
immutable `Program` views containing only ready entries. A record contract first closes its positive
by-value payload dependencies. Callable headers need validated signature identities, not the payload
schemas of their inputs/results or of nested callable signatures. Reserved nominal keys provide
those identities before record counts or fields are ready. Source/body checking, invocation and ABI
consumers independently require the actual payloads they use. Its consuming `finish` closes every remaining callable and record, including unused
records. `Builder::finish` uses that same path. `record_identity` and `declaration_label` can
identify/name a reserved nominal before its payload schema is ready; they grant no fields,
containment, Copy or payload-readiness evidence.
No authored definition or type resolution is available during reservation, and no new declaration
can be reserved after name closure. Untyped construction grants no body-checking admission. A ready view
can support typed-only checking of its established contracts, but only `headers_complete` can
permit whole-unit closure. `is_defined` distinguishes known unloaded definitions from unknown IDs;
it does not grant a validated contract. Raw reference, payload and Copy validation all run on demand.
That boundary is not early array-length evaluation. Defining headers
in a different order cannot change parameter identities.
`HeaderPublication::has_same_reserved_layout` compares a current closed name/slot domain with a
complete retained program. It checks the common symbol and namespace facts, original callers and
audiences, and each code/parameter/record/field slot assignment. Equal total counts do not substitute
for equal ownership and positions. This predicate grants no signature agreement, source equivalence,
header readiness or type lineage; source adapters must establish their own reuse proof and keep the
retained program. Fresh structurally equal arenas never satisfy `preserves_headers`.

The lowering owner reserves functions and records in canonical namespace order.

Reservation takes each callable's input count and each record's field count, assigning the entire
slot domain in authored position order before any data type is resolved. `with_symbols` can share
the source inventory's exact immutable spelling owner; ordinary construction delegates to that same
reservation path. Construction and incremental-publication slot queries expose those identities,
not parameter contracts, field payloads or a partial program.
Definitions must match their reserved arity exactly; a wrong count leaves the slot/definition
state untouched and can be retried. A second definition still rejects without replacement.
Publication maps those exact reserved slots into typed parameters and fields; it never assigns
new identities during type normalization. Formal callable witness keys therefore use the same
input identities that were available before any definition. Missing definitions cannot publish
merely because their slots exist.

`namespace` constructs one immutable table keyed by `ScopeId`, `Namespace::{Type, Value}` and
spelling at reservation closure. Records and values may share a spelling; independent scopes may
also use the same spelling for distinct declarations. Duplicates in one scope and namespace fail
before type normalization. Functions, constants and statics occupy the value namespace.
`Reservations::new_scope`, scoped reservation and `bind` prepare that single table; equal bindings
have set semantics and aliases/exposure retain the original declaration identity. Bindings neither
reserve another declaration nor append a type. Named-declaration enumeration counts each canonical
reservation once, independently of how many scopes or aliases reach it, and unready presentation
retains its authored name. Unknown scopes reject before reserving slots and cannot obtain builtin
lookup as a fallback.
The same authority serves forward header references, parameter collision checks in the declaring
scope, and published lookup. Record schemas maintain no second global index. Explicit scoped
lookup has no implicit root or transitive fallback. `new_lexical_scope` creates a fixed child of an
existing scope; only these explicit parents participate in unqualified lookup and the first
qualified component. Nearest bindings retain their original identities, independent siblings cannot
see each other, and lexical ancestry inherits the original caller module. A child cannot replace
that caller with a more privileged module. Once a namespace link is selected, subsequent path
components search its exact target without lexical inheritance. Parent edges always point to
already existing scopes, so construction cannot create a cycle and deep lookup remains iterative.
This semantic mechanism alone does not admit source-local block imports. The single-source convenience methods use only
`ScopeId::ROOT`. Reserved identities grant no schema or function-value readiness. Source inventory,
physical visibility, contract/body contexts and authored import policy remain the lowering owner's
work. `Scopes` constructs source-independent namespace links and original caller-module facts before
that same name closure; links allocate no code, record, payload or type. Namespace names conflict
with authored type/value names and parameters, while type/value names remain independent.
`bind_export` shares an immutable normalized audience with an outward declaration binding;
`bind_namespace` can attach an audience to one path step. Invalid construction does not replace an
existing module/link. Equivalent audience sets are equal binding requests; different audiences
cannot silently widen an existing binding.
`declaration_path_in` walks a finite path iteratively, retains the original caller across every
link and checks both intermediate and final audiences. A target scope's own module never lends its
privilege to the caller. Failed paths report their exact component position, without fallback or
header readiness; unknown caller identity cannot pass a restricted boundary. Explicit outward
`declaration_visible_from` uses the same audience predicate. `declaration_prefix_in` uses the
same traversal to stop at an actual declaration and return its consumed path length; record/tuple
members remain checking's responsibility. An uninterned spelling is represented by `None`,
never a fabricated symbol. A suffix after the declaration need not exist in the name catalog. Source path spelling, lexical block scope and ordinary v2 discovery/CLI admission remain
separate obligations.

Callable/input and record/field tables use model-owned sparse arenas. A missing entry has no data, invocation
contract or Copy evidence; known names can still identify its required header or forbid shadowing.
Only a non-cloneable writer can fill a reserved slot once. Immutable views share persistent storage;
their exact ancestry is independent of structural metadata equality. All views retain the same
closed namespace and preserve every established contract. Nominal leaf keys alone cannot expose
an unready payload schema or Copy evidence. Named invocation keys publish with their selected
validated callable contract, not during reservation. Record publication
reports missing record contracts through `HeaderPublication::advance`. A step publishes only the
selected contract once every dependency is ready; a request publishes neither provisional parent
fields nor TypeIds. The complete publication entry drives those same steps with an explicit
[construction dependency query](../nocter-dependency-query/README.md). There is no competing eager
normalization or containment validator. Each shared child normalizes once before
its parent's complete fields and local member namespace. Reserved callable and declaration
enumeration belongs to this same authority, independently of source projection and readiness.

`value_access::data` owns closed data-name resolution, TypeId projection and Copy queries over
one privately owned type authority. Callable metadata headers admit `bool`, the intrinsic integer catalog, `void`, opaque move-only
`error`, program-extent text descriptors, closed nominal records, structural tuples, optional/fallible layers and closed array descriptors, plus flat callable
input/result contracts. Construction metadata also admits callable leaves inside compound input
and callable-result descriptors; ordinary source admission remains a separate checking/consumer obligation.
`CodeKind::Primitive` is a runtime-only bodyless callable metadata role. It shares ordinary
static function-witness publication through `is_callable`; source authorization and exact
primitive binding remain declaration-lowering obligations, independently of witness shape.
Construction-time
`ValueDefinition` slots share data or callable normalization. `DataReference` composes named
leaves into minimum-two-component tuple trees, compact runs of optional layers, fallible carriers
and closed fixed arrays. A fallible schema maps its primary component to the declared payload and
its secondary component to built-in `error`; it never derives a branch from the payload's type.
Fallible data is non-Copy even when its success data is Copy. Header validation rejects repeated
failure on an uninterrupted outcome chain, including beneath optional layers or zero-array
metadata. A tuple/array component begins its own chain. Body construction, witness substitution
and canonical replay retain these constructors in the same type authority. The
[checking outcome path](../nocter-checking/README.md#outcome-values) owns qualified source operations;
schema admission alone does not establish evaluator error graphs or executed failure cleanup.
The admitted `str` domain grants no Copy proof. Its current producers have immutable
program-extent UTF-8 backing; general string owners and externally dependent text views need
their own qualified payload contracts.
Optional layers reuse the model type authority and retain positive payload containment/Copy
requirements even when an actual value is absent. Array lengths are already
normalized u64 target-usize values, not source expressions or symbolic guesses. One iterative
resolver interns their TypeIds in the same declaration transaction before publication. They do not expose released type-store eligibility
helpers or borrow constructors. Unit is a value in this product. Input operation is
separate from data identity and payload Copy evidence. Stored parameter records carry a data
TypeId and Read/Edit/Take operation; callable results carry a data TypeId and, for a callable
result, its structural constraint. The authored compile-time capability is a separate callable
header fact; it proves neither body-domain validity nor successful evaluation. `CodeKind`
distinguishes source functions, constant initializers, immutable-static initializers and anonymous
required type expressions in the same `AccessCodeId` arena. Initializer roles derive their
compile-time capability, reject function inputs/results,
and grant neither a function namespace entry nor a named-function witness. A `ValueStatic` key
identifies the static's place separately from invoking its initializer code; `static_declaration`
validates that relationship through the sole declaration table. No parallel static metadata or
namespace owns its name/type. Neither introduces a result access operation. No result
mode/referent set or old read-result normalization remains.

`reserve_type_expression` reserves anonymous code against an existing parameter, result or record-field
slot. Its source-independent `TypeExpressionSource` records that exact declaration/position;
unknown, non-type and anonymous-owner slots reject before allocation. Its define-once contract has
no inputs and a usize result. It is construction metadata, not an authored definition or a resolved
array type. `Code::name` is absent, the closed namespace gains no entry, and no function witness is
published. `code_ids` includes these roots although named `declaration_ids` does not; final header
closure still requires every code slot. A selected anonymous contract can publish before its
owning authored header is ready, without making that parent ready. Presentation derives a label
from the real owning declaration, including an unready owner, rather than interning a synthetic
name. The [source owner](../nocter-declaration-lowering/README.md#exact-source-evidence) pairs these
contracts with exact executable roots and supplies source-computed lengths with their required
supplier evidence. Array value operations remain part of the
[structural request work](../../../design/compiler-realization.md#structural-type-request-closure).

The declaration product also admits closed fixed-array metadata as `PayloadSchema::Array` with an
`ArraySchema` element TypeId and u64 cardinality. It reuses the model's `FixedArray` identity and
closed `UsizeTerm`; array cardinality never expands metadata or becomes a tuple identity. The
schema does not grant tuple/record component selectors or runtime value operations. `component`,
concrete `project_data` and generic `project_data_by` share one schema projector. The latter
consumes caller-owned bounds evidence for an opaque index without substituting a guessed number.
An empty array never admits an element projection, even with a permissive callback. This metadata
query grants no access, ownership, payload readiness or executable bounds proof. Structural
Copy is true for a zero count, otherwise it consumes the element's evidence once. A zero-byte
non-Copy element with positive count remains non-Copy.

Raw type validation still visits every element name and storable-type leaf, including inside empty
arrays. Finite containment and explicit Copy consume the same separate payload-leaf traversal,
which omits proven zero-count element edges. Diagnostic closing-field selection uses that same
policy. Normalization interns validated nominal name keys without requiring their payload schemas;
this lets a zero array break self containment or refer to another unready record. It does not make
that record ready or valid. Complete closure still rejects independently invalid unused
declarations. Positive counts require the ordinary ready element payloads. Copy and containment
rejection occur before publishing any parent field or array type.

These are declaration metadata contracts. Source normalization feeds closed counts into this
same resolver. Array value checking, resource cleanup, const-data publication and runtime
realization remain separate admission gates;
the [source owner](../nocter-declaration-lowering/README.md#exact-source-evidence) states the current
source boundary.

The same program publishes closed nominal record metadata used by parsed callable and body
checking. Record names use the common closed namespace above. Fields have a distinct value-field
identity domain. Record
schemas retain declared field order, resolved TypeIds and a frozen local member namespace.
Definition order cannot affect those identities. Name collisions follow the scoped type/value
namespace contract above.
Requested payload validation visits nominal containment edges, including those inside tuple
fields, without host recursion or payload expansion. An active cycle rejects at its closing field.
Once dependencies are ready, explicit Copy validation consumes their established evidence before
opening any type transaction or publishing fields. Invalid Copy declarations cannot publish as
ordinary non-Copy records to satisfy a parent. Ordinary records never acquire Copy implicitly.
`HeaderFailure` distinguishes unknown identities, declaration-owned rejection with exact header
positions, and construction inconsistency. A failed prerequisite leaves valid completed siblings
available but grants no ready schema, field or newly interned structural type for the rejected
contract. The fallible final closure still rejects unused invalid declarations. Standalone
construction and source-driven steps consume these same rules.

`Program::existing_data_reference` resolves an already interned structural identity through the same
iterative data-reference fold used by publication. Missing shapes return no identity and do not
extend the snapshot. Finding a reserved nominal identity still grants no payload or Copy readiness.
The source owner uses this read-only query to reconcile actual required header values with the
already published data, without constructing another type authority.

`aggregate` exposes one ordered component schema for records and tuples. Record components retain
their declared field identities; tuple components retain their positions. `AggregateForm` projects
the explicit record or tuple family from that validated schema. Family-specific consumers match
this view; absence of a record projection is not permission to assume tuple semantics. The view
cannot construct a new schema or choose another type authority. `component` and
`project_data` validate each selection against its exact containing shape, without looking up names
or inferring access permissions. A tuple position cannot select a nominal field or vice versa.
An empty path preserves its validated data type; a scalar projection or a field from another owner
is rejected.
Concrete `ValueField::Array` projections validate the u64 index against the homogeneous descriptor
and select its element type without expanding components. They cannot select tuple/record data,
and tuple/record selectors cannot select arrays. `Component` retains only the selected logical key
and data. Packed record/tuple ordinals come from `component_position`; that separate encoding has
no array case. These metadata projections establish neither dynamic-index relations nor access
or initialization evidence.

`data_schema` exposes builtin, aggregate, fixed-array and named/formal/result/application witness schemas from
the supplied program's sole type authority. It never falls back to a different header/body store.
`data_copyable` consumes that validated shape with iterative memoized structural traversal;
`data_kind` deliberately remains a builtin-only projection. These APIs do not publish arbitrary
borrow types, generic eligibility guesses, or a second type store. Ordered record/tuple aggregate
schemas remain distinct from the homogeneous fixed-array descriptor.
A tuple is Copy exactly when every component is; it does not make an ordinary nominal component
Copy. These schemas are metadata, not proof of body acceptance, partial initialization, survival, destruction or
native execution. Selected source places consume these schemas through the private body checker;
only closed unit acceptance establishes the combined body/flow/resource proofs.

`value_access::functions` owns named/formal/result/application witnesses and structural contracts as distinct
products. `publication` normalizes input and result slots through one value publication path in the
same header TypeAuthority. Named functions publish all invocation views; formal and result families
publish only the views their declared invocation can supply, independently of body use. A local
annotation validates exact input operations, data and result, then selects a permitted view of that same witness. Typed mismatch domains and the view
adapter policy belong here; consumers cannot substitute another declaration with an equal
signature. `FunctionConstraint<Data>` uses the same ordered contract shape for normalized data
identities and recursive `FunctionPattern` annotation constraints. Pattern leaves require exact
`TypeId` equality; callable nodes constrain an already selected witness. Structural pattern nodes
retain optional/fallible payloads, ordered tuple fields and homogeneous arrays without manufacturing
callable data identities. Matching resolves each node against the selected witness's own facts,
retains every original code/environment identity and delegates exact outer contract checking to
`constrain_function`. Shape/leaf failures retain the original component path and array cardinality.
Pattern matching creates no type, code or environment identity. Only the outer invocation
view can adapt; nested callable contracts remain invariant. Failures retain their input/result
path independently of source coordinates. Normalized contextual contracts retain exact data
identities rather than expanding those identities into anonymous structural signatures.
Abstract type-contract binding has a separate relation: `constrain_callable_contract` compares
nested abstract signatures through the canonical data graph while retaining every original
contract, formal and producer identity. `function_binding` and instantiated function receipt use
this relation only for a receiving `Contract` origin. The exact normalized `constrain_function`
and pattern-leaf identity checks above retain their existing behavior. Only the outer invocation
view may adapt; nested invocation views and input operations stay invariant. Concrete nested
witnesses keep exact code identity, nominal types keep identity, and even zero-length arrays keep
their declared element contract. A shared pair traversal and parent-linked diagnostic paths avoid
expanding deep shared signatures or large array cardinalities. This comparison creates no type,
concrete witness, capture environment, payload or const capability.
`constrain_declared_function` uses the same graph comparison for authored header constraints, but
opens only expected `Contract` nodes. Normalized formal/result/application and concrete leaves
remain exact. This separate relation serves original header-input bindings, contextual function
receipt and checked result substitution; `constrain_function` and exact pattern leaves retain
their stricter behavior. Ordinary top-level leaf mismatches preserve input/result diagnostics;
nested contract failures retain their original path. Higher-order invocation still consumes
actual source-context evidence independently of this signature relation.

`Program::callable_input_parameters` is the sole classifier of direct callback inputs by original
explicit ordinal and actual code-owned formal ID. Ordinary frame binding and source-instance
selection consume this same classification. Named code uses its reserved physical parameters;
anonymous code uses its lexical input positions. Capture environments stay outside those ordinals,
and nested type contracts allocate no extra parameters. Unknown lexical input ownership rejects
before constructing a frame. Signature matching alone cannot supply the actual argument values.

Named witnesses are capture-free Copy values with trivial cleanup and no outside D/S
under their exact schema. This is not evidence about arbitrary structural callback contracts.
Anonymous callable inputs use `ValueParameter::Closure` with the lexical closure and input position;
header inputs use `ValueParameter::Declaration` with their actual reserved parameter slot.
Callable leaves inside a compound input use `ValueParameter::Component`, keyed by `ValueInput`
(the reserved header parameter or the lexical closure and explicit input position) and the
descriptor's ordered callable-leaf ordinal. They allocate no physical parameters. Header inputs
own their component contracts, published atomically with their complete schema through the shared
iterative data-reference fold. Lexical components use the existing body-owned formal-contract
registry; closure publication and canonical replay validate every lexical formal against its exact
input position and ordinal, rejecting foreign, missing and surplus formals. Shared subgraphs are
summarized once; exact code/result dependencies in internal compound schemas retain their identity
without expanding their logical paths. Arrays contribute one homogeneous element schema, regardless
of cardinality. The environment prefix is not an explicit input position. Equal signatures retain
distinct formal identities, including separate tuple positions.
These descriptors supply neither Copy nor owned-payload evidence. Canonical compound-input keys
retain the concrete witness at each component instead of adding those witnesses to direct input
bindings. Raw callable data at an entire input must use the existing `ValueDefinition::Function`
form. A declared result uses one `Code::result_contracts` row, addressed by
`ValueCallableResult`'s declaration and callable-leaf ordinal. Root callable results occupy the
sole slot zero; compound results retain separate same-signature leaves without physical inputs.
`callable_result_path` selects an exact published result identity in that declaration's original
descriptor. Its array selectors name homogeneous element schemas, including uninhabited zero-array
metadata; they prove neither bounds nor an initialized element. The read-only traversal preserves
outcome layers and visits shared schemas once, independently of array cardinality.
`matches_uninhabited_metadata` compares declaration-owned data contracts while permitting
signature-equivalent callable descriptors only below an empty array. Positive leaves and nominal
identities remain exact. It grants no element value or Copy evidence to the element itself.
`matches_uninhabited_metadata_with` additionally reconciles callable leaves when both have an
independently proved empty domain and the same callable contract. One empty side cannot substitute
a produced callable. Concrete data-instance comparison uses that rule only after both keys have
explicit empty domains; it does not infer emptiness from missing code or a matching signature.
`matches_guarded_transport_metadata` uses the same contract traversal for directed symbolic
transport: an independently excluded source callable can widen beneath an outcome guard, while
narrowing, unknown sources and differing signatures reject. This remains a symbolic contract
check; concrete contexts and executed transfer require the
[guarded transport contract](../../../design/compiler-realization.md#guarded-representation-transport).
`BodyTypes::application` publishes independent applied-result templates and rebuilds their
containing structural types in one atomic extension. These are descriptors of one original call,
not extra invocations or concrete results. Empty arrays publish no applied element template;
remaining leaves preserve their declared ordinals. Reusing a call key cannot replace its template
set, including ordinal gaps; original header contracts remain immutable.
The same iterative reference fold publishes these types and reconciles existing input/result
recipes under their actual owners. Metadata does not close a result's producer: it grants no
concrete witness, Copy, payload, layout or cleanup. Initializers, record fields and higher-order
component signatures retain their separate rejection boundary, including inside zero-array metadata.
Checking owns the
[qualified source input scope](../nocter-checking/README.md#context-bound-callable-captures).
`BodyTypes::closure_parameter` publishes each anonymous formal's structural signature once, before
body checking; compound leaves use `closure_component` under the same registry. A signature may
retain a reserved nominal identity without demanding its payload header. Contract matching and
lexical-formal validation preserve that distinction, including an unstored zero-array element.
Neither operation establishes an input value, payload layout or invocation availability.
The completed closure signature refers to its own formal descriptors. Older snapshots
cannot acquire the new signature, and canonical replay rejects a formal without a completed owner
or the matching input position. Formal signature publication supplies no owned environment.
Only ordinary input/result data within these anonymous input signatures is currently qualified.

Formal and unresolved result witnesses deliberately have no Copy evidence. Their environment
cleanup remains symbolic; a formal cannot inherit payload properties from one currently known caller. `function_contract`
projects the selected named, formal or result contract without searching for an implementation.
Checking must establish each result family's named/captured-code or own-input witness relation and substitute
actual bindings at each application before body acceptance. A result header never invents a
producer or its payload properties. Distinct formals retain distinct witness
identities even with equal structural signatures. Released
declaration validation rejects all private witness keys.

`FunctionWitness::substitute` owns mechanical one-step substitution through frozen formal bindings.
It preserves the requested invocation view and rejects a weaker supplied view, even when its
underlying declaration supports a stronger operation. Direct code origins need no lookup; missing
bindings and direct result/application terms return no selection. Bound symbolic terms are not
followed recursively. This operation creates no type, chooses no target and grants no executable
or payload proof. Consumers retain identity-domain and binding-coverage validation, with their own
conservative-analysis or closed-execution admission boundary.

`code_bindings` owns canonical code-frame metadata and concrete static witnesses. `bind_code`
requires exact direct callable-formal coverage, rejects duplicates and foreign identities, and
reuses the common signature/view normalization authority. Bare code witnesses cannot stand in for
closures whose environments require captured callable substitutions. `bind_callable_environment`
binds exactly those dependencies from the canonical environment descriptor; pending producers
cannot supply a closed context. Distinct frozen views of one formal share the strongest required
view without acquiring a stronger capability than the supplied witness.

A `ConcreteWitness` retains its code, selected invocation view and complete contextual environment
as a `DataInstanceKey`; there is no competing flattened capture map. `bind_callable_data` retains
an exactly typed environment, including different contexts at same-source-type component positions.
Environment validation checks present edges and permits an omitted edge only when its declaration
schema has no contextual dependency. Missing positive contexts and noncanonical stored closed
edges reject; empty arrays still require no element context.
`bind_callable_environment` is the exact free-parameter binding constructor and normalizes through
that same environment authority. Nested witnesses retain their own contexts. `bind_instance` receives those complete witnesses;
forwarding two values of the same lexical closure can therefore retain different captured callbacks
in the same receiving frame. Direct input bindings remain separate from the environment. `resolve`
derives only requested free bindings from that environment; conflicting occurrences remain unresolved,
while exact component selection retains each context. Own direct inputs do not require flattening
unrelated captures. Selection preserves frozen views without repeating lookup or body checking.
Closing a template over its own code frame retains that frame's environment directly.
`bind_instance_with_inputs` additionally retains received compound-input contexts at frozen explicit
input ordinals. It requires exactly the inputs whose schemas need contextual data; direct callable
inputs keep their witness bindings, while independently closed inputs, including empty arrays,
need no extra receipt. `compound_input_contracts` owns that classification for producers and frame
validation. Their complete canonical data keys participate in frame equality and order,
so equal code, direct bindings and environment cannot merge distinct received compound contexts.
Input validation shares the iterative environment/witness validator; a matching outer schema alone
cannot admit missing children, unresolved positive dependencies or noncanonical stored edges.
`CodeInstanceKey::input_data` selects this metadata without flattening its nested callable contexts
into free-parameter bindings. Symbolic template closure supplies no compound-input receipts, even
for the same code: those require the original supplying bundle. The checking owner qualifies
source-call and body-entry projection separately; this constructor adds no source or payload admission.
`instantiate_environment` selects that complete environment input, including an independently
closed environment when no contextual substitutions are needed. It shares environment selection
with callable data projection. `DataInstanceKey::project_path` preserves context through validated
fields; it cannot expose callable backing as source fields or manufacture an empty-array element.
Structural context participates in key equality and ordering. Validation and comparison use
iterative walks that visit shared contexts or context pairs once, including independently built
equal graphs. Diagnostic formatting emits indexed graph nodes once instead of recursively expanding
the same environment tree. Allocation identity only bounds those local walks; it is never the semantic order,
an instance key or a reuse receipt.

Expression/initializer frame metadata grants no callable role or executable body. Neither a key
nor a concrete witness proves const availability, representation, ownership, lifetime, primitive
authority or native admission. Consumers retain those separate proofs and must use the key with
its original declaration authority. A key is not a source/revision reuse certificate; caches remain
scoped to their checked program and body guards. The
[checking owner](../nocter-checking/README.md#context-bound-callable-captures) defines the qualified
source scope; additional contextual carriers and changing representations still need integration.

`Program::adapt_witness` is the shared concrete invocation-view operation. It validates the full
witness, preserves captured substitutions and rejects an upgrade from a weaker supplied view.
Formal resolution and producer-frame realization use this same operation; adaptation adds no
ownership, body capability or representation evidence.

`DataInstanceKey` pairs an unchanged semantic TypeId with a canonical shared component-context DAG.
Inhabited function leaves retain complete selected witnesses; carriers retain only contextual child edges,
so two fields of the identical source type can keep different captured substitutions. Context-free
children are omitted canonically. Equality and ordering walk shared graphs iteratively rather than
expanding repeated subtrees or array cardinality.
`instantiate_function_alternatives` retains a finite positive function domain in that same graph.
It validates every complete witness through ordinary function instantiation, sorts/deduplicates
full code/view/environment identities, rejects empty input and normalizes a singleton to the
existing single-witness key. `InstanceSchema::FunctionAlternatives` exposes the domain without
selecting an active value. Comparison, ordering and debug output share the existing iterative
context traversal. Single-function/environment selection and concrete frame binding reject a
domain rather than choosing a representative. Exact domain comparison and same-descriptor
metadata projection do not qualify positive injection, an executable selector, ABI or storage;
source execution remains unqualified. `function_representations` projects each complete case onto
its actual callable descriptor in canonical domain order, using the same concrete-leaf validator as
single representation selection. It retains distinct nested environments and invocation views;
non-callable and explicitly empty data supply no cases. This metadata projection grants neither
selection nor conversion. The
[positive alternative contract](../../../design/compiler-realization.md#positive-callable-alternatives)
owns those integrated obligations, including budgeted source discovery and healthy retry.
`join_data_domains` unions already validated domains under the same source descriptor. It uses
one iterative, memoized schema-pair walk and the same function-domain normalization. Explicit
uninhabited domains contribute no positives; function joins retain whole witnesses, and arrays
retain one element schema. A caller-supplied work step meters graph/alternative visits and memo
hits; binding and work failures remain distinct, and no partial result is published. Required
source discovery must supply its existing request budget. Component domains conservatively retain
possibilities; they do not replace original branch correlations or justify concrete transport.
Different source descriptors still require their existing validated reprojection first.
`has_same_representation` and `permits_guarded_transport` share that structural comparison. The
second relation is directed and permits only compatible excluded alternatives to widen; positive
callable contexts and nominal identities remain exact, and an empty root has no transferable value.
`permits_value_transport` adds a third, explicit injection mode over that same graph walk: each
complete positive source witness must remain unchanged in the destination domain. It permits neither
positive narrowing nor substitution of an equal-signature environment. Equality and guarded-only
queries retain their stricter meanings. Context-free comparison nodes use semantic descriptor
identity rather than newly allocated closed handles, so repeated closed children reuse the same
obligation instead of expanding shared schemas. Equal closed descriptors undergo one complete
validation in the requested program, then discharge that subtree without rebuilding every remaining
suffix. A Closed marker from a foreign graph cannot bypass unknown descriptors or unresolved
callable contexts. Deep closed carriers use this same comparison authority; evaluator consumers
need no local equality shortcut. None of these metadata relations implements a
conversion, active selector or byte-copy equality. The
[guarded transport contract](../../../design/compiler-realization.md#guarded-representation-transport)
owns the remaining integrated obligations before broader source admission.
`reproject` selects an equivalent existing declaration descriptor from the supplied instance,
preserving each occurrence’s actual code, environment and empty domain. It rebuilds through the
same canonical component constructors and visits shared context/descriptor pairs once, including
homogeneous arrays independently of their cardinality. It neither adapts callable invocation views
nor widens an outcome representation; a missing positive context cannot be supplied by the target.
Positive function domains reproject every complete alternative through the same single-witness
validator, preserving all contexts and exact invocation views. Normalization uses the same domain
constructor; a concrete-source mismatch in any alternative rejects the whole request.
`reproject_result` additionally matches a receiving result family to one selected declaration-owned
result path. A bounded schema-pair walk checks the exact result owner and leaf ordinal, including
applied templates, before the same projection authority retains the supplied concrete contexts.
Foreign or swapped same-signature leaves reject; empty arrays remain metadata-only. Opaque result
contexts use the existing canonical function/component representation and validation, without a
second context registry or provenance in instance identity. General `reproject` and
`instantiate_function` still reject an unresolved producer target. The qualified projection grants
no original return receipt, value, source conversion, invocation, ownership or physical transport.
An ordinary compatible empty-domain reprojection remains a representation operation; it does not
certify a foreign result-family binding.
`instantiate_data` uses the same binding normalization and frozen witness resolution as code
frames; it discards irrelevant input bindings. `function_representation` explicitly selects the
actual callable leaf's code/environment key while leaving its containing carrier unchanged. `InstanceSchema` exposes the canonical carrier
schema and complete concrete function witnesses. `component` selects a declaration-validated field
and filters its dependencies; a bare child TypeId is insufficient to identify that selection.
`instantiate_closed_data` constructs an independent context-free root through the same normalization.
`instantiate_function` retains an already resolved callable occurrence without reconstructing its
captured context from outer bindings. A formal leaf checks its structural contract and frozen view;
a concrete leaf also requires the same source code. A pending producer descriptor alone cannot
identify an occurrence. Frame-based leaf construction uses this same normalization.
`instantiate_components` assembles a carrier from complete, exactly typed component instances and
normalizes to the same identity as frame-based construction. Arrays supply one homogeneous element
schema at `Array(0)`; this does not describe distinct element values or prove a dynamic index.
Invalid fields and empty-array elements reject even when their type would otherwise be closed, while
closure-environment projection uses the selected value's own captures rather than the receiving
function's inputs. Two occurrences of one lexical code can therefore retain different nested
contexts. Empty carriers retain element identity without a positive element-payload proof;
pending producers and missing dependencies cannot be closed from their signatures. The key and
schema grant no payload, storage layout, access, const eligibility or source-generation receipt.
Evaluator and native consumers qualify their contextual representations through this common
projection before extending that source scope.

`instantiate_uninhabited_data` selects an empty representation domain for an existing semantic
descriptor. `InstanceSchema::Uninhabited` supplies neither a concrete function nor a positive
component, and remains distinct from an independently closed or unresolved descriptor. Required
tuple/record fields and positive arrays propagate an empty child domain to the whole product;
zero arrays remain inhabited. Optional data retains its absence alternative; fallible data remains
inhabited when either alternative is inhabited. Complete component metadata is still required:
omitting a child is not an empty-domain proof. Normalization retains the original semantic TypeId.
An empty root cannot become a concrete closure environment or received input. The evaluator
classifies it as unavailable value data, while an optional containing that domain can still
represent absence. Owned-payload conversion and native materialization reject an empty root.
This metadata capability supplies no source admission or initialization/tag proof. Guarded
inactive-alternative layout, lifecycle transport and ordinary producer-less callable outcomes
require separate integrated qualification.

`instantiate_callable_domains` uses the same iterative data-instance fold as ordinary witness
instantiation. Its resolver supplies a concrete callable or explicitly selects an empty domain;
missing evidence must remain an error. The fold retains each requested semantic leaf identity
and checks positive code, signatures, invocation views and environments through the ordinary
function constructor. Shared schemas resolve once and array cardinality never expands the graph.
This descriptor-level resolver is not an occurrence selector: distinct values with the same
lexical callable type still require their original source/component contexts. It grants no
owner, source acceptance, active-variant proof or payload materialization.

`CodeTemplateKey` retains concrete or validated outer-parameter witnesses for condition composition.
`code_template` builds a code's identity parameter frame; `bind_template` uses the same exact
coverage, signature and view-normalization routine as concrete `bind_code`. Its one-step `resolve`
keeps an unresolved outer parameter rather than choosing a matching code. `close` substitutes a
matched concrete outer frame and returns the same canonical `CodeInstanceKey` as direct binding.
Result/application signature terms cannot become producer witnesses through this API. Symbolic
keys grant no representation or body capability, and a restricted view cannot be upgraded when
its outer value later becomes concrete. `CodeBindingError` describes either metadata domain;
concrete executable consumers continue to require the stronger concrete-witness key.

`body_types` owns the narrow body-type construction capability. Every selected call returning a
callable interns its caller/call key and exactly the views permitted by its declared result. One
immutable `Program::application_template` fact owns that result witness template; the call IR owns
the selected callee. The template supplies structural constraints, not a concrete return producer,
owned environment or Copy proof. Named, anonymous and already returned callees share this path.
A root and its closures have independent operation arenas. Canonical replay preserves their
application facts in the same authority; no dispatch search or source syntax enters the program.
Application contract lookup follows templates iteratively and rejects cycles without a recursion
limit. Branches share declaration, namespace and record metadata instead of cloning those authorities.
Its owner is a complete `ValueCodeRoot`, including a body-local expression identity when selected.
Different expressions in one source body have distinct suffix, closure and application domains;
sharing their source owner cannot merge those domains. A declaration branch requires its ready
contract; an expression branch requires only its enclosing reserved code identity. The parent
contract need not close for an independent expression. Neither grants source expression pairing
or evaluator admission. The source
owner must establish that pairing before ordinary checking. Canonical replay preserves these
full roots while interning equal structural data as one type.
The same body construction authority reserves closure slots before their bodies are checked.
Nested checking advances that root's lexical cursor; publishing an inner schema cannot reuse a
pending outer slot. Publication and key exhaustion preserve existing reservations. This cursor is
discarded on freeze and supplies no runtime environment identity or ownership registry.
That branch also publishes owned closure environment descriptors under lexical
owner/slot code keys. A closure result may retain an existing named, lexical or pending application
witness in the same authority. Result metadata grants no producer, environment, Copy or invocation
proof; ordinary body/result closure still establishes those obligations. Callable input witness
introduction remains separate. Environment, input/result data and inferred invocation are distinct
schema components. `owned_payload_supported` identifies independently closed payload descriptors:
closed builtin/record/tuple/array data and exact named/captured callable payloads. Formal and
unresolved result/application witnesses supply no such payload evidence. Environment descriptors
may retain valid formal and producer dependencies. `payload_dependencies` derives each formal's
frozen invocation view and the exact unresolved result/application witnesses from the same
canonical schema traversal used by `payload_readiness`; it adds no capture registry. Distinct
application occurrences and invocation views remain distinct even when their signatures agree.
The pending status is derived from that producer set, rather than stored as a second fact. Shared subtrees are visited once, while empty
arrays introduce no payload dependency on their element. A formal descriptor alone supplies no
owned payload, even when it also has a pending producer. Descriptor publication preserves
those independent facts through canonical replay; source checking retains the supplying value
operations and consumers select the exact contextual instance before execution. Copy and recursive cleanup
follow the actual environment schema, independently of invocation. All permitted views belong to the same body-owned key.
Tuples and closed fixed-array descriptors use that same body construction authority, including
within a capture body. Its array constructor takes an established element TypeId and closed
u64 count; it neither evaluates a count nor expands elements. Empty arrays still require valid
element data. `type_identity_named` resolves the preinterned nominal key independently of
`type_named` payload readiness. Structural tuple/array descriptors may retain that key without
granting a payload schema, including nested descriptors beneath a zero-count edge. Actual payload
construction and selection still require their ready definitions. Structural descriptors may also
retain valid result/application leaves while their producers are pending. `payload_readiness`
distinguishes Closed, Pending and Unsupported over actual payload edges; empty arrays require no
element payload. `owned_payload_supported` accepts only Closed. A pending descriptor is neither an
environment, a Copy proof nor an executable payload. Structural descriptors can retain a formal
callable whose parameter contract belongs to the same authority. Tuple, array and outcome descriptor
construction share that validation; replay preserves the formal's original owner and view. This
does not close a concrete carrier, create an element value or widen executable array admission.
Equal header/body array shapes intern as one type. This metadata capability does not admit
array expressions, resource operations, evaluator array values or native storage.
Canonical replay remaps nested tuple/array components and every closure-schema data slot with the body
type suffix in the same program; headers remain unchanged. `closures` owns their immutable
projection, not body checking.

After checking establishes result relations, `resolve_body_types` validates the selected
contract-preserving witness substitutions and re-interns containing tuples/arrays in that same
canonical authority. Its transient projection covers all source type identities and closure-schema
data slots; immutable header/nominal metadata stays unchanged. Equal normalized shapes become one
type, including maximum-count arrays, without expanding their cardinality. Checking must project
every body slot and discharge construction/supply obligations before executable admission.
The same unpublished normalization checks result-induced captured-environment containment with
the shared dependency query. Tuples, positive arrays and captured payloads retain their actual
containment edges; zero arrays cut the element edge. Named functions contain neither their input
nor result signatures, and the header owner retains nominal finite-containment authority. Unknown
formal/result leaves gain no payload evidence. A positive environment cycle rejects atomically
before publishing resolved types; empty-edge cycles remain finite and may be Copy. Query-owned
frames and memoized shapes avoid host recursion and expansion of shared payload paths.

Canonical replay validates retained ready contracts, exact namespace and sparse-table lineage, persistent
header-prefix ancestry and each suffix's body owner. It records each body's own prefix boundary;
an explicit type-scope proof retains the owning code root even when its suffix is empty. A header
view, a body branch and a canonical merged product cannot substitute for each other's construction role.
Later header keys may occupy the same raw positions as an earlier body's suffix. The prefix is
proof input, never an alternative semantic lookup store. Replay re-interns witness keys and
structural tuples/arrays in semantic owner order and
publishes exact per-body TypeId projections with one final program. Coinciding raw suffix positions from two
bodies cannot substitute for each other. Header programs stay unchanged and reusable; an
extended program cannot masquerade as a fresh header base. Checking rewrites all body data
slots through this projection before closing relations.

`BodyTypes::callable_contract` constructs body-owned callable type contracts with a distinct
`ValueCallableContract` identity and complete invocation/input/result signature. Child contracts
may occur directly or inside structural signature data. Duplicate ordinals and unsupported data
reject before publishing a type or signature. One shared contract-data projection preserves nested
references through canonical body replay and result substitution, including raw-ID collisions
between independent body suffixes. Equal signatures do not merge different contract identities.
Deep nested metadata remains a graph of type references rather than an expanded value tree.

These contracts are not formal inputs, result producers or concrete witnesses. They gain no Copy,
owned-payload eligibility, environment, runtime layout or const permission. Unsupported lifecycle
and execution consumers reject them explicitly. Record-field publication uses the same contract
identity under each reserved field. Its structural fold publishes every nested contract, field and
type in one atomic step; invalid signatures or positive non-Copy fields in a Copy record publish
none. Zero arrays supply no callable payload. Payload dependency/Copy walks stop at the callable
value rather than following its signature, so a signature returning the containing record is not
a recursive stored payload. Each immutable field owns its shared signature graph; header snapshots
reuse the existing persistent field arena rather than cloning a global contract table. Header
lineage and body replay preserve these field contracts exactly. Read-only field-recipe reconciliation
uses that field's contract ordinals through the same structural fold as publication.
Ordinary callable headers publish signature children under their original parameter or result
owner. The direct callable input/result and its compound payload leaves retain their existing
formal/result identities; entering a callable signature creates type contracts instead of more
physical inputs or result producers. Each immutable header row owns its shared child-signature
graph, and the header transaction publishes the entire graph together. The same scope-aware
structural fold reconciles source recipes read-only, including complete direct callable headers;
source lowering does not maintain a second signature matcher. Header lineage and canonical body
replay retain these identities. Header construction alone does not qualify higher-order invocation,
body-dependent results, concrete environments or payload layout.

`Program::function_binding` checks actual callable values against these receiving contracts while
keeping the complete source code, invocation view and environment in the instantiated value key.
The stricter same-origin `function_adapter` relation remains separate. A raw contract still cannot
instantiate a closed payload. Payload dependency queries report contracts separately from formal
inputs and result producers; batched roots share one schema traversal. Original field construction,
forwarding and retained-read invocation now consume that value evidence through checking and native
execution. Wider compound higher-order and retained-reference invocation channels remain separately
unqualified. The
[realization contract](../../../design/compiler-realization.md#callable-type-contracts) owns the
remaining integration obligations.

Outcome/capability/generic/user-destructor schemas, general higher-order execution and general captured
environments remain outside the accepted private fragment. Exact
scalar/unit/error independence is checked under its owning data contracts, never inferred from Copy or value receipt for arbitrary future types.
