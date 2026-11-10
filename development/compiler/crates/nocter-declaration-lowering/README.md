# nocter-declaration-lowering

## Responsibility

Lower one closed compile input into an immutable declaration program, validate all declaration-level
contracts, and publish accepted or explicitly recoverable declaration evidence.

## Contract

The crate consumes syntax, compile-unit topology, target selection, toolchain/runtime contracts, and
model construction authority. It produces `DeclarationProgram`, `AcceptedDeclarationProgram`, and
one source-neutral `ReusableDeclarations` result containing the accepted authority, primitive,
trusted target-service, and runtime-storage bindings, and `FrontendProjectionRecipe`. The recipe materializes frontend
bindings, diagnostic origins, and source projection together for one current syntax generation. It
does not check callable bodies.

## Internal Responsibilities

- deterministic identity reservation and definition
- a bounded private access-declaration leaf query and exact-current context, sharing the syntax
  owner's declaration-surface/locator projection rather than a second parser or released borrow
  lowering; header/body checking and public-module migration remain separate
- constant/static metadata defined independently of initializer eligibility or value production
- one restricted dependency query for fixed-array lengths and their scalar structural constants,
  keyed by semantic identities rather than evaluation-pass order
- one canonical `DeclarationSurface` topology consumed by both reservation and focused topology tests
- define-once semantic projection recipes and current-generation materialization
- declaration-only authority projection separated from current body imports and spellings
- stable path-plus-block identities mapping body query keys to declaration-owned `BodyId` values
- canonical source-domain and body-import rebinding for reused declarations
- module namespaces, imports, visibility, and exports
- generic and type-position normalization
- simultaneous receiver/parameter reservation followed by value-provenance clause resolution
- one exported projection of authored callable modifiers into declaration and structural-type
  contracts, reused by body checking for body-local callable types and kept separate from callable
  execution scheduling
- construction-time binding of inherited associated names before declaration capability freeze
- declaration surfaces and contract/definition joins
- primitive, trusted target-service, runtime-storage, builtin, standard-role, and package-target
  projection
- declaration recovery and diagnostic classification

`src/pipeline.rs` owns only production stage composition and failure classification. Its
cross-stage integration tests live in `src/pipeline/tests.rs`; focused responsibility tests remain
beside their owning modules.

## Invariants

- Accepted declaration semantics are built once and cannot contain a deferred invalid edge.
- The eager whole-stage convenience entry exists only behind the `test-api` feature or the crate's
  own test build. Production consumers can use reusable query contracts but cannot invoke a second
  declaration scheduler.
- Target directives, primitive roles, target-service roles, and standard roles are selected once
  upstream or here.
- A selected non-builtin primitive type becomes one opaque nominal identity and one runtime-storage
  binding; lowering does not invent or expose a field representation for it.
- `SourceIndex` is output projection, never semantic input.
- A projection recipe contains semantic identities and declaration-surface locators, never
  `SourceId`, `NodeId`, token ranges, documentation text, or body-local syntax.
- A recipe retains the source-neutral module identity, canonical path, and source kind for every
  source ordinal. Current materialization rejects a different domain before resolving any locator.
- `FrontendBindings` and `SourceIndex` are both interpreted from the same recipe. Documentation is
  read from the current syntax tree during materialization, so trivia-only edits cannot retain
  stale hover text.
- Current checking materialization computes the exact body-symbol suffix once and returns it with
  frontend bindings and source projection. Session cannot rescan syntax to construct a parallel
  symbol domain.
- Body-local imports remain current body input and cannot enter the reusable declaration recipe.
  Their current syntax identities are joined only through the exact source-neutral module mapping
  frozen with `ReusableDeclarations`; materialization neither repeats module lookup nor declaration
  lowering.
- Contract and private definition joins use exact identities, not text matching downstream.
- Lowering evaluates only constants eligible for structural type construction. It never evaluates
  immutable statics or publishes a final initializer value. Metadata freeze is driven by reserved
  declaration identities, not by whichever values the restricted evaluator can produce.
- Each fixed-array length and its structural constant dependencies share one memoized query.
  Speculative ordinary-constant planning is isolated and cannot leak source projections or a
  partial semantic product.
- Public contracts and private bodies must retain the same authored guarantee fingerprint before
  either occurrence reaches semantic declaration storage.
- Callable execution is derived only from the syntax-owned `async` modifier. Result normalization,
  aliases, and generic substitution cannot classify or reclassify execution.
- Callable input clauses resolve only after the complete header identity domain exists. Forward
  references and mutually constrained inputs therefore cannot depend on source walk order.
- A derived associated binding resolves through bound prerequisite identities to the original
  declaration; lowering never creates an alias declaration for inheritance. The accepted
  declaration graph then freezes the effective identity so later stages do not repeat this binding.
- Declaration recovery remains one closed product across the computation and session boundary;
  consumers cannot recombine its graph, type authority, source ownership, and source projection.


## Future Private Access Declarations

The access-declaration query accepts only complete `V2CoreUnit` syntax. It collects all headers,
canonicalizes declaration symbols, reserves functions and records by name order, and resolves
data types, input operations, namespaces, authored const capability and value-result constraints
before examining body semantics.
`headers` extracts syntax-only header evidence for functions and explicitly typed required constant/static initializers.
Initializer expressions use the syntax owner's existing body-surface exclusion and current-body
locator mechanism, so initializer-only edits rebind headers without retaining stale values. `type_syntax::value_type_syntax` projects one
`ValueTypeSyntax` for input/result headers and checking's local annotations. Its typed invocation and input-operation projection is the only
decoder for these contexts; neither consumer recovers a mode from names, types or body use.
`definitions` resolves input and result slots through one value normalization path and the
declaration builder's single data namespace. Every callback leaf retains its owning annotation
as the diagnostic location for unknown/unsupported data. Forward record references, invalid fields,
containment cycles and Copy eligibility therefore share the declaration authority instead of a
lowering-owned type table. Existing built-in-name and no-authored-shadowing rules apply. Duplicate declarations/parameters retain
the first conflicting declaration; forward references cannot alter collision selection.

The reusable result owns a single immutable value-access declaration program, its exact neutral
header surface, and projection locators. It contains no current syntax IDs, source identity,
body syntax, or source coordinates. Binding an equal surface constructs one current context
with that same program and the current header/body nodes. Changed headers, a wrong parse goal,
invalid syntax, and mismatched locators cannot create a context or trigger fallback lowering.
Body-only edits can rebind but receive no body-validity certificate from declaration reuse.
The complete neutral surface guards record/field and const-capability header changes too. Function locators retained
for body rebinding do not duplicate record schemas or create a second record-header cache.
`projection` maps declaration-owned failures through current header evidence; it does not select
semantic failures from diagnostic codes or recover meaning from source coordinates.

The source-neutral declaration product remains owned by `nocter-declarations`. The pairing with
syntax remains here; there is no public parts constructor or conversion to the legacy declaration
program. The [cross-stage boundary](../../../architecture/pipeline/value-access-declarations.md)
owns the handoff to typed-only body checking and the still-required flow/safety validation.

The current access-declaration context supplies the exact immutable header program to checking.
Body queries retain that handle for source reuse and open declaration-owned application-type
branches over its prefix. Unit construction canonically replays those branches into one final
semantic authority; lowering does not inspect or cache their contents. Rebinding must still prove
the same original header authority. Structurally equal declarations rebuilt by a different query
cannot substitute for it. Source projection remains outside semantic type lookup.
