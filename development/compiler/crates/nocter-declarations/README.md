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
immutable `Program` views containing only ready entries. Each contract first closes its positive
by-value payload dependencies. Its consuming `finish` closes every remaining callable and record, including unused
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
input/result contracts.
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
`TypeId` equality; callable nodes constrain an already selected witness. Matching resolves the
pattern against that witness's own input/result facts and delegates exact contract checking to
`constrain_function`. It creates no type, code or environment identity. Only the outer invocation
view can adapt; nested callable contracts remain invariant. Failures retain their input/result
path independently of source coordinates. Normalized contextual contracts retain exact data
identities rather than expanding those identities into anonymous structural signatures.
Named witnesses are capture-free Copy values with trivial cleanup and no outside D/S
under their exact schema. This is not evidence about arbitrary structural callback contracts.
Formal and unresolved result witnesses deliberately have no Copy evidence. Their environment
cleanup remains symbolic; a formal cannot inherit payload properties from one currently known caller. `function_contract`
projects the selected named, formal or result contract without searching for an implementation.
Checking must establish each result family's named/captured-code or own-input witness relation and substitute
actual bindings at each application before body acceptance. A result header never invents a
producer or its payload properties. Distinct formals retain distinct witness
identities even with equal structural signatures. Released
declaration validation rejects all private witness keys.

`body_types` owns the narrow body-type construction capability. A selected factory call interns
its caller/operation/callee key and all permitted views over its immutable ready-header prefix. A
construction-only reservation forbids selecting two callees for one operation within the same lexical
`ValueCode`. A root and its closures have independent operation arenas and reservations. It is discarded
on freeze; no dispatch search or source syntax enters the program. Branches share declaration,
namespace and record metadata instead of cloning those authorities.
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
schema components. `owned_payload_supported` owns the common closed payload domain for environments, tuples and arrays:
closed builtin/record/tuple/array data and exact named/captured callable payloads. Formal and
unresolved result/application witnesses supply no such payload evidence. A valid pending result
may occur in an environment descriptor, but source admission must establish its closed payload
before execution. Copy and recursive cleanup
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
environment, a Copy proof nor an executable payload. Formal callable components remain outside
this structural construction domain.
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

Outcome/capability/generic/user-destructor schemas, nested callback contracts and general captured
environments remain outside the accepted private fragment. Exact
scalar/unit/error independence is checked under its owning data contracts, never inferred from Copy or value receipt for arbitrary future types.
