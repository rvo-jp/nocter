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
produces ordered plans of direct callable, compiler primitive, or indirect callable-value steps.
Structural indexing and comparison may therefore retain required coercion steps instead of being
incorrectly collapsed to one callable. Interface requirements resolve through the retained
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
unused by the body. Callable receivers precede ordinary parameters. Closure signatures add one
capability-correct environment input before closure parameters. Drop bodies retain their exact
readwrite receiver, and tests have no inputs. Signature specialization belongs to executable
construction; MIR cannot apply generic substitution or infer ABI inputs from body references.

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
only the concrete type store, declaration members needed for projection validation, and the closed
executable-item domain. It validates specialized nominal projections and aggregate layout, local
and place capability, operation typing, edge arguments, reachability, SSA dominance, switch shape,
and return behavior. Program validation then checks direct calls and drop invocations against the
complete function arena. This split permits future per-item incremental validation without giving
MIR access to source or package setup state.

MIR validation requires closed successors, valid place projections, initialized-use discipline,
resolved call targets, balanced region release, and complete terminal behavior. These are compiler
integrity checks over an accepted program, not a second source diagnostic system.

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

The first end-to-end lowering slice now covers concrete signatures, constants, primitive integer
operations, aggregate construction, ordinary copy/move/borrow places, initialization and
assignment, value-producing branches, short-circuit logic, returns, and direct static calls. It
runs through the complete source-to-executable fixture rather than a second hand-built input
model. Unsupported checked operations and every non-empty cleanup schedule fail explicitly until
their dedicated lowering paths are implemented; the current slice cannot silently omit accepted
semantics.

## Prohibited Designs

- filtering target-gated declarations after namespace or body checking
- storing a target as an unchecked string after compilation setup
- matching a toolchain, standard package, primitive, entry, or runtime item by display spelling
- allowing checking input and declaration/checked graphs to carry different targets
- reparsing package target paths after discovery
- creating separate generic-instance or conformance indexes for MIR and code generation
- returning public `check` success before selected-target buildability is complete
