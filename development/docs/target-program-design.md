# Target, Executable, and MIR Program Design

This document assigns implementation responsibility for v0.14.0 Phase 4. It does not define
language behavior. Target names and gates, package targets, entry contracts, CLI acceptance,
generic requirements, ABI rules, and primitive contracts remain owned by the public specification.

## Boundaries

Phase 4 has three consuming program boundaries:

```text
CheckedProgram + ToolchainSnapshot
  -> TargetProgram
  -> ExecutableProgram
  -> MirProgram
```

`TargetProgram` is the sole public success boundary shared by `check`, `build`, and `run`. It owns
the complete `CheckedProgram`; a target identity cannot be paired with a graph checked for another
target. It validates target availability, compiler-selected primitive completeness, package target
identity, and every selected-target buildability condition. A library-only package may stop at
this boundary. It does not receive a synthetic entry.

`ExecutableProgram` consumes one `TargetProgram` and one exact executable or test selection. It
validates the selected module's entry contract, then instantiates the entry-driven reachable graph.
It owns the only monomorphized item table and freezes every concrete conformance dispatch. It
cannot retain a callable requirement that MIR would have to resolve again.

The implemented executable root is compiler-owned metadata, not a synthetic source declaration. A
process root names one dense entry item and its process-result contract. A test root retains direct
test cases in declaration order and maps each case to a dense item. Empty test targets remain valid.

`MirProgram` consumes one `ExecutableProgram`. It owns concrete control-flow graphs, places,
operations, calls, and cleanup edges. MIR validation checks representation integrity only; it does
not reject a source-language capability accepted by `TargetProgram`.

## Selected Target Authority

`CompilationTarget` is a closed syntax-independent identity in `nocter-model`. Recognition is
separate from implementation availability: all names listed by the specification can select
frontend declarations, while target-program validation currently accepts only `arm64-darwin`.
The compile-unit input requires an explicit target, and `DeclarationGraph` retains it through
`CheckedProgram`.

Target-gate selection occurs once before semantic identities are allocated. One temporary
selection inventory is shared by:

- discovery-edge validation, which ignores block imports contained in inactive items
- symbol collection, which omits every token and decoded string owned by an inactive item
- declaration-surface collection, which omits the declaration and its complete body

This order permits disjoint target declarations to reuse a name and prevents inactive source from
changing active symbol IDs or producing name, type, body, ownership, or import diagnostics. Later
stages receive only the selected declaration graph; they do not inspect `#target` syntax or filter
semantic arenas. An unknown gate name is an authored `E0233` failure. A recognized reserved name is
valid source and becomes an availability error only when that target is selected.

## Toolchain Snapshot

The target-program crate receives one immutable toolchain snapshot selected before validation.
It contains typed target identity, ABI identity, executable-writer identity, and the exact standard
primitive registry. Paths, environment variables, package display names, and runtime symbol
spellings are discovery metadata and cannot grant capability.

The snapshot and checked graph must name the same target and exact standard package. Target
validation resolves each required primitive role to its already checked semantic declaration and
stores the resulting identity table in `TargetProgram`. Missing, duplicate, wrong-signature, or
wrong-authority primitives are target-program failures. MIR and code generation consume the table;
they never search declarations by spelling.

The implemented registry contains 49 closed roles matching the selected standard-library source
boundary. Discovery attaches each role to one `CallableId`; `PrimitiveRegistry` rejects missing,
duplicate, or aliased attachments before a snapshot exists. `TargetProgram` then validates the
attached declaration's canonical standard module, name, visibility, generic shape, parameters,
result, provenance, target gate, and absence of a source body. It also rejects every primitive
declaration not attached to a role. The `arm64-darwin` syscall result is part of that contract: its
nominal authority, gate, copy declaration, field order, field names, field types, and field
visibility are checked rather than inferred by the backend.

Target recognition alone grants no backend capability. `CompilationTarget` remains the closed
frontend identity, while `ToolchainSnapshot::select` is the sole implementation-availability
authority. Only `arm64-darwin` currently selects the indivisible Arm64 backend, Nocter
Arm64-Darwin ABI, and Arm64 Mach-O writer. Reserved targets fail before a `TargetProgram` can be
constructed.

## Package Targets and Entries

Package discovery selects each relevant `#executable` and `#test` record and resolves its exact
module. Directive path interpretation ends at discovery. The existing typed `PackageTargetId`
arena is populated during declaration lowering and becomes the only target selection index.

The implemented lowering input pairs an exact package-directive `NodeId` with the resolved
`ModuleIdentity`. The directive remains the authority for target kind, decoded name, and source
order; the resolution supplies only the module edge. Lowering validates that the node is a direct
target directive in the owning package and that the module belongs to that package, then reserves
the typed target in canonical package/source order. `SourceIndex` projects `PackageTargetId` to the
exact name literal. Duplicate selected names and declaration positions cannot enter a frozen
program.

Target validation checks that every supplied target belongs to its package and selected module.
Executable construction selects one `PackageTargetId`; no filename convention or imported `main`
fallback exists. Entry lookup uses the selected module's authored namespace and requires its exact
top-level `main` callable to satisfy the specified non-generic, parameter-free process-result
contract. Test targets instead select direct `test` declarations in source order.

Single-file mode creates one explicit executable selection owned by discovery. It does not mutate
the source into a package manifest and does not introduce a parallel entry algorithm.

That selection is implemented as one ordinary semantic `PackageTargetId`: `PackageMode::SingleFile`
supplies the sole root module and source display name, and the target projects to the complete file
root. Package and file execution therefore enter the same target and entry selectors.

Executable entry selection reads only the selected module's authored namespace. It freezes the
exact target, package, module, callable, body, source result type, and classified process-result
contract. Only top-level, non-generic, parameter-free functions returning `void`, `void!`, `i32`,
`i32!`, `usize`, or `usize!` are accepted. Prelude fallback, imported modules, re-exported
callables, and same-spelled non-functions have no entry authority.

Test-target selection filters the checked `TestId` arena by the exact selected module and retains
canonical declaration order. Each selected case freezes its declaration, name, and body. Imported
modules and dependencies are not traversed, and no callable or synthetic source `main` is created.

## Instantiation Authority

A monomorphized callable key contains callable identity and the complete owner-plus-callable
generic domain keyed by `GenericParameterId`. Receiver type is not stored a second time: an
instance, construction, or conformance target is reconstructed from its declaration and that one
substitution. The canonical key rejects missing, extra, duplicate, and still-symbolic arguments.
The work queue is deterministic by the complete key.

Executable specialization forks the checked type store while preserving its existing `TypeId`
prefix. Applying generic substitutions may intern additional concrete types only in this fork.
One checking-owned concrete dispatch resolver consumes checked `StaticSelection` values and
produces semantic-shaped plans containing direct callable, compiler primitive, or indirect
callable-value steps. A plain invocation contains one step. A comparison retains independent
left-operand coercion, right-operand coercion, and operation lanes. An index projection retains an
independent receiver coercion and operation lane. This shape prevents ordered step arrays from
losing which value a coercion consumes. Interface requirements resolve through the retained
conformance authority, including required/default method selection. MIR never receives an
unresolved `RequirementId`.

The same resolver owns concrete destruction planning and its specialized type-store fork. A plan
records a nominal type's exact drop-body substitution before its reverse-order field or active
variant payload work, and recursively covers arrays, outcomes, closure environments, and opaque
witness representations. Copy types and move-only representations with no owned destruction work
produce no plan. Closure definitions retain each environment binding together with the type
actually stored in that field; a readwrite capture is therefore move-only without being mistaken
for ownership of its referent. Opaque destruction opens only the checked witness table after the
opaque generic domain is concrete. The executable closure can enumerate every required drop body
without rematching types or reconstructing storage layout.

Checked cleanup dependencies preserve their representation shape rather than collapsing to a type
set. Complete values use ordinary recursive glue. An enum residual records its exact active variant
and still-initialized payload identities after pattern transfer; its plan excludes both moved
payloads and an owner drop body that already ran before transfer. This distinction prevents a later
generic lowering from turning residual cleanup into a second whole-enum destruction.

One executable dependency traversal covers calls, receiver and operand coercions, comparisons,
index projections, iteration, typed literals, interpolation, closures, explicit pattern drops,
and every scheduled cleanup type. It excludes source retained under `Unreachable` and unreachable
pattern fallbacks. This is the only edge inventory from which the monomorphization queue may grow.
An explicit pattern drop retains both its `DropId` and the complete declaration-generic
substitution selected from the subject type; a later stage never rematches a source type pattern.

Instantiation substitutes the checked signature and body, proves retained requirements through
the checked program's conformance authority, resolves abstract dispatch once, and enqueues exact
callees, drop bodies, construction members, closures, and compiler-generated semantic operations.
Opaque witnesses become concrete at this boundary. Unreachable generic declarations are not
instantiated and cannot create target code.

This closure is implemented with `CallableInstanceKey`, `ClosureInstanceKey`, and
`DropInstanceKey`, each validated against the complete declaration-owned generic domain. A
`BTreeSet` work queue closes semantic keys; dense `ExecutableItemId` values are assigned only after
closure, in full key order. Discovery order and first-use queue order therefore cannot affect item
identity. Each executable body freezes source-to-concrete type edges, direct item IDs, typed
standard and structural primitive calls, indirect callable contracts, nested closure IDs, exact
drop item IDs, and cleanup-specific destruction plans. Bodyless direct calls are accepted only
when the selected toolchain registry assigns their callable to a primitive role.

Each item separately freezes its complete concrete runtime signature even when a parameter is
unused by the body. Callable receivers precede ordinary parameters and materialize their declared
owned, readonly-borrow, or readwrite-borrow capability instead of reusing the owner type. Closure
signatures add one capability-correct environment input before closure parameters. Drop bodies
retain their exact readwrite receiver, and tests have no inputs. Each standard primitive call also
freezes its concrete signature. Signature specialization belongs to executable construction; MIR
cannot apply generic substitution or infer ABI inputs from body references.

Each closure item additionally freezes one concrete environment layout: its `ClosureId`, concrete
closure type, invocation capability, and every capture binding paired with the concrete type stored
in that field. A borrow capture therefore remains a borrow field rather than being collapsed to its
referent. The enclosing executable body points to that exact item, so MIR construction, capture
projection, invocation, and destruction all consume one layout authority. Each executable body
also freezes the deterministic first-use node domain reached from its root. Preparation passes may
not scan sibling closure roots merely because those nodes occupy the same checked-body arena.

## MIR Authority

Each instantiated body lowers to dense basic-block and operation arenas. Terminators name exact
successors. Calls name monomorphized item IDs. Places retain concrete projection and storage
identity. Cleanup schedules already frozen in checked HIR become explicit MIR edges in their
recorded order; MIR does not infer cleanup timing from syntax or operation kind.

The canonical schema gives locals, drop flags, places, SSA values, operations, and blocks distinct
dense identity domains. Typed block parameters are the only merge-value mechanism. A block owns
one ordered operation list and exactly one terminator; conditional cleanup branches on an explicit
drop flag. Enum, optional, and fallible switches inspect a typed place directly, so cleanup and
pattern lowering never move an aggregate merely to recover its active representation.

Construction is mutable only through `MirFunctionBuilder` and `MirProgramBuilder`; finishing
consumes both builders. Function validation receives a narrow immutable environment containing
only the concrete type store, declaration members needed for projection validation, concrete
closure layouts, and the closed executable-item domain. It validates specialized nominal and
closure-capture projections, aggregate layout, local and place capability, operation typing, edge
arguments, reachability, SSA dominance, switch shape, and return behavior. Program validation then
checks direct calls, closure environment signatures, and drop invocations against the complete
function arena. This split permits future per-item incremental validation without giving MIR
access to source or package setup state.

The implemented MIR validator requires closed successors, valid typed place projections, resolved
and signature-correct call targets, SSA dominance, and complete terminal behavior. Flow-sensitive
initialized-use and balanced region-release validation remain coupled to pending region
flow analysis and whole-function storage validation; documentation must not claim those gates
before they exist. These are compiler integrity checks over an accepted program, not a second source
diagnostic system.

## Construction Order

1. Select and validate target gates before import and symbol processing.
2. Freeze the selected target into `DeclarationGraph` and preserve it through checking.
3. Lower discovery-owned package targets into typed semantic identities. **Complete.**
4. Introduce the target-program crate and immutable toolchain capability snapshot. **Complete.**
5. Validate selected-target availability, standard primitive roles, package targets, and complete
   buildability into `TargetProgram`. **Complete.**
6. Select an executable/test entry, define canonical concrete callable/closure/drop keys, enumerate
   checked-body dependencies, resolve concrete dispatch and destruction plans, and close one
   deterministic reachable item graph. **Complete.**
7. Define typed MIR identities, immutable builders, CFG schema, and closed validation. **Complete.**
8. Lower concrete checked bodies and cleanup schedules into MIR, then materialize compiler-owned
   process and test roots.

The current end-to-end lowering slice covers concrete signatures, constants, primitive integer
operations, aggregate construction, ordinary copy/move/borrow places, initialization and
assignment, value-producing branches, short-circuit logic, returns, direct and standard-primitive
calls, receiver preparation and one-step receiver coercion, borrow conversions, and primitive or
selected comparisons. Comparison lowering consumes source-selected operand coercions and
specialization-selected coercions through the same lane-preserving plan. Selected and coerced
indexing lowers a place prefix, borrows it with the frozen receiver capability, invokes its exact
coercion/operator lane, and continues projection from the returned borrow. Nested field projection
and readwrite storage therefore use the same MIR place model as ordinary storage. The slice runs
through the complete source-to-executable fixture rather than a second hand-built input model.
Outcome injection and tags lower to typed aggregates. Propagation, force, and recovery materialize
their operand exactly once, switch on storage, and move a typed payload projection only on the
selected branch. Propagation reconstructs its failure with retained inner-to-outer result layers;
recovery initializes an authored catch binding before lowering its fallback. Unconditional
cleanup schedules lower at their checked event timing and consume only executable destruction
plans. They cover owned paths and staged values, assignment replacement, propagation failure,
user drop calls, reverse struct/array payloads, active enum/outcome payload switches, opaque
witnesses, and region release. Borrowed receiver roots remain initialized inputs but never become
callee-owned cleanup. One canonical value-storage slot is shared by borrow preparation, outcome
inspection, pattern projection, and cleanup. Conditional path and value schedules use
explicit entry-visible drop flags updated on initialization, move, replacement, and destruction.
Exact typed place interning makes the flag and ordinary storage operations share one identity.
Block fallthrough and explicit control transfer consume the same checked `BeforeTransfer` cleanup
events. Explicit drop, compound integer assignment, `break`, `continue`, while and infinite loops,
and integer ranges lower into closed CFG; a checked nonbreaking loop has no synthetic exit, and a
range uses a dedicated increment latch. Collection loops consume frozen source-expansion and
`next` dispatch, retain one canonical iterator slot, borrow it at the loop header, switch on the
returned optional place, and move only a present payload into the loop binding. Exhaustion, break,
and return cleanup share the iterator drop flag. Enum patterns switch directly on retained,
consumed, temporary, or borrow-dereferenced subject storage. Checked binding modes select payload
copy, move, or borrow without repeating copyability proof, while one checked remainder plan
selects complete or variant-residual destruction. Mutually exclusive remainders use independent
drop flags on the shared subject slot, and value-producing arms join through typed block
parameters. Lexical region entry creates and initializes one compiler-owned allocation-context
local from its checked parent borrow. Region exit remains part of the ordinary cleanup schedule,
which orders inner values before child release on fallthrough and every explicit transfer.
Validation requires the compiler-selected allocator and allocation-context nominal identities for
both operations. Concrete closures lower through a binding-preserving aggregate tied to one
executable closure item. Direct closure calls borrow or consume that same environment according to
its frozen capability. Inside the closure body, hidden environment and stored-borrow dereferences
remain explicit typed place projections; move captures participate in the ordinary cleanup-flag
and recursive-destruction machinery. Other unsupported checked operations still fail; the current
slice cannot silently omit accepted semantics.

## Prohibited Designs

- filtering target-gated declarations after namespace or body checking
- storing a target as an unchecked string after compilation setup
- matching a toolchain, standard package, primitive, entry, or runtime item by display spelling
- allowing checking input and declaration/checked graphs to carry different targets
- reparsing package target paths after discovery
- creating separate generic-instance or conformance indexes for MIR and code generation
- representing composite dispatch as a flat step sequence that loses operand ownership
- returning public `check` success before selected-target buildability is complete
