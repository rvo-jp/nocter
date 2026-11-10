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
  value results and optional flat callable constraints, without borrowing/source-syntax representations
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
`Reservations` assigns callable/record and input/field identities; its consuming `finish` closes
the complete name domain into a `Builder`. The builder defines and validates all source data
references, finite record containment and explicit Copy requests before opening `HeaderPublication`.
This single writer normalizes requested callable or record contracts and exposes immutable
`Program` views containing only ready entries. Each contract first closes its transitive record
dependencies. Its consuming `finish` closes every remaining callable and record, including unused
records. `Builder::finish` uses that same path.
No definition or type resolution is available during reservation, and no new declaration can be
reserved after name closure. Untyped construction grants no body-checking admission. A ready view
can support typed-only checking of its established contracts, but only `headers_complete` can
permit whole-unit closure. Raw source references and containment validation still close as one
stratum; deferred field-type publication is not early array-length evaluation. Defining headers
in a different order cannot change parameter identities.
The lowering owner reserves functions and records in canonical namespace order.

Reservation takes each callable's input count and each record's field count, assigning the entire
slot domain in authored position order before any data type is resolved. Construction-only slot
queries expose those identities, not parameter contracts, field payloads or a partial program.
Definitions must match their reserved arity exactly; a wrong count leaves the slot/definition
state untouched and can be retried. A second definition still rejects without replacement.
Publication maps those exact reserved slots into typed parameters and fields; it never assigns
new identities during type normalization. Formal callable witness keys therefore use the same
input identities that were available before any definition. Missing definitions cannot publish
merely because their slots exist.

`namespace` constructs one tagged callable/record name table at reservation closure. The same table
serves forward header references, parameter-shadowing checks and published role-specific lookup;
record schemas do not maintain a second global index. Invalid cross-domain names retain only a
temporary first-record diagnostic witness, never another lookup authority. Name failures precede
type normalization, so an invalid namespace cannot request or invent types to diagnose itself.

Callable/input and record/field tables use model-owned sparse arenas. A missing entry has no data, invocation
contract or Copy evidence; known names can still identify its required header or forbid shadowing.
Only a non-cloneable writer can fill a reserved slot once. Immutable views share persistent storage;
their exact ancestry is independent of structural metadata equality. All views retain the same
closed namespace and preserve every established contract. Named code or nominal leaf keys alone
cannot expose an unready function witness, payload schema or Copy evidence. Record publication
uses iterative postorder over validated containment; it normalizes each shared child once before
publishing a parent's complete fields and local member namespace.

`value_access::data` owns closed data-name resolution, TypeId projection and Copy queries over
one privately owned type authority. Callable headers admit `bool`, `i32`, `usize`, `void`, opaque move-only
`error`, closed nominal records and structural tuples, and flat callable input/result contracts.
Construction-time
`ValueDefinition` slots share data or callable normalization. `DataReference` composes named
leaves into minimum-two-component tuple trees; one iterative resolver interns their TypeIds in the
same declaration transaction before publication. They do not expose released type-store eligibility
helpers or borrow constructors. Unit is a value in this product. Input operation is
separate from data identity and payload Copy evidence. Stored parameter records carry a data
TypeId and Read/Edit/Take operation; callable results carry a data TypeId and, for a callable
result, its structural constraint. The authored compile-time capability is a separate callable
header fact; it proves neither body-domain validity nor successful evaluation. `CallableKind`
distinguishes source functions, constant initializers and immutable-static initializers in the same
code arena. Initializer roles derive their compile-time capability, reject function inputs/results,
and grant neither a function namespace entry nor a named-function witness. A `ValueStatic` key
identifies the static's place separately from invoking its initializer code; `static_declaration`
validates that relationship through the sole declaration table. No parallel static metadata or
namespace owns its name/type. Neither introduces a result access operation. No result
mode/referent set or old read-result normalization remains.

The same program publishes closed nominal record metadata used by parsed callable and body
checking. Record names use the common closed namespace above. Fields have a distinct value-field
identity domain. Record
schemas retain declared field order, resolved TypeIds and a frozen local member namespace.
Definition order cannot affect those identities. Record and callable names cannot collide.
Structural validation visits nominal containment edges, including those inside tuple fields,
from leaves to parents without recursion or payload expansion. It rejects non-finite containment
with a field witness and rejects an explicit Copy request containing a non-Copy component.
Ordinary records never acquire Copy implicitly.
Only validated metadata can be published; no partial schema authority escapes a rejected build.

`aggregate` exposes one ordered component schema for records and tuples. Record components retain
their declared field identities; tuple components retain their positions. `component` and
`project_data` validate each selection against its exact containing shape, without looking up names
or inferring access permissions. A tuple position cannot select a nominal field or vice versa.
An empty path preserves its validated data type; a scalar projection or a field from another owner
is rejected.

`data_schema` exposes builtin, aggregate or named/formal/result/application witness schemas from
the supplied program's sole type authority. It never falls back to a different header/body store.
`data_copyable` consumes that validated shape with iterative memoized structural traversal;
`data_kind` deliberately remains a builtin-only projection. These APIs do not publish arbitrary
borrow types, generic eligibility guesses, or a second type store. Current
aggregate schemas contain only values over the closed builtin leaves, records and tuples.
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
signature. Named witnesses are capture-free Copy values with trivial cleanup and no outside D/S
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
construction-only reservation forbids selecting two callees for one operation. It is discarded
on freeze; no dispatch search or source syntax enters the program. Branches share declaration,
namespace and record metadata instead of cloning those authorities.
That branch also publishes closed single-value closure environment schemas under lexical
owner/slot code keys. Environment, flat input/result data and inferred invocation are distinct
schema components. The environment is ordinary closed builtin/record/tuple data; Copy follows that
schema, independently of invocation. All permitted views belong to the same body-owned key.
Inferred tuples use that same body construction authority, including within a capture body.
Canonical replay remaps nested tuple components and every closure-schema data slot with the body
type suffix in the same program; headers remain unchanged. `closures` owns their immutable
projection, not body checking.

Canonical replay validates retained ready contracts, exact namespace and sparse-table lineage, persistent
header-prefix ancestry and each suffix's body owner. It records each body's own prefix boundary;
an explicit type-scope proof retains the owning callable even when its suffix is empty. A header
view, a body branch and a canonical merged product cannot substitute for each other's construction role.
later header keys may occupy the same raw positions as an earlier body's suffix. The prefix is
proof input, never an alternative semantic lookup store. Replay re-interns witness keys and
structural tuples in semantic owner order and
publishes exact per-body TypeId projections with one final program. Coinciding raw suffix positions from two
bodies cannot substitute for each other. Header programs stay unchanged and reusable; an
extended program cannot masquerade as a fresh header base. Checking rewrites all body data
slots through this projection before closing relations.

Outcome/capability/generic/user-destructor schemas, nested callback contracts and general captured
environments remain outside the accepted private fragment. Exact
scalar/unit/error independence is checked under its owning data contracts, never inferred from Copy or value receipt for arbitrary future types.
