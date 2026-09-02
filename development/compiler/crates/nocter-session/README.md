# nocter-session

## Responsibility

Translate one closed compiler-query outcome into target validation or explicit recovery evidence.

## Contract

The crate depends on `nocter-semantic-product` and accepts only the sole complete-or-incomplete unit
outcome from the semantic query graph.
It opens the supplied success or rejection branch and publishes one immutable session outcome
with capability-oriented semantic views for complete programs, typed bodies, lexical names, typed
interruptions, and exact repair authority.
Recovery storage variants and phase order remain private. The crate does not implement stage rules,
invoke semantic compiler stages, implement editor queries, generate native code, or project
protocol values.

## Internal Responsibilities

- closed complete-or-incomplete outcome translation
- complete diagnostic retention
- target and executable request composition
- semantic evidence handoff
- inseparable discovery-snapshot and session-outcome publication
- recovery-storage to query-capability projection
- executable and test selection

## Invariants

- The crate cannot invoke declaration lowering, preparation, name resolution, body checking, or
  semantic finalization.
- Successful and recovered evidence are exclusive variants.
- A later failure cannot expose an older successful program.
- Failure-specific repair evidence moves once into its typed recovery owner.
- Consumers cannot select raw recovery phases or reconstruct phase fallback order.
- An analyzed unit cannot pair semantic evidence with a different discovery snapshot.
- Bulk executable closure retains the declaring target program with every selected identity; a
  consumer cannot combine identities and programs from separate compilations.
- Consuming an analyzed unit for compilation returns either its target or one failure envelope that
  retains the exact source snapshot, a non-empty ordered cause trace, and the complete
  query-selected diagnostic set. The first cause remains authoritative; causes reached while
  continuing editor recovery cannot be dropped or reconstructed downstream.
- The analyzed unit shares one immutable discovery snapshot with computation inputs; it never
  clones, rebuilds, or substitutes the source graph after a semantic query has consumed it.
- A checked query result has already crossed checking's paired exact-current transition. Session
  cannot materialize frontend bindings, reopen declaration preparation, or select a body-symbol
  generation.
- Declaration rejection opens an owned branch of the query's exact-current recovery
  product. Session composes its diagnostics and editor evidence without invoking declaration
  lowering a second time.
- Declaration-only evidence retains the originating lowering or checking recovery product intact;
  session never reconstructs an authority from independently supplied graph, type, ownership, and
  source-index values.
- Session maps only the semantic component of `CheckedProgramOutput` through target validation.
  The output owner preserves the projection on success and reconstructs the exact original pair on
  rejection; target and session never manipulate a free `CheckedProgram + SourceIndex` pair.
- Compiler-domain query failure retains its typed originating cause rather than being translated
  into a generic missing-authority state.
- Lexical rejection opens the finalization query's exact-current recovery branch.
  Session opens the checking-owned failure branch without reconstructing its error variant or
  invoking name resolution, checking preparation, or body checking.
- No public query-continuation entry point accepts declarations, prepared programs, body-name sets,
  or typed-body sets. A source-complete query consumer must supply one closed final success or
  rejection branch.
- Query-backed analysis has one public consumer accepting the exact-current unit product. It opens
  either the complete or incomplete compiler-domain branch; branch-specific translators remain
  private, so callers cannot choose syntax, declaration, preparation, lexical, body, or
  finalization order.
