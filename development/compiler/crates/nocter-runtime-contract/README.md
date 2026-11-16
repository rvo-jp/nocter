# nocter-runtime-contract

## Responsibility

Own source-independent runtime primitive roles, trusted typed import identities, canonical
runtime representations, target runtime requirements, and the closed environment passed toward
native lowering.

## Contract

Declaration lowering projects authorized source declarations onto runtime contracts. Target, MIR,
machine, and executable stages consume selected identities; they do not rediscover them by function
name or standard module path. Loader symbols in this crate are validated target-catalog values, not
public source-level imports.

## Runtime Storage Identities

`RuntimeDataId` identifies data within one closed runtime environment. `Source(TypeId)` preserves
an established direct source identity; `Instance` identifies a contextual representation in that
environment's own namespace. A contextual index never becomes a new semantic type in declarations.
Neither form is a source-generation, ownership, access or executable-admission certificate.

Runtime type/representation tables and value signatures use this same identity domain. Stored
references must close in that environment; primitive roles remain unique. Empty-array element
identities retain their separate metadata-only contract below. Source semantic validation adapts
only established direct source IDs, while Machine storage, layout, ABI and cleanup retain runtime
identities throughout. There is no total conversion from a runtime identity to a semantic type.

`RuntimeEnvironment::value_field` and `value_path` own value-model component-type projection from
these closed tables. They preserve logical selectors and consume the caller's saved-index bound
check without consulting declaration types. They supply no initialization, source-bounds, backing
or access proof. Layout offsets and actual ownership transitions remain their consumers' separate
responsibilities. The [private native materializer](../nocter-target-program/README.md#private-storage-projection)
owns contextual graph construction and its current source-admission boundary.

## Finite Callable Choice Storage

`CallableAlternatives` retains an ordered list of at least two distinct `StaticCallable` runtime
identities. Each identity denotes one complete code/view/environment alternative selected upstream;
case order preserves canonical witness order rather than runtime-ID allocation order. Table closure
rejects missing, duplicate, non-callable and nested-choice entries. Each static case retains its own
ordinary environment dependency. No nominal source variant or erased heap-callable identity is
manufactured for this storage form.

This descriptor supplies neither an active selector nor source, invocation, ownership or transport
permission. Exact representation comparison still preserves runtime identities; directed guarded
transport does not inject positive cases into a choice. Target storage can project complete canonical
domains, while ordinary source execution remains unqualified until selected-case construction,
transport, invocation and cleanup close.
The [Machine layout owner](../nocter-machine/README.md#finite-callable-choice-layout) describes the
stored geometry independently of that admission boundary.

## Internal Responsibilities

- primitive role identities
- one role-owned value input/data signature for the error-constructor fragment, independently of
  source authorization, physical ABI, execution facts and result-origin proof
- one exhaustive per-role runtime profile joining primitive execution facts, produced-computation
  classification, and hidden-context requirements
- closed invocation allocation and synchronous-wait facts for primitive roles
- hidden allocation/process context requirements for primitive roles
- a typed produced-computation classification for primitive roles that construct drive-safe
  `future T` values
- kind-preserving trusted function/data symbol identities and operating-system library identities,
  without concrete loader paths
- finite target-service roles, target identity, calling convention, and fixed foreign ABI classes
- closed compiler-owned storage roles and their ABI-specific layouts
- canonical representation classes
- fixed async owning-handle and heap-frame-header ABI
- the fixed Darwin Blocks ABI subset admitted by compiler-owned native adapters
- the closed Darwin file-operation, raw-failure, partial-write, exclusive-create access,
  non-waiting lock, seek, capacity, and loader dependency vocabularies
- the generated Darwin file-job and pre-reserved retirement ownership state machines
- the generated Darwin file-service root, operation-job frame, and pre-reserved close-record ABI
- the fixed Darwin network callback-event record, payload ownership table, provider state codes,
  and two-step release fence consumed through a descriptor channel
- the closed Darwin network adapter import catalog, callback signatures, and complete-transfer
  classification
- closed async lifecycle-state tag sequences
- one fixed process-lifecycle context layout retaining the Darwin event descriptor, three original
  signal dispositions, and only the sticky termination observation
- target-independent descriptor, monotonic-timer, and process-completion wait interests
- target runtime capability requirements
- closed runtime environment schemas
- storable unit, payload-free empty arrays, static-code environment-only shapes, and value-model component/input identities

## Invariants

- A role has one numeric and structural authority.
- Source spelling and visibility are not runtime identities.
- `PrimitiveRole::value_signature` owns the shared error-constructor shape. Published target
  validation adapts its read/take operations to the published type-qualified representation;
  consumers must not maintain another constructor signature table. Other roles need their own
  signature closure before entering this fragment. A descriptor alone grants no v2 source,
  evaluator or executable primitive admission.
- A closed operating-system role owns its syscall identity, fixed flags, and result convention in
  the target backend. Standard-library source can select the semantic role but cannot reproduce or
  override those ABI constants.
- A loader symbol is validated once, retains whether it denotes a function or data object, and
  cannot be selected from user source.
- A target-service binding owns its closed descriptor; later stages cannot reconstruct the
  descriptor from its role or declaration spelling.
- Process lifecycle observation has one runtime context authority. Reload remains a consumed event,
  while interrupt or termination becomes sticky; neither source wrappers nor ARM64 selection may
  invent a parallel process-global state layout.
- A runtime-storage role has one declaration binding and one ABI-selected layout; source cannot
  supply its fields, size, or alignment. Native owners and ownership-bearing callback events keep
  distinct roles even where their current layouts happen to have equal dimensions.
- Machine consumers cannot reach declaration or checking storage through this contract.
- Value-model aggregates preserve record-field keys and tuple positions without equating them
  with physical offsets. Their ordered representation does not erase source data identity.
- `EmptyArray` retains an established element identity without a stored-payload edge. Table closure
  validates actual payload references; it does not require an unused empty element's runtime entry
  or reinterpret a released zero-count `FixedArray` as this value-model shape. `RuntimeType::array`
  supplies the common homogeneous identity/count view, independently of physical geometry.
- `Uninhabited` is an explicit empty represented domain, distinct from missing metadata and an
  inhabited zero-byte value. Outcomes may retain it as an excluded payload descriptor; it supplies
  no positive value, callable, address or stored layout. Source admission and tag proofs remain
  upstream obligations.
- `RuntimeEnvironment::has_same_value_representation` and `permits_guarded_value_transport` share
  one structural comparison for MIR and Machine. The first preserves represented domains; the
  second permits an explicitly excluded alternative to widen only beneath an outcome guard.
  Empty-array metadata supplies no positive element; represented leaves, including static callable
  identities, remain exact. Structural tuples and outcome/array carriers retain their children,
  while distinct nominal records remain distinct. The directed relation rejects empty roots,
  narrowing and unknown entries, and visits shared pairs independently of array cardinality.
  `value_relations()` retains those comparison obligations for one immutable environment. Each
  pair/mode/guard obligation is expanded once; failures propagate through dependent parents,
  including cycles, before a result is returned. Repeated child queries reuse that result without
  mixing exact equality with directed transport or a guarded child with a root value.
  Neither relation supplies a source conversion, code/environment proof, ownership or executable
  layout conversion; guarded transport is distinct from same-layout copying.
- Stored data shape does not encode a value-model input operation. RuntimeValueSignature retains
  the environment/explicit local channels independently; it introduces no borrow data type.
- `ValueText` grants stored descriptor representation only to the exact Text primitive identity.
  Unmarked published Text remains unsized. The marker supplies descriptor geometry, not backing
  lifetime, Copy evidence or a general string-owner/destruction contract.
- Native callback layouts have one numeric schema; source code cannot construct or inspect them.
- A generated file job has one fixed field schema, one operation-to-field contract, and one checked
  allocation-size calculation. Workers never retain authored path or transfer-buffer storage; a
  read destination is consumer-only state and cancellation clears it before detaching the worker.
- The Darwin callback channel carries complete event records. A shared queue, lock, and wake-only
  signal are not part of the runtime contract.
- Network adapter consumers select typed catalog roles; they cannot reproduce loader symbols,
  symbol kinds, libraries, Block signatures, event size, or interrupted-transfer classification.
- A final Network.framework state must be followed by a barrier on the same serial dispatch queue;
  event receipt alone never authorizes native-owner release.
- Effective Network.framework addresses cross the target boundary only as complete copied Darwin
  IPv4 or IPv6 socket records. Provider path and endpoint ownership never enters source storage.
- Primitive execution facts are keyed by closed roles, never inferred from source names or target
  instruction sequences.
- Caller-storage preservation is separate positive evidence. The qualified error snapshot
  constructors preserve live caller storage while constructing or receiving their own payloads.
  Other roles retain no such evidence; read inputs or allocation/blocking guarantees cannot supply it.
- Every primitive role has one exhaustive runtime-profile arm. Adding a role without assigning its
  execution facts, produced-computation classification, and hidden contexts fails compilation;
  omission cannot silently certify allocation-free or nonblocking behavior.
- Primitive hidden-context requirements are keyed by the same closed roles. Machine planning
  consumes that contract and cannot maintain a second target-call classification.
- A primitive role may immediately construct an opaque asynchronous value. This does not classify
  the primitive as a deferred Nocter body; the role, result contract, and target helper jointly
  define the construction boundary.
- Semantic descriptor, timer, and process interests and their numeric ABI records have one mapping
  here. Reactors and target backends consume that mapping instead of assigning independent meanings
  to tags.
- A computation supplies only its suspension-state count. This contract assigns its initial,
  suspension, and completed tags; a target backend cannot repeat the arithmetic or assume zero as
  the initial encoding.
- Timer ordering is a half-domain wrapping-counter contract. A target wait-width cap may divide one
  deadline into multiple native waits, but cannot make the logical interest eligible early.
