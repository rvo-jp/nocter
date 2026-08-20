# New Nocter Compiler

This directory is the implementation root for the specification-first Nocter compiler rewrite.
The lexical and syntactic grammar gate is closed. The completed Phase 1 workspace owns normalized
source storage, lexical projection, an immutable syntax arena, parser diagnostics, and the complete
G001-G033 recognition boundary from source roots through declarations, types, blocks, statements,
patterns, and expressions.

## Authority

The compiler derives public behavior from [`spec/`](../../spec/README.md). Missing language rules
block implementation; they are not inferred from the archived compiler, old tests, released
binaries, or historical implementation documents.

## Isolation

The new compiler must never depend on, copy, execute, or compare itself with the compiler preserved
by commit `f6c08da3`. Existing standard-library implementation details are also not bootstrap
semantics. Public standard-library contracts come from the specification and will receive new
implementations after the required language foundation exists.

## Planned Dependency Direction

```text
source
  -> syntax
  -> semantic core
  -> analysis and checked program
  -> executable program
  -> MIR
  -> machine program and code generation
  -> CLI and editor adapters
```

Later stages cannot import syntax representations to reconstruct earlier decisions. Source ranges
remain outside semantic identity, and runtime linkage is a one-way output projection.

The Cargo workspace must begin with source and syntax responsibilities only. Its parser fixtures
derive from the [grammar conformance plan](../docs/grammar-conformance.md); semantic crates cannot
be introduced to make an unresolved syntax choice.

## Current Crates

- `nocter-source` owns source identities, CRLF normalization, normalized byte spans, and line
  projection.
- `nocter-syntax` owns lexical tokens, exact reserved keywords and punctuation, comment metadata,
  joint-token facts, string/interpolation boundaries, lexical and parse diagnostics, and the
  lossless syntax tree. Its parser covers the complete normative grammar, including token-only
  ambiguity decisions, continuation-newline ownership, body-result classification, control-header
  brace ownership, and bounded malformed-source recovery.
- `nocter-model` owns typed semantic ID domains, the canonical compile-unit symbol table,
  normalized parameter-origin sets, and interned structural types. It has no crate dependencies;
  source spans, syntax nodes, and rendered type names cannot enter its identities or interning
  keys. An interface-owned `Self` has a canonical interface-identity placeholder distinct from
  explicit generic parameters and nominal applications; conformance specialization can therefore
  substitute it without inventing an implicit binder.
- `nocter-declarations` owns the immutable declaration-program spine: exact package-and-module
  identities, normalized visibility boundaries, package targets, imports, every declaration and
  member domain, generic requirements, bodies, opaque results, and the compile-unit type store. A
  two-pass reservation builder supports recursive headers, then validates every reference and
  owner edge before freezing. It depends only on `nocter-model`.
- `nocter-source-index` owns the separate immutable projection between semantic entities and exact
  syntax-node or syntax-token origins. It indexes the same bindings independently by semantic
  identity and by source coordinate. A lowering stage may consume and extend the index without
  losing duplicate detection; canonical semantic programs do not depend on it.
- `nocter-declaration-lowering` owns the one-way syntax-to-declaration boundary. Its input is an
  explicit package graph and module/source topology supplied by discovery; it never probes the
  filesystem. It validates declared-package and single-file layouts, canonicalizes package and
  module order, and requires one discovery-owned source-or-module target for every authored `use`.
  It validates that source composition stays private, same-module, and root-reachable, permits
  idempotent source cycles, rejects module import cycles, and never reinterprets canonical paths to
  recover a missing edge. It then constructs the compile-unit symbol table, inventories every
  declaration and member with its exact syntax owner, allocates stable topology identities, and
  records their source projections. The temporary surface inventory also enforces the root-source-
  only API boundary before semantic reservation. A canonical-header pass joins eligible public
  bodyless contracts
  to exactly one private implementation body without resolving names or types; both source forms
  therefore enter reservation through one representative identity. The reservation pass then
  allocates every recursively referenceable typed ID—including associated types—in canonical
  surface order. Header preparation resolves exact declaration names and normalized visibility,
  creates declaration sites, rejects deterministic namespace collisions, and only then projects
  named entities from their exact name tokens rather than whole declaration ranges. Generic
  preparation allocates binder identities from their already-reserved owners, carries immutable
  lexical scopes into members, reuses repeated declaration-pattern binders, rejects explicit
  duplicates and nested shadowing, and projects every authored binder occurrence. Joined contract
  and implementation sources share one generic identity sequence.
  Authored import preparation builds one visibility-bearing namespace per module. Direct
  declarations, private imports, scoped/public re-exports, selected aliases, and module namespaces
  use the same table and collision rule. Dependency modules are completed before importers;
  selected names must be accessible, re-exports cannot widen their targets, and source imports add
  no semantic import identity. Exact module paths and selected-name tokens project back to their
  resolved semantic entities. The compiler-selected standard prelude is a separate fallback table:
  authored names shadow it, it never becomes an implicit re-export, standard-package modules do
  not receive it, and source code cannot import the compiler-managed prelude explicitly.
  Header type binding then converts every type occurrence into a flat syntax-independent arena.
  It resolves module selections, authored and prelude names, generic identities and arity, `Self`
  ownership, fixed-array lengths, and structural-callable origin names exactly once. Alias
  applications and associated selections remain explicit bound nodes until the normalization pass
  has the requirements needed to resolve them. That pass expands generic aliases through an
  explicit evaluation stack, rejects expansion cycles, substitutes canonical binder identities,
  resolves `Self` and associated names, and interns structural results without introducing alias
  or name-based selection kinds. Declaration target patterns use the same module,
  symbol, arity, and source-projection context and bind their bare argument names directly to the
  generic identities already allocated for that declaration. Nominal interface and structural
  callable capabilities also reuse this path resolver and flat type arena; capability syntax
  cannot establish an alternate lookup or callable-provenance path. Generic predicates and
  associated-type bounds are then bound into one closed requirement representation. Directed
  pattern refinements, general equalities, capabilities, copy, operators, borrow coercions, and
  expansion retain semantic IDs and bound types only. Their normalized forms use the same type
  evaluator, so capability and predicate types cannot diverge from declaration types. Structural
  callable parameter spellings disappear after named origin candidates become canonical parameter
  positions. Opaque results use a dedicated binding path for their interface application,
  associated bindings, captured generic identities, outcome layers, and canonical opaque type;
  they do not become a callable-header exception. The parser now represents mandatory
  interface-member `pub` with the same `Visibility` node used by every other declaration, so this
  boundary requires no interface-specific visibility recovery. Declaration-surface traversal is
  non-recursive, keeping the complete boundary safe for the parser's 5,000-layer type contract.
  Header definition then allocates fields, parameters, receivers, requirements, and bodies in
  canonical order and completes every reserved declaration arena slot. Public contracts and
  private implementations retain one callable identity but receive distinct source roles.
  Authored result provenance is stored separately from the inference state that checked body
  analysis will produce. The compiler-selected standard package and its scalar, string, error, and
  slice attachment modules are recorded as exact semantic IDs; freeze-time validation never grants
  built-in authority from a path spelling. The completed builder validates all owner edges and
  declaration shapes before returning an immutable `DeclarationProgram` and independent
  `SourceIndex`; the syntax-owned surface inventory cannot cross that boundary. Production callers
  enter through `lower_compile_unit_declarations`, which owns the complete pass order from surface
  collection through graph freezing. Individual passes remain public only as independently
  testable compiler boundaries and cannot be reordered by a production caller.
  Source-backed failures use one `SourceDiagnostic` envelope for a stable code, primary origin,
  related notes, and correction guidance. A stage-specific diagnostic retains the semantic rule
  identity separately from that presentation envelope. Compiler-state inconsistencies remain typed
  internal errors and are never assigned a language diagnostic code merely because they crossed the
  production facade. Module-surface diagnostics select only authored root-versus-implementation
  violations; malformed syntax snapshots and incomplete discovery edges stay internal. Name and
  visibility rules retain exact syntax subjects when selected, before temporary surface identities
  are consumed. The shared namespace rule domain prevents declaration and import collisions from
  acquiring stage-specific codes or messages. Import namespaces retain the exact declaration name,
  selected name, or alias token that introduced each binding. Missing selections, inaccessible
  names, and widening re-exports preserve their source subject and the target declaration before
  the temporary import state is consumed. Module dependency edges retain the authored `use` node.
  Cycle validation derives one deterministic complete edge witness rather than reporting a module
  selected from residual graph state; every edge becomes the primary span or an ordered related
  note. Source-import shape violations likewise retain their exact `use` declaration. Generic
  scopes retain each binder's declaration token together with its semantic identity. Reserved
  binders, same-list duplicates, and nested shadowing therefore project `E0280`-`E0282` directly;
  repeated names in declaration target patterns remain authored references rather than duplicate
  declarations. Header type binding likewise separates malformed compiler input from authored
  rules `E0290`-`E0302`. Resolved paths retain the token for each segment and the optional argument
  container; duplicate callable names, provenance origins, and opaque bindings retain both authored
  tokens. The diagnostic layer consequently performs neither name lookup nor tree search.
  Prelude composition adds no second import diagnostic system: authored import preparation retains
  every module-path origin, and an explicit compiler-managed prelude import reuses `ImportRule`
  `E0262`. Missing compiler-selected modules, missing retained origins, and invalid builder state
  remain internal `PreludeError` variants. The frozen program retains authored and fallback module
  namespace layers separately. Later body lookup consumes them directly; fallback names remain
  shadowable and non-exportable. Block imports belong to checked lexical scopes rather than the
  declaration import arena.
  Type binding owns a `BindingArena` containing bound kinds, root indexes, and the temporary
  `NormalizationOrigins` side index. Normalization consumes that index to project recursive alias
  cycles, unknown or ambiguous associated selections, ambiguous callable provenance, and general
  equalities without an associated projection as `E0310`-`E0313` and `E0320`. The alias cycle
  witness is complete and canonically rotated by declaration identity. General equalities are
  validated after alias expansion. No source coordinate enters a `BoundTypeKind`, canonical
  `TypeKind`, or semantic ID.
- `nocter-checking` owns the Phase 3 syntax boundary. It catalogs every declaration `BodyId` from
  its exact source projection and resolves body scopes, locals, block imports, and explicit closure
  captures without creating a second module namespace. Its program-wide conformance table applies
  binder refinements before storing target/interface patterns, substitutes `Self`, generic, and
  associated identities into method contracts, rejects unifiable patterns, selects required or
  default methods, and proves associated interface/callable bounds through the same table. It does
  not discover bodies by source containment or filesystem paths. Its separate program-wide
  instance-operation table normalizes binder refinements and retained predicates, rejects
  overlapping target patterns, and indexes operation members without using declaration order as
  candidate priority. One iterative type-position
  validator covers declaration data, callable results, type operands, borrow/raw-pointer pointees,
  structural callables, generic arguments, outcomes, and unsized forms after alias expansion. The
  same source-independent validator remains open for concrete generic substitution. Typed
  checked-HIR construction consumes these frozen boundaries through `PreparedChecking`, which
  owns the sole graph/type/conformance/name inputs but cannot escape as a partial checked program.
  The syntax-independent output schema has separate dense place, loop, and typed-node identities;
  selected calls, requirements, coercions, indexing, iteration, outcomes, aggregates, closures,
  literals, and control operations are explicit rather than rediscovered by later stages. A shared
  opaque-result authority selects one concrete witness pattern only from reachable success paths,
  proves its advertised interface and associated bindings, and preserves outcome injection through
  an explicit HIR conversion. Calls expose only advertised interface methods; `OpaqueMethod`
  dispatch opens the witness table only after executable specialization supplies concrete generic
  arguments. One concrete destruction authority shares that specialization type store. It records
  exact generic drop-body arguments and recursive reverse-order struct, enum-payload, array,
  outcome, closure-environment, and opaque-witness work. Closure definitions pair every captured
  binding with its stored environment type, so non-owning readwrite captures remain move-only
  without acquiring referent destruction. The structural unifier treats only an explicitly
  supplied set of generic identities as variables;
  requester-owned generics remain opaque. Callable inference collects receiver, argument,
  contextual-result, and equality evidence independent of discovery order, projects statically
  known outcome layers, ranks exact result identity before recursive outcome injection, and rejects
  incomplete or invalid data substitutions before checked-node construction. One program-wide
  copyability table collects normalized generic copy proofs,
  memoizes substituted `copy struct`, enum, array, outcome, pointer, and borrow classifications by
  canonical type identity, and closes over the final extended type store before becoming part of
  `CheckedProgram`. It also retains normalized family conditions and rejects an unconditionally
  move-only `copy struct` field as `E0366` during preparation. Generic-dependent conditions remain
  valid specialization facts. Callable signatures never stand in for closure-environment
  ownership. `check_prepared_program` is now the production consuming boundary for the current
  checked-body slice. It constructs scalar, local, readonly-borrow, binding, return, body-result,
  recursive-outcome, copy, named-field move, simple/compound assignment, checked integer
  arithmetic, conditional, and
  while/infinite/integer-range loop nodes. Semantic move paths track initialized
  parameter/local state independently of syntax, inherit field state from the nearest ancestor,
  preserve disjoint siblings, invalidate partially moved parents, and join only paths visible at a
  control-flow entry. Named-field selection is one visibility-aware authority that substitutes
  generic owner arguments and emits exact field source projections. The checked program owns one
  nominal-family `DropTable`; a partial move through the nearest enclosing type-owned drop projects
  `E0381` and the exact drop declaration instead of rediscovering cleanup by method or name lookup.
  Pattern transfer records that drop together with its canonical declaration-generic substitution,
  so executable specialization never rematches a source type pattern.
  The checker also rejects copy-value moves, borrow-binding moves, later uninitialized uses,
  value-producing expression statements, and reachable non-value fallthrough, and projects every
  `BodyNodeId` back to its exact syntax origin.
  Typed HIR freezes stable node/place/loop identities exactly once. A separate repeatable ownership
  walker interprets that immutable body; fixed-point analysis never reconstructs semantic nodes.
  Ordinary `if`/`else if` checking snapshots state after the condition and joins only normally
  completing branch exits. Terminal branches are excluded, branch locals are projected out at the
  entry boundary, and field state uses the same join. Source after a terminal is retained beneath
  an explicit checked `Unreachable` operation: semantic checks still run, but flow-dependent
  ownership state and later buildability receive no invented continuation. Exact loop identities
  connect nested `break`/`continue` operations to their owners. Loop headers conservatively join
  their preheader with normal and `continue` backedges until stable, then expose only reachable
  breaks and possible false-condition exits. Integer ranges evaluate both endpoints once and
  initialize a typed loop binding on each body edge; loop-local paths cannot escape the join.
  Name resolution also maps every syntax block directly to its semantic `BodyScopeId`, which the
  checked block retains. The ownership walk produces a dense node-indexed cleanup table from the
  same state used for move validation. Normal exits, returns, breaks, and continues clean inner
  locals in reverse declaration order; return then cleans owned parameters. Maybe-initialized
  paths become conditional actions, moved paths disappear, partially moved structs expand to their
  remaining fields, and discarded move-only values remain explicit value cleanup targets. Each
  nonempty cleanup schedule declares either pre-transfer or pre-store timing. Simple assignment
  uses the destination type as the RHS expectation, evaluates ownership effects on the RHS first,
  and then reuses the partial-path cleanup planner and initialization transition for whole mutable
  bindings, writable fields, and fields reached through readwrite borrows. Replacement therefore
  cannot accidentally destroy the newly stored value or maintain a second reinitialization model.
  Ordinary and compound integer arithmetic share one closed operation selector; compound nodes
  retain one RHS and one place rather than a desugared binary expression. Body failures keep their
  semantic rule identity beside the independent source diagnostic, allowing contextual diagnostic
  selection without comparing presentation strings or error codes.
  Built-in and source-defined indexing share one postfix-place constructor. Instance and lexical
  structural requirements select an exact dispatch plus canonical generic arguments; one-step
  receiver coercion is explicit in the place projection, direct operations take priority, and
  ambiguity never depends on declaration order. Readwrite selection preserves both operation
  capability and the original receiver's writability. Conditional index and coercion predicates
  recursively use the same selector and fail closed on proof cycles.
  Prefix logical/numeric operations, signed/unsigned shifts, equality, strict ordering, and
  short-circuit logic are also closed checked nodes. One comparison plan covers primitive,
  structural-requirement, direct-instance, and one-step-coercion selection. It records readonly
  operand preparation and source-order coercions separately from semantic reversal. Negative
  literal range checking produces one signed constant for exact minima, and logical ownership
  joins the RHS with its bypass path.
  Direct module function and primitive calls retain one static selection with canonical generic
  arguments and one source-ordered argument list. Argument and ranked result-context inference run
  before normalized requirements use the shared recursive proof authority. Ownership visits call
  inputs in language order and therefore uses the ordinary explicit-move state transitions.
  Construction surfaces index named functions and literal shapes once. Fixed and empty typed
  sequences infer construction binders from their elements or result context, and typed strings
  share the syntax crate's decoded-text authority with package data. Every checked literal keeps
  the exact constructor dispatch and complete generic substitution; source projection covers only
  the selected delimiter. Bare non-interpolated strings are static readonly `&str` constants.
  Compiler-owned standard semantic roles are supplied as exact declaration-name tokens and
  resolved through `SourceIndex` into one validated program-wide table. Project declarations and
  path/name lookalikes cannot acquire allocator, allocation-context, owned-String, or formatting
  authority. Typed literal `using` accepts only the exact allocator/context families from this
  table and retains its place as an explicit allocation operand evaluated before every element.
  Exact-size sequence spread uses validated standard Iterator and ExactSizeIterator identities,
  one shared expansion selector, and a dedicated iterator-acquisition node. Fixed and spread
  elements constrain one construction inference session in source order. The checked spread keeps
  its selected `next`, associated item type, exact remaining-length operation, and copy/borrow/move
  contribution mode; cleanup, provenance, and loans consume those facts without reopening lookup.
  Interpolation likewise freezes the exact standard owned-String constructor and text appender,
  selects the exact standard Format identity for every source-order operand, and retains one owned
  partial String for cleanup across propagation and explicit transfer.
  Explicit `drop name` uses the same root place, path state, and cleanup action. Copy and borrow
  bindings are rejected structurally, an initialized owned binding becomes uninitialized after its
  drop edge, and later automatic cleanup cannot destroy it twice.
  A post-ownership program-wide provenance fixed point interprets that immutable HIR without
  reopening call selection. It retains field-sensitive aggregate, enum-payload, outcome, and
  element origins; maps exact callable summaries through receiver and parameter identities; and
  stores caller-visible origins separately from compiler-owned current-allocation dependence.
  Return validation projects `E0395` for local, owned-parameter, temporary, region, unknown, or
  undeclared origins, and conformance implementations cannot exceed their interface method bound.
- `nocter-target-program` owns the selected-target and executable boundaries. `TargetProgram`
  consumes the checked program and one immutable toolchain snapshot, validates target/package
  identity and the complete closed primitive registry, and is shared by check, build, and run.
  `ExecutableProgram` then selects one process or test root and closes only reachable callable,
  closure, and drop instance keys. Every key carries its complete concrete declaration-owned
  generic domain. A key-ordered work set resolves interface, opaque, and structural dispatch once,
  converts bodyless standard calls to typed primitive roles with concrete signatures, opens
  checked destruction plans, and assigns dense item IDs only after closure in semantic-key order.
  Each body retains its exact reachable node domain, source-to-concrete type edges, prepared borrow
  types, cleanup-specific glue, and one call-site plan for every sequence literal. A sequence plan
  binds the dense literal item and specialized pack signature to its fixed/spread producer order,
  concrete iterator/item/contribution types, already selected iteration operations, and allocation
  selection without creating ordinary variadic inputs. Fixed values and spread iterators retain
  concrete residual destruction plans that close every required user drop body before MIR. Closure
  items also retain one concrete ordered environment
  layout, including binding identities and stored capture types. Composite comparison and index
  dispatch retain named coercion and operation lanes, so MIR
  never infers operand ownership from step order. Enum residual cleanup keeps its active variant
  and still-initialized payload set, so it cannot repeat a pre-transfer owner drop or destroy a
  moved payload. No unresolved requirement or source name reaches MIR.
- `nocter-mir` owns the backend-independent control-flow representation and its consuming builders.
  Distinct dense identities cover locals, places, values, operations, blocks, and drop flags.
  Validation checks concrete operation and projection types, CFG closure, edge arguments, SSA
  dominance, terminal behavior, and direct/primitive call signatures. The current checked-body
  lowering slice handles scalar and aggregate expressions, ordinary places, branching, enum
  patterns, direct and primitive calls, receiver and operand coercions, borrow conversions,
  comparisons, and selected/coerced index places. Call-backed indexing continues from the returned
  borrow as a normal place root, including nested fields and readwrite storage. Outcome lowering uses one typed
  temporary plus explicit storage switches and payload projections for propagation, force, and
  recovery; nested failure results preserve their authored outer layers. Unconditional cleanup
  consumes the checked timing table and frozen destruction plans for owned paths and values,
  assignment replacement, propagation edges, user drop calls, reverse structural payloads, opaque
  witnesses, and region release. Borrowed receivers remain initialized flow inputs without
  acquiring callee destruction responsibility. Borrow preparation, outcome inspection, patterns,
  and cleanup share one canonical value-storage slot. Conditional path and value cleanup uses
  explicit entry-visible drop flags updated by initialization, move, replacement, and destruction;
  typed place interning keeps those flags on the same identity used by ordinary operations.
  Block fallthrough, explicit drop, compound integer assignment, `break`, `continue`, while and
  infinite loops, and integer ranges lower through the same cleanup events and closed CFG builder.
  Nonbreaking loops omit an exit block, while ranges use a dedicated increment latch. Collection
  loops consume frozen source-expansion and `next` dispatch, retain one iterator storage slot,
  switch on each optional result in place, and move the present payload into the loop binding.
  Exhaustion and early transfer share the iterator's drop flag. Pattern lowering consumes checked
  copy/move/borrow modes and complete-or-residual cleanup plans, projects specialized payload
  places, and keeps mutually exclusive cleanup flags separate on one canonical subject slot.
  Lexical regions use paired typed creation and release operations; the existing cleanup schedule
  orders body destruction before release on fallthrough and early transfer. Closure construction,
  concrete invocation, capture access, owned capture moves, and recursive closure destruction use
  the executable-owned layout and binding-preserving MIR projections. Callable bounds specialize
  to direct generated-body calls; owned contracts retain explicit post-call destruction when the
  intrinsic closure body only borrows its environment. Owned callable operands are staged before
  later arguments and transferred only after those arguments succeed, preserving checked cleanup
  on propagation. No erased callable ABI reaches MIR.
  Static text constants are typed as readonly `&str` values. Typed string construction calls the
  frozen literal body and carries `using` as a call-scoped allocation-place override; MIR validates
  both the literal item authority and the selected allocator/context nominal identity.
  Static opaque results retain their public identity through one explicit executable receiver
  representation lane. MIR constructs the checked witness as an opaque aggregate and opens it only
  through a capability-preserving witness projection whose specialized type is validated against
  the checked witness table. Exact iterator selection and ordinary method lookup share this same
  advertised-interface evidence path.
  Sequence literal bodies use one dedicated `MirPackInput`, separate from ABI parameters.
  `PackLength` and consuming `PackNext` expose only the checked pack operations, and every returning
  path ends in validator-required `DestroyPack` cleanup. Caller-side `MirPackArgument` construction
  preserves allocation-before-elements evaluation, source order, one-time spread acquisition,
  checked exact total length, selected `next` calls, and copy/direct contribution modes. Deferred
  `MirDestructionPlan` recipes carry dense user drop items for unconsumed fixed values and iterator
  suffixes. Per-function and cross-function validation close the pack operands, nested calls,
  cleanup shapes, drop signatures, and hidden caller/callee schema.
  Interpolation uses no string-specific MIR operation. It invokes the frozen standard constructor,
  text appender, and formatter callables in source order, retaining the partial output in the
  interpolation node's canonical temporary. Normal completion moves it once; postfix propagation
  and explicit return destroy the same slot; a forced-outcome trap has no fictional cleanup. MIR
  and later stages do not know the `String` layout or recover any operation from a spelling.
  Callables and compiler-owned roots share one `MirBody` CFG schema without forging source-item
  identities. Process roots materialize all six entry result contracts through root-only `Exit`
  and allocation-free `ReportError` operations. Test targets retain one isolated root per case in
  declaration order, including a valid empty target. Callable `Return` and root `Exit` contracts
  are disjoint and validated before whole-program direct-call validation.
- `nocter-machine` owns target-independent stored layout and the remaining machine-program
  boundary. Executable specialization first freezes declaration-order concrete field and variant
  payload types plus exact opaque witnesses; layout never re-runs generic substitution or
  conformance selection. `MachineLayoutStore` then closes only the runtime types referenced by
  validated MIR and records size, alignment, stride, representation kind, and every view, error,
  tag, payload, field, and capture offset required by later lowering. Stored layout remains
  separate from callable transport. Symbolic or unsized values, incomplete representations,
  recursive by-value storage, invalid alignment, and arithmetic overflow are typed integrity
  failures. One immutable `MachineAbiPlan` classifies zero/direct/indirect values and freezes every
  dense function's argument registers, ordered stack slots, final stack padding, direct or
  caller-owned result transport, and separate literal-pack pointer lane. A spill closes the
  argument-register window, so later smaller values cannot reuse an abandoned register. Machine
  linkage is separately keyed by exact executable item, process target, or test declaration; no
  display name participates. Test roots retain declaration order outside that key table. Static
  text receives content-sorted, deduplicated `MachineDataId` values rather than first-use IDs.
  `MachineProgram` now owns distinct dense function, generated-destruction, stack-object, drop-flag,
  address, SSA-value, operation, literal-pack, block, linkage, and data identities. Layout-owned byte offsets, checked
  fixed/view indexing, loads, address formation, stores, aggregate writes, stored-tag control, scalar control,
  explicit allocation contexts, and direct calls lower without retaining MIR fallback nodes.
  Stored, completion, and diverging SSA representations are distinct. User-drop calls, process
  error reporting, and region creation/release now use machine identities as well. Standard
  primitives retain closed roles and use the same concrete ABI planner as direct calls. Direct and
  primitive targets share one call representation. Literal descriptors retain ordered fixed and
  spread segments through a dedicated body-local identity, while residual cleanup is frozen into
  machine-function targets, tags, strides, and byte offsets. Compiler-provided comparison, checked
  index, and borrow-weakening dispatch is lowered into representation-exact machine operations;
  no structural call target survives. One whole-program allocation-context fixed point now marks
  roots, context-independent functions, and incoming-context functions across inherited calls,
  user drops, and hidden pack callbacks or destruction. Explicit `using` selections satisfy their
  target without making the caller context-dependent. Every completed machine function also owns
  one immutable dataflow table. It expands values hidden inside addresses and literal packs once,
  validates bidirectional value definitions and typed CFG edges, computes deterministic block
  `live_in`/`live_out` sets, and records exact inputs and `live_after` values for every operation.
  ARM64 selection consumes those facts instead of rebuilding machine semantics.
  A content-ordered `MachineDestructionTable` now interns every nonempty pointer and literal-pack
  residual-destruction plan. Generated linkage is appended after the stable source domain, and
  each plan becomes an ordinary machine function with the compiler-owned
  `(byte_pointer, byte_offset)` ABI. Pointer calls and pack cleanup identities share that function
  authority. Structs, active
  enum/outcome payloads, closure captures, and opaque witnesses use layout-owned address steps;
  fixed arrays use one reverse CFG loop rather than unrolled code. Authored drops remain normal
  direct calls, so allocation-context propagation and call liveness require no generated-function
  exception. Empty plans remain an explicitly validated native no-op.
- `nocter-arm64` owns physical ARM64 register roles and instruction encoding without depending on
  MIR or any semantic crate. Register 31 is typed as `sp` or the zero register per instruction
  form, and every immediate, scaled offset, wide-move shift, and branch displacement is validated
  before little-endian encoding. Dense local labels are resolved only after monotonic conditional
  branch relaxation; duplicate, unbound, misaligned, and out-of-range targets are typed failures.
  One ABI register-role authority excludes compiler scratch and reserved registers from general
  allocation. The hidden allocation-context pointer has the fixed `x9` lane, while general virtual
  values use only `x10`-`x15` and `x19`-`x28`. The deterministic linear-scan allocator restricts
  call-crossing ranges to callee-saved registers or spills and reports the exact preservation set.
  One `Arm64ValuePlan` now classifies zero-storage, one- or two-word direct, and memory-backed
  machine values before selection. It derives deterministic intervals from machine CFG facts and
  marks only values actually live after a call as call-crossing; flattened sibling block order
  cannot force an unrelated value into a callee-saved register. Direct words share the allocator,
  while larger values remain explicit fixed-frame requests.
  `Arm64FunctionFrame` feeds maximum outgoing call storage, machine stack objects, drop flags,
  memory values, one reusable direct-aggregate construction object, one maximum-size
  memory-edge cycle temporary, literal descriptor/state pairs, spill lanes, hidden
  result/pack/context pointers, and preserved registers through that one planner. Pack descriptors
  have one uniform four-word callback ABI, while each call site's source-ordered state has its own
  checked layout. Call selection initializes the descriptor and state, and target lowering emits
  stable next/destroy function identities for every body-local pack. Fixed segments execute the
  complete callback path: next returns the planned direct or caller-owned `Optional<T>` result,
  advances the state cursor, and residual cleanup destroys every unconsumed owner in reverse order
  through ordinary generated functions. Spread-segment callback execution remains open.
  Fixed frames reserve the maximum outgoing argument area, lay out objects and
  callee-saved slots deterministically, and terminate in the `x29`/`x30` frame record while
  preserving 16-byte alignment. Prologue and epilogue materialization uses checked immediate forms
  or the reserved `x16` scratch path, so large frames and distant save slots do not impose an
  accidental immediate-width limit. `Arm64SelectedFunction` now lowers constants, extension-aware
  scalar memory loads, exact aggregate/memory copies, layout-owned aggregate construction,
  checked stack/pointer/view address formation and indexing, integer and boolean operations, raw-value
  and readonly-borrow comparisons, direct scalar arguments/results, indirect caller-owned
  aggregate transport, local branches, one- and two-word value switches, layout-owned stored-tag
  switches, returns, and process exits into virtual/fixed register transfers. Signed byte and
  halfword loads retain their
  meaning through explicit sign-extending target instructions. A separate materializer resolves
  physical registers, injects spill loads/stores, and emits frame-safe code. Unsupported machine
  nodes fail selection explicitly. Allocation-context selection initializes the root header,
  saves incoming context pointers, and materializes inherited or explicit selections through
  `x9`; current allocator state/kind primitives expand through their ordinary ABI plans. Pure
  pointer/view roles share one closed primitive selector: representation-preserving conversions
  retain staged lanes, view observers select pointer or length, unchecked string subviews adjust
  the pointer/length pair, and pointee size/alignment consume `MachineLayoutStore`. Concrete type
  arguments referenced only by a primitive are included in the layout closure. Typed CFG edges
  own both direct-lane and memory-value parallel copies. The direct resolver orders register/spill
  chains and breaks cycles through one reserved boundary register. The memory resolver applies the
  same parallel-assignment contract to frame objects and breaks a cycle through its planned frame
  temporary. Both run only after their conditional or switch edge is selected. Dense function
  and data identities already feed typed fixups:
  function branches resolve only after stable text layout, while function- and data-address
  `adrp`/`add` pairs resolve only after the writer supplies final section virtual addresses. Static text selection now
  populates layout-owned pointer/length lanes through that data mapping. One- and two-word direct
  values use the same lane projection across parameter registers, outgoing stack arguments, local
  storage, and result registers; views have no private transport path. Memory selection and memory
  instruction materialization are separate modules. A non-native 3-, 5-, 6-, or 7-byte tail is
  decomposed into exact in-bounds fragments rather than widened across an adjacent frame object.
  Aggregate recipes zero their full representation before member writes, so padding never carries
  uninitialized process state; stack-copy materialization rejects overlapping ranges explicitly.
  One selected address plan normalizes static frame paths and runtime pointer/view paths. Place
  access and structural index borrows share its unsigned bounds and layout-owned stride evaluator.
  Memory-transfer roles use a separate selector and materializer over the same instruction schema.
  One machine-owned stored-value classifier drives both ABI planning and generic primitive
  validation. Runtime-sized copies use a zero-safe byte loop; indexed byte storage and generic
  `store/take<T>` reuse exact direct-lane loads/stores or fixed-size indirect copies.
  Darwin system roles use another closed selector and materializer. Syscall arguments are shifted
  from the ordinary Nocter call lanes to Darwin's syscall lanes, and the carry flag is normalized
  into the declared value/errno result. Primitive and root process exit share one emitter. Trap,
  unreachable, and allocation abort terminate through distinct compiler-owned break reasons.
  Direct user destruction uses the same function fixup and inherited allocation-context boundary
  as an ordinary call after validating its frozen one-borrow/void ABI. Machine operations expose
  call-boundary behavior directly, so live-range allocation preserves values across user drops and
  pack callbacks without enumerating target-selected call instructions. Dense drop flags occupy
  exact one-byte frame objects initialized at entry and shared by writes and conditional branches.
- `nocter-macho` consumes only a completed `Arm64Program`. It assigns the `__TEXT`, `__const`, and
  `__LINKEDIT` file and virtual ranges, resolves section-address-dependent function/data pairs, writes the
  native entry and dyld/libSystem load commands, derives a content-stable UUID, and emits its own
  SHA-256 ad-hoc code signature. Its target test writes and executes the resulting file on ARM64
  macOS without invoking an assembler, linker, or signing tool.
- `nocter-conformance` owns tests that intentionally cross every compiler crate and the native
  image boundary. It compiles constants, scalar calls and arithmetic, control, structural
  comparisons, narrow signed values, direct- and memory-valued block joins, optional tag switches,
  static text, two-word view calls, direct and memory-backed aggregates, dynamic fixed-array places,
  fixed/view index borrows, and
  pure pointer/view primitives, runtime-sized or generic memory transfers, Darwin syscalls,
  primitive process exit, user destruction, conditional drop flags, and generated recursive
  pointer destruction
  from source, emits signed Mach-O images, and executes them on ARM64 macOS. A nine-view call also
  crosses the register-window boundary into outgoing stack transport. The constant case proves
  byte-for-byte output determinism.

Accepted fixtures through G033 have human-readable node-shape snapshots. Accepted, rejected, and
semantic-boundary fixture groups all verify exact lexical-token projection; error recovery cannot
silently discard a token.

Phase 2 is complete. Declaration validation identifies exact semantic subjects, and diagnostics
project them through `SourceIndex` without duplicating validation in lowering. Every production
failure is classified as an authored rule or an internal compiler/discovery integrity error.
Declaration-owned semantic-boundary fixtures compare complete diagnostics under reversed package
and module input order. Phase 3 is complete. Checked-body construction covers the closed body
grammar, including collection iteration, authored binding annotations, and executable lexical
regions. Region handles are typed compiler-managed resources; one cleanup authority orders body
destruction before region release on every reachable exit, retains the parent allocator loan for
the complete child lifetime, and emits no fictional unwinding for `never` termination.

## Verification

Run from `development/compiler/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
