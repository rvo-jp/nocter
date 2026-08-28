# nocter-declaration-lowering

## Responsibility

Lower one closed compile input into an immutable declaration program, validate all declaration-level
contracts, and publish accepted or explicitly recoverable declaration evidence.

## Contract

The crate consumes syntax, compile-unit topology, target selection, toolchain/runtime contracts, and
model construction authority. It produces `DeclarationProgram`, `AcceptedDeclarationProgram`,
frontend bindings, diagnostic origins, and source projection as separate outputs. It does not check
callable bodies.

## Internal Responsibilities

- deterministic identity reservation and definition
- define-once semantic and presentation projection
- module namespaces, imports, visibility, and exports
- generic and type-position normalization
- construction-time binding of inherited associated names before declaration capability freeze
- declaration surfaces and contract/definition joins
- primitive, builtin, standard-role, and package-target projection
- declaration recovery and diagnostic classification

## Invariants

- Accepted declaration semantics are built once and cannot contain a deferred invalid edge.
- Target directives, primitive roles, and standard roles are selected once upstream or here.
- `SourceIndex` is output projection, never semantic input.
- Projection rejects duplicate semantic relations before updating the companion source index, so
  the two products cannot silently retain different winners.
- Contract and private definition joins use exact identities, not text matching downstream.
- A derived associated binding resolves through bound prerequisite identities to the original
  declaration; lowering never creates an alias declaration for inheritance. The accepted
  declaration graph then freezes the effective identity so later stages do not repeat this binding.
