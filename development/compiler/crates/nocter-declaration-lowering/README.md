# nocter-declaration-lowering

## Responsibility

Lower one closed compile input into an immutable declaration program, validate all declaration-level
contracts, and publish accepted or explicitly recoverable declaration evidence.

## Contract

The crate consumes syntax, compile-unit topology, target selection, toolchain/runtime contracts, and
model construction authority. It produces `DeclarationProgram`, `AcceptedDeclarationProgram`, and
one source-neutral `ReusableDeclarations` result containing the accepted authority, primitive
bindings, and `FrontendProjectionRecipe`. The recipe materializes frontend bindings,
diagnostic origins, and source projection together for one current syntax generation. It does not
check callable bodies.

## Internal Responsibilities

- deterministic identity reservation and definition
- define-once semantic projection recipes and current-generation materialization
- canonical source-domain and body-import rebinding for reused declarations
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
- A projection recipe contains semantic identities and declaration-surface locators, never
  `SourceId`, `NodeId`, token ranges, documentation text, or body-local syntax.
- A recipe retains the source-neutral module identity, canonical path, and source kind for every
  source ordinal. Current materialization rejects a different domain before resolving any locator.
- `FrontendBindings` and `SourceIndex` are both interpreted from the same recipe. Documentation is
  read from the current syntax tree during materialization, so trivia-only edits cannot retain
  stale hover text.
- Body-local imports remain current body input and cannot enter the reusable declaration recipe.
  Their current syntax identities are joined only through the exact source-neutral module mapping
  frozen with `ReusableDeclarations`; materialization neither repeats module lookup nor declaration
  lowering.
- Contract and private definition joins use exact identities, not text matching downstream.
- A derived associated binding resolves through bound prerequisite identities to the original
  declaration; lowering never creates an alias declaration for inheritance. The accepted
  declaration graph then freezes the effective identity so later stages do not repeat this binding.
