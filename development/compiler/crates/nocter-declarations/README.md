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

The value-access product owns one immutable program for the future private core goal. It is not
an adapter to `DeclarationProgram` or accepted by the released target/checking pipeline. Its
builder reserves identities before definitions and freezes only after all header checks. Defining
headers in a different order cannot change parameter identities. The lowering owner
reserves functions and records in canonical namespace order.

`value_access::data` owns closed data-name resolution, TypeId projection and Copy queries over
one privately owned type authority. Callable headers admit `bool`, `i32`, `void`, opaque move-only
`error`, closed nominal records, and flat callable input/result contracts. Construction-time
`ValueDefinition` slots share data or callable normalization; `DataReference` leaves resolve to
TypeIds before publication. They do not expose released type-store eligibility helpers or borrow
constructors. Unit is a value in this product. Input operation is
separate from data identity and payload Copy evidence. Stored parameter records carry a data
TypeId and Read/Edit/Take operation; callable results carry a data TypeId and, for a callable
result, its structural constraint. Neither introduces a result access operation. No result
mode/referent set or old read-result normalization remains.

The same program publishes closed nominal record metadata used by parsed callable and body
checking. Record reservations construct the namespace once; validation and freeze retain that
same namespace rather than rebuilding a parallel name index. Fields have a distinct value-field
identity domain. Record
schemas retain declared field order, resolved TypeIds and a frozen local member namespace.
Definition order cannot affect those identities. Record and callable names cannot collide.
Structural validation visits the containment graph from leaves to parents without recursion or
payload expansion. It rejects non-finite containment with a field witness and rejects an explicit
Copy request containing a non-Copy component. Ordinary records never acquire Copy implicitly.
Only validated metadata can be published; no partial schema authority escapes a rejected build.

`project_data` validates selected field paths against the exact containing record and field
owner, without looking up names or inferring access permissions. An empty path preserves its
validated data type; a scalar projection or a field from another owner is rejected.

`data_schema` exposes the admitted builtin, nominal or named/formal/result function-witness schema.
`data_copyable` consumes that validated shape; `data_kind` deliberately remains a builtin-only projection. These APIs do not
publish arbitrary borrow types, generic eligibility guesses, or a second type store. Current
nominal schemas contain only values over the four closed builtin leaves and other such records.
They are metadata, not proof of body acceptance, partial initialization, survival, destruction or
native execution. Selected source places consume these schemas through the private body checker;
only closed unit acceptance establishes the combined body/flow/resource proofs.

`value_access::functions` owns named/formal/result witnesses and structural contracts as distinct
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
Checking must establish each result family's actual producer before body acceptance; a result header
never invents a producer or its payload properties. Distinct formals retain distinct witness
identities even with equal structural signatures. Released
declaration validation rejects all private witness keys.

Outcome/capability/generic/user-destructor schemas, input-dependent callback results, nested callback
contracts and captured environments remain outside the accepted private fragment. Exact
scalar/unit/error independence is checked under its owning data contracts, never inferred from Copy or value receipt for arbitrary future types.
