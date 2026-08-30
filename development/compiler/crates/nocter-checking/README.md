# nocter-checking

## Responsibility

Consume one accepted declaration program and produce syntax-independent typed semantics, ownership
facts, dispatch decisions, and explicit source-justified recovery evidence.

## Contract

Checking receives immutable program facts, exact body syntax projections, diagnostic origins, and
one semantic construction authority. Program-wide authorities can be prepared from the stable
declaration-symbol prefix and reused independently of body text. Opening a current generation
adds body spellings and source access without rebuilding those authorities. A successful result
exposes `CheckedProgram` and immutable semantic authority. A rejected analysis result classifies
every reached name/body domain and may expose only the recovery capabilities justified by its
diagnostics. Source projection is extended beside, never inside, semantic output.

## Internal Responsibilities

- program-wide preparation and standard semantic roles
- lexical name evidence and body scopes
- source-neutral lexical recipes with current body-local locator and spelling rebinding
- body-local structural type-extension recipes independent of sibling allocation order
- source-neutral checked-body and source-evidence recipes with canonical current replay
- type checking, inference, operations, construction, and calls
- interface implementation and instance-operation selection
- specialized interface-capability evidence and prerequisite validation
- ownership, loans, provenance, regions, cleanup, and destruction
- persistent type/copyability/closure transactions
- checked and recovery semantic queries

## Invariants

- One source-neutral `ProgramEnvironment` carries stable facts through the complete checking
  lifetime. Generation-local `SourceAccessTable` storage is paired only by prepared and checked
  current-generation owners.
- Eager prepare/check convenience entries exist only behind the `test-api` feature or the crate's
  own test build. Production orchestration consumes query contracts and cannot reopen the stage.
- Query-owned program preparation accepts only a closed set of authored rule variants. It retains
  declaration recovery and repair evidence as one exact-current rejection; opening a session
  branch clones that authority and cannot rerun preparation or publish an internal error as source.
- Declaration proof requirements cannot carry runtime evidence. Body requirements always carry
  one evidence identity; no optional-evidence state exists.
- One independent capability-evidence table owns each specialized predicate and every authored
  root/origin derivation that establishes it. Requirement order cannot select one derivation as
  the semantic authority.
- Type and copyability authority cannot be paired across generations.
- A body transaction commits all semantic mutations together or is discarded/frozen as one branch.
- Checked dispatch is selected once; Target and MIR receive no lookup inputs.
- Generic lookup, provenance, loans, concrete dispatch, and editor queries consume the same frozen
  capability-evidence identity; a later stage cannot reinterpret the predicate or collapse its
  source derivations to whichever requirement was visited first.
- A checked query derives type and visibility from its own body generation.
- Reusable body-name evidence contains body-local locators and spellings, never `NodeId`,
  `SyntaxToken`, `SourceId`, source spans, or current symbol IDs.
- A reusable checked body contains no current source identity, span, syntax identity, or symbol ID.
  Node origins, references, and associated-type completion sites use one body-local locator recipe.
  Local and capture declarations are restored from the lexical recipe instead of being duplicated
  in the typed result.
- A body type extension distinguishes the immutable prepared-program prefix from dense body-local
  additions. Closure references are body-local as well. Canonical program finalization re-interns
  both domains, so one sibling cannot change another body's reusable type identities.
- Each body checker opens from the same prepared semantic prefix and an empty closure domain. It
  cannot observe inferred types, copyability memoization, or closure allocation from a preceding
  sibling. Only a successful body recipe is replayed into the canonical program authority, and one
  closed rebinder rewrites every checked type, closure, dispatch substitution, place, and witness.
- Successful body queries are replayed in canonical `BodyId` order before ownership, provenance,
  and loans run once over the complete program. Session never invokes body checking again for a
  complete query-owned body set.
- Canonical replay and whole-program authorities are exposed only through one finalization
  contract. Recipes own their body IDs, exact-current checked/failure outputs open explicit owned
  branches, and no caller can pair a recipe with a separately supplied identity.
- Authored typed-body rejection is an exact-current query value. Successful siblings replay into
  editor evidence while the rejecting body contributes its diagnostic and typed-interruption
  capability; session assembles `BodyAnalysisRecovery` without checking either body again.
- Authored lexical rejection retains its exact diagnostic and an optional source-neutral partial
  recipe. Canonical catalog materialization produces either complete names or one
  `NameAnalysisRecovery`; callers cannot provide a body ID separately from the recipe that owns it.
  The complete lexical catalog is the only input accepted by this materialization contract, so a
  consumer cannot reconstruct recovery by rerunning one rejected body independently.
- `SourceIndex` cannot affect a semantic decision.

The [checked-program boundary](../../../docs/checked-program-design.md) documents contracts shared
with adjacent stages.
