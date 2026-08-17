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

## Instantiation Authority

A monomorphized item key contains callable identity, optional concrete receiver type, and generic
arguments keyed by `GenericParameterId`. The work queue is deterministic by that complete key.
Insertion of the same key with different substitutions is an internal integrity failure.

Instantiation substitutes the checked signature and body, proves retained requirements through
the checked program's conformance authority, resolves abstract dispatch once, and enqueues exact
callees, drop bodies, construction members, closures, and compiler-generated semantic operations.
Opaque witnesses become concrete at this boundary. Unreachable generic declarations are not
instantiated and cannot create target code.

## MIR Authority

Each instantiated body lowers to dense basic-block and operation arenas. Terminators name exact
successors. Calls name monomorphized item IDs. Places retain concrete projection and storage
identity. Cleanup schedules already frozen in checked HIR become explicit MIR edges in their
recorded order; MIR does not infer cleanup timing from syntax or operation kind.

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
6. Select an executable/test entry and instantiate one deterministic reachable graph.
7. Lower concrete checked bodies and cleanup schedules into MIR.
8. Validate MIR without source or syntax access.

## Prohibited Designs

- filtering target-gated declarations after namespace or body checking
- storing a target as an unchecked string after compilation setup
- matching a toolchain, standard package, primitive, entry, or runtime item by display spelling
- allowing checking input and declaration/checked graphs to carry different targets
- reparsing package target paths after discovery
- creating separate generic-instance or conformance indexes for MIR and code generation
- returning public `check` success before selected-target buildability is complete
