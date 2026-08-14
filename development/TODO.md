# Nocter Development Handoff

## Current Task

v0.14.0 Phases 0 through 2 are complete. Phase 3 is in progress. The production MIR route now owns
the scalar control-flow subset described below, nested value/recovery blocks, scalar borrow
bindings and lexical loan endings, borrow-parameter forwarding, copy aggregate call-result locals,
fields, forwarding, fixed-array index loans, payload-variant construction, and active-tag cleanup
for enums and outcomes. One construction context owns mutable MIR state and one backend
context projects the checked body. MIR construction and validation are authoritative after route
selection; buildability and machine-IR lowering consume the same retained body and identity-backed
call edges. The next boundary is partially initialized aggregate ownership and projected aggregate
moves. Do not teach the backend to rediscover drop glue from an AST/type name. After that, expand
regions, closures,
typed literals, interpolation, expansion, and iteration until the AST lowering/buildability routes
can be deleted. Do not add language features or standard-library APIs during the migration. The
v0.13.0 tag, archive, release notes, and qualification record are immutable.

## Active v0.14.0 Phase 3 MIR Checkpoint

- MIR body-local identities, places, checked scalar locals, operands, arithmetic and comparison
  rvalues, basic blocks, `Goto`, `Switch`, direct `Call`, and `Return` are implemented and
  structurally verified
- parameters and bindings retain `LocalSymbolId`; temporaries retain `ExprId`; every local retains
  `TyId` and its checked scalar representation
- typed expressions retain intrinsic and contextual `TyId` separately, so lowering consumes the
  checked effective type without rewriting the expression's authored type
- entry and ordinary functions share one route selector; selected MIR failures never fall back to
  AST lowering
- scalar `if` expressions and terminal `if` statements produce the same diamond and return join
- one compile-unit cache retains checked MIR for buildability and lowering; MIR-routed bodies bypass
  AST buildability traversal, and calls carry canonical `DefId` targets plus explicit returning or
  non-returning continuations
- a dedicated control-flow builder owns incomplete blocks during construction; completed MIR never
  contains placeholder terminators, and direct calls split straight-line bindings, assignments,
  nested arguments, and operands into ordered CFG edges
- checked expression facts retain divergence independently from contextual result type, so
  non-returning calls do not depend on matching the authored spelling `never`
- MIR-to-IR structuring follows each linear branch path to its common join, so branch-local calls do
  not require AST-shaped lowering or a one-block branch restriction
- scalar fallible calls have explicit success and failure blocks in MIR; failure terminators
  distinguish trapping from propagation, and fallible bodies wrap successful returns explicitly
- scalar optional/fallible `otherwise` calls use the same outcome continuation with a lexical
  failure branch that assigns the destination and rejoins success; machine IR `Recover` is derived
  from that CFG instead of being selected by AST outcome syntax
- `catch _` for scalar fallible calls shares the same recovery CFG without manufacturing an error
  binding; discarded error payloads stay absent from MIR locals and backend recovery skips the
  success payload write after executing the failure branch
- unused named scalar catches retain one logical `error` local that is initialized only on the
  failure edge; machine-storage projection alone expands that value into its code and message
  views, while catches that inspect the payload remain on the legacy route
- buildability classifies plain and fallible scalar returns before constructing the shared MIR body;
  MIR validation rejects propagation from plain bodies, and MIR-to-IR derives its outcome failure
  mode from the checked failure edge
- scalar `while` statements lower to a condition header, body back-edge, and exit block; `break` and
  `continue` normalize to exit and back-edges instead of surviving as syntax-specific MIR nodes
- loop conditions and linear bodies may contain ordinary, trapping, and propagating scalar calls;
  dedicated MIR-to-IR loop structuring discovers the complete condition path rather than assuming
  that the condition occupies one block
- conditional loop-body branches classify each terminal edge as a local join, back-edge, or loop
  exit; conditional `continue` and `break` therefore remain ordinary checked CFG edges until
  machine-IR structuring
- checked bodies retain validated `LoopRegion` records alongside CFG edges, so machine-IR lowering
  does not rediscover loop structure from incidental block shape; range loops use an explicit
  increment target while ordinary `while` loops continue directly to their header
- MIR origins distinguish authored `ExprId` operations from compiler-desugared operations; scalar
  range comparison and increment operations therefore do not borrow unrelated expression identity
- the checked type arena interns foundational scalar types for compiler-generated typed operations,
  and virtual comparison locals do not consume machine-local slots after condition inlining
- scalar `while`, unconditional `loop`, and i32/usize range loops, including `break`, `continue`,
  and post-loop shadowing, now use the same retained MIR body in buildability and lowering
- boolean `&&` and `||` lower to an explicit MIR switch with distinct right-hand and
  short-circuit paths followed by a shared join; calls and failures on the right-hand side are
  therefore evaluated only on the selected CFG edge
- expression-valued `if` uses one reusable MIR conditional builder at function tails, inside
  arithmetic, and in call arguments; each branch retains a child lexical scope and converges on
  the destination place instead of selecting a route by syntax position
- MIR locals now keep checked value representation, ownership behavior, logical storage, and
  source identity as independent contracts; validation rejects invalid combinations and duplicate
  parameter storage before lowering
- parameter storage records source ordinal rather than reusing an ABI word index; mixed-width and
  aggregate parameter layouts are a machine-IR projection instead of leaking into MIR identity
- one backend parameter projection maps each source ordinal to its validated scalar ABI word or
  aggregate staging slot, including parameters following multiword values
- copy aggregate parameters retain aggregate MIR representation; checked field selections become
  typed projection paths and backend lowering maps them to the shared ABI staging-slot projection
- one immutable MIR build-input bundle carries the semantic database, current resolver,
  compile-unit resolver map, and checked HIR; aggregate layout no longer silently narrows to the
  current source while selecting the MIR route
- copy aggregate parameter forwarding uses MIR call arguments and the validated direct/indirect
  parameter classification; indirect stack-slot arguments prohibit tail-call frame reuse
- nested field access retains one parent-linked projection segment per checked field and folds
  relative offsets only at machine-storage projection
- canonical integer identity extends MIR scalar construction and machine projection to all
  built-in widths for parameters, arithmetic, shifts, comparisons, calls, fields, and returns
- legacy-specialized `u8` has its own MIR scalar identity and projects to the existing `U8`
  parameter, local, call, field-load, arithmetic, comparison, outcome, and return storage; it no
  longer falls out of the integer route because generic word slots cannot describe its ABI
- authored numeric negation and boolean inversion retain dedicated checked MIR unary operations;
  structural validation rejects operator/scalar drift before backend instruction selection
- explicit integer `as` expressions selected as exact or lossless by type checking retain source
  and target `TyId`/scalar identities in MIR; the verifier independently rejects lossy casts and
  backend projection performs the required sign or zero extension
- scalar compound assignments normalize to one MIR read-modify-write assignment using the same
  checked binary operators as ordinary expressions; all integer storage classes share this route
- logical locals no longer become storage-less `Virtual` places for loop optimization; a dedicated
  MIR-to-IR storage projection omits only proven single-definition/single-use loop conditions
- MIR retains a validated lexical `ScopeId` tree on locals and basic blocks; branch joins and loop
  exits can derive inner-to-outer scope-exit order without revisiting AST block shape
- range setup runs in an explicit loop-scope preheader, so a later failing aggregate initializer
  cannot acquire a loop-owned value while its CFG block still belongs to the parent scope
- a fixed-point definite-initialization pass validates every reachable MIR operand and return;
  branch joins intersect initialized sets, and fallible calls initialize their destination only on
  the success edge
- one dense body-local dataflow set backs initialization and is the shared state domain for the
  upcoming move, loan, and drop-obligation analyses; CFG joins do not allocate hash sets
- MIR distinguishes copy and move operands; move consumes definite-initialization state, so a later
  use on any reachable path is rejected before machine IR, while static copy/move/borrow behavior
  remains independent from representation and drop shape
- `Drop` is an explicit MIR terminator, and a separate fixed-point obligation pass uses may-live
  and must-live sets to reject leaks, overwrites, double drops, and cleanup reached after a move;
  conditional ownership is therefore not flattened into one unreliable live flag
- borrows have body-local `LoanId`, explicit `BeginLoan`/`EndLoan` statements, source and
  destination places, mutability, and lexical scope; one fixed-point loan pass rejects conflicting
  borrows, mutation or move while borrowed, path-dependent invalid ends, and live loans at exits
- loan conflicts use projected-place overlap instead of root-local equality: roots overlap children,
  ancestors overlap descendants, distinct fields are disjoint, and index projections remain
  conservatively aliasing until their indices can be proven distinct
- initialization, ownership obligations, and loans share one typed dense-set implementation while
  keeping their `LocalId` and `LoanId` domains statically distinct
- aggregate fields and indexes use body-local `ProjectionPathId` records rooted at a `LocalId`;
  each path retains its checked type, representation, ownership, parent path, and layout operation,
  so later analyses do not recover field or index meaning from AST member expressions
- definite-initialization analysis retains edge and exit states; cleanup materialization combines
  those states with `ScopeId` transitions to insert reverse-declaration-order `Drop` chains on
  ordinary, branch, call-success, call-failure, return, and propagation paths
- initialization and drop-obligation dataflow now uses a shared projected-place state: whole roots,
  explicitly initialized projections, and partially moved projections remain distinct across CFG
  joins, so moving one field neither consumes its siblings nor leaves the root falsely available
- cleanup materialization drops an available owned root once, or the maximal remaining owned
  projections after a partial move; moved fields are not destroyed and sibling destructors are not
  lost
- one MIR `Call` terminator now carries the checked value representation of every argument rather
  than a scalar-only tag; scalar machine-IR lowering is an explicit projection, while aggregate
  and borrow call routes can reuse the same call identity and continuation model
- primitive source names are recognized once as a closed `IntrinsicId` domain; pointer, view,
  process, allocation, I/O, and syscall lowering no longer dispatches backend semantics by string
  comparison
- MIR construction has one `finalize` boundary that rejects invalid initialization, materializes
  cleanup, and validates the completed body; retained consumers cannot observe construction-only
  MIR or independently decide when cleanup insertion occurs
- semantic MIR drop plans now carry destructor `DefId`s plus recursive struct/array structure;
  backend symbol names and ABI offsets are projected only while lowering checked MIR
- owned aggregate parameters use the production MIR route, including direct, nested-field, and
  reverse-order fixed-array destruction; move-only values without runtime cleanup use an explicit
  no-op ownership plan
- scalar-field owned struct literals now use an aggregate MIR rvalue and the same cleanup model;
  MIR branch/loop structuring follows materialized cleanup chains, including break and continue
- nested struct and fixed-array literals flatten into scalar leaves under semantic field/index
  paths; validation owns path uniqueness while ABI offsets remain a backend-only projection
- aggregate-field borrow bindings retain projected MIR loans, and owned aggregate call arguments
  transfer their initialization/drop obligation through an explicit move operand
- direct borrow arguments also use temporary MIR loans; a late single-use projection elides their
  machine slot without erasing the semantic loan or weakening conflict validation
- checked fixed-array element borrows now append an index segment to the same MIR projection arena;
  the segment retains its `usize` operand, length, and stride, while backend projection supplies
  the existing bounds-checked aggregate address operation for both constant and dynamic indexes
- fixed-array scalar reads, assignments, and compound assignments use the same index place;
  assignment validation reads the projected type and representation, while one backend scalar
  projection emits field or bounds-checked index loads and stores
- lexical allocation regions have body-local identities, scope ownership, explicit entry/exit
  statements, and cleanup-generated exits on every CFG edge; routed bodies no longer depend on
  the AST lowerer's parallel region-cleanup stack
- generic destruction stays keyed by `DefId` and concrete `TyId` until backend projection selects
  the monomorphized runtime symbol
- payload variant rvalues retain the selected variant `DefId` and semantic payload paths; enum drop
  plans test the stored tag and destroy only the active payload fields
- optional and fallible drop plans retain ordered outcome layers plus the payload `TyId`; backend
  projection derives every tag and payload offset from the shared outcome storage layout and drops
  the payload only through nested success edges
- named-field moves use the same projection arena as borrows and initialization; the arena expands
  the owned sibling tree before a partial move, so cleanup destroys each remaining field exactly
  once and never reconstructs source member expressions
- direct and indirect aggregate call arguments can address a projected slot range without copying
  it into a temporary whole-value slot
- aggregate construction has explicit begin, projected-field initialization, and finish statements;
  failure during a later field drops only completed owned children through ordinary place-state
  cleanup, without runtime live flags or an atomic aggregate rvalue
- the next checkpoint migrates stored optional/fallible values, useful named-error views, and the
  remaining advanced-expression families
- value blocks retain whether their tail is an implicit value or an explicit `return`; terminal
  recovery branches write return storage and exit, while value fallbacks assign the call
  destination and rejoin success
- backend conditionals preserve one shared return join, and obsolete exact-IR tests no longer
  require AST-era temporary reuse or nested short-circuit instruction shape
- all 2,660 library tests pass at this checkpoint

## Completed v0.14.0 Phase 2 Editor Projection Checkpoint

- successful hover, completion, navigation, references, rename, semantic tokens, signature help,
  and edits consume retained semantic, syntax, and lexical-scope indexes
- incomplete-source AST walkers are isolated under recovery entry points and cannot override a
  successful semantic result
- type and generic-parameter occurrences select declarations by `DefId`; binding type, scalar
  view, mutability, and payload facts use `LocalSymbolId`
- instance callable syntax indexes methods, operators, and coercions with their generic owner; a
  regression covers sequence-expansion specialization through an instance operator
- the complete repository and distributed installed-home matrix, formatting, warnings-denied
  Clippy, documentation generation, and diff gates pass

## Completed v0.14.0 Phase 1 Typed HIR Checkpoint

- every authored expression has an `ExprId` and owning `BodyId`; normalized checked types use a
  checked-file `TyId` arena within the compile-unit generation
- `TypedExpression` stores known or explicit error semantics, so diagnostics do not erase the
  partial typed result needed by editor recovery
- one `TypecheckOutput` owns diagnostics and `TypedHir`; normal compile-unit analysis and
  single-file LSP recovery retain that exact output
- the raw typed-HIR builder is private to type checking; only the named opaque-result prepass may
  construct a provisional product before final witness elaboration
- downstream analysis, specialization, buildability, and IR fields consistently refer to
  `typed_hir`; the former fact-collector API and independently recollected normal-analysis path are
  gone
- the complete repository verification matrix, formatting, warnings-denied Clippy, and diff gates
  pass

## Completed v0.14.0 Phase 0 Identity Checkpoint

- `InstanceDecl` owns one source-ordered member sequence across parser, formatter, AST JSON,
  resolver, checker, analysis, and lowering consumers
- one shared `SemanticDb` indexes top-level, member, and nested block-import declarations and
  survives opaque-result elaboration without creating mixed semantic generations
- resolver symbols retain `SymbolId` only as a local table handle and carry compile-unit `DefId`
  for declaration identity
- borrow coercions retain `DefId` from source surface through selection, facts, generic evidence,
  specialization, and IR callable indexing
- coercion plans no longer store declaration spans or generated target strings as identity;
  backend-only symbol presentation lives in its own lowering file
- trusted declaration roles are bound from validated source inputs into a `DefId`-keyed semantic
  map; lookup remains stable across a declaration's full and focused spans
- protocol-method facts and method specializations carry `DefId`, and buildability plus IR consume
  one `DefId`-keyed method-specialization table
- callable contracts, implementation bodies, receivers, parameters, and literal captures are
  paired by `DefId`; source spans remain only as diagnostic and presentation locations
- `CompileUnit`, callable pairing, resolution, trusted facts, and post-opaque elaboration share one
  `Arc<SemanticDb>` instead of rebuilding independent semantic generations
- every authored callable block and nested closure has a source-ordered `BodyId` with an explicit
  owner and parent; closure specialization is keyed by `BodyId`
- function, method, coercion, destructor, literal, and closure specialization indexes use `DefId`
  or `BodyId` rather than a declaration or expression span
- trusted String/Format/Iterator runtime descriptors convert validated source locations into
  `DefId` facts once; type checking, analysis, buildability, and lowering consume those facts
- source-backed editor identities use `DefId`; generic parameters use owner-relative ordinal
  identity and project to spans only for navigation or edits
- callable AST is name-free below actual named methods; operators and coercions no longer create
  synthetic `MethodDecl` identities
- buildability and IR callable names are keyed by canonical `DefId`; specialization walks
  definition-owned body records, including operator and coercion bodies, instead of matching spans
- the complete repository test suite, formatting, warnings-denied Clippy, documentation
  generation, and diff checks pass at this checkpoint

## Completed v0.13.0 Stabilization Checkpoint

- the Phase 0 through Phase 6 public contract is frozen; the architecture audit separates known
  v0.14.0 migration work from v0.13.0 public behavior instead of adding release-time workarounds
- candidate identity is `0.13.0` across Cargo metadata, the lockfile, installed `VERSION`, the
  distribution manifest and archive name, `std/nocter.nct`, CLI output, and LSP initialization
- incremental and clean verification each passed all 3,584 tests, formatting, warnings-denied
  Clippy, public examples, source corpus, and the distributed installed-home suite
- the clean run followed removal of 424 candidate build files totaling 643.9 MiB; documentation
  generation produced 143 pages and `git diff --check` passed
- the two-build archive comparison and complete isolated fresh-install matrix passed without
  `NOCTER_HOME`
- the retained 3,888,262-byte `arm64-darwin` archive has SHA-256
  `515b4c696bfc3f3a9bd96d9278904d16ca70a8cd32d57ddaa159836aa04bd761` and contains all 27
  standard-library files
- release-content commit `718b8ca80ec96553f4e4d13d054a9f9eca3f1e70` is frozen; publication must not
  rebuild or replace its qualified archive
- annotated tag `v0.13.0` resolves to publication commit
  `07dec92c87d5209b85d6e807404158742d270bfe`
- GitHub resolves v0.13.0 as the latest release with exactly the qualified archive and marks it
  neither draft nor prerelease
- a separate public download reproduced the qualified bytes, size, and SHA-256 and passed version,
  installed-home, locked/offline package, native test, deterministic graph, run, build, direct
  execution, and framed LSP checks

## Completed v0.13.0 Phase 6 Checkpoint

- borrow coercions are ordinary `instance` members; the standalone declaration and synthetic
  instance adapter no longer exist
- dedicated `where Source as Target` evidence is resolved through the same one-step selector used
  by contextual, explicit, receiver, comparison, and indexing conversion
- generic evidence specializes to the concrete accessible declaration before analysis and
  lowering, without runtime witnesses, nominal recognition, or a parallel compatibility path
- standard source, fixtures, normalized LSP presentation, specification, implementation documents,
  and the generated website agree on the new syntax and responsibility boundary
- all 3,584 tests, `cargo check`, formatting, warnings-denied Clippy, 141-page documentation
  generation, and diff verification pass

## Completed v0.13.0 Phase 5 Checkpoint

- `[T]` is the canonical owner of contiguous borrowed access and indexing; `Vec<T>` reuses it
  through coercion, and `Sequence<T>` is removed
- exact public construct, receiver, I/O, iterator, and nonfallible formatting forwarding surfaces
  are removed without aliases; distinct failure and ownership contracts remain public
- `std/testing.assert_eq` uses structural equality, while obsolete imports fail through ordinary
  resolver and visibility diagnostics
- standard source, distributed fixtures, LSP presentation, specification, examples, and generated
  website content agree on canonical declarations
- all 3,577 tests, `cargo check`, formatting, warnings-denied Clippy, 140-page documentation
  generation, and diff verification pass

## Completed v0.13.0 Phase 4 Checkpoint

- one fixed binary-comparison declaration and immutable plan serve equality and source-defined
  strict ordering while their generic evidence remains independent
- `<` is the sole authored strict-order primitive; `>`, `<=`, and `>=` select that declaration with
  recorded semantic orientation and boolean inversion
- reversed comparison evaluates source left then source right exactly once before swapping stable
  ABI arguments, so declaration selection cannot alter language evaluation order
- `str` and `[T]` own lexical ordering in standard source; `String` and `Vec<T>` reach it only
  through existing readonly coercions and no compiler-recognized nominal type path
- diagnostics, formatting, AST JSON, hover, completion, semantic tokens, definition, references,
  rename, specification, implementation documents, examples, and fixtures agree on one identity
- all 3,577 tests, `cargo check`, formatting, warnings-denied Clippy, 141-page documentation
  generation, and diff verification pass

## Completed v0.13.0 Phase 3 Checkpoint

- source-owned readonly, readwrite, and consuming expansion operators replace `Iterable` and
  `IntoIterator` as the sole collection-to-iterator conversion authority
- one immutable expansion plan serves collection `for`, sequence spread, generic requirements,
  specialization, ownership, provenance, buildability, lowering, diagnostics, and editor analysis
- `for item in &+source` yields mutable element borrows under one exclusive source loan and common
  path-sensitive cleanup; aggregate mutation uses a reusable aggregate-location borrow operation
- sequence spread uses the same readonly and consuming selector, retains exact-size and
  exactly-once rules, and rejects mutable expansion because packs retain multiple elements
- `Vec<T>` owns all three expansion forms in standard source, and a focused mutable view iterator
  provides allocation-free forward iteration without compiler recognition of collection names
- public specification, compiler architecture documentation, standard source, examples, source
  corpus, LSP presentation, and the generated website describe the same expansion model
- all 3,563 tests, `cargo check`, formatting, warnings-denied Clippy, documentation generation, and
  diff verification pass

## Completed v0.13.0 Phase 2 Checkpoint

- reachable `catch` blocks produce the operand success type and rejoin the surrounding expression;
  terminal handlers and empty `void` recovery remain valid
- one fallback-result abstraction serves `otherwise` and `catch` across scalar, borrow, view,
  aggregate, field, argument, assignment, return, stored-outcome, and composed-outcome destinations
- branch joins preserve return, provenance, borrow, initialization, ownership, and allocation
  facts for named and discarded error bindings
- runtime aggregate liveness prevents cleanup of uninitialized failed-call destinations and marks
  explicitly moved recovered values dead before cleanup
- diagnostics, formatting, AST JSON, hover, semantic tokens, definition, references, rename,
  completion, fixtures, examples, specification, and implementation documents agree on the same
  value-producing semantics
- all 3,555 tests, formatting, warnings-denied Clippy, documentation generation, and diff checks
  pass

## Completed v0.13.0 Phase 1 Checkpoint

- one built-in source authority registry governs construction, instances, conformances, implicit
  loading, and editor identity for every capability-bearing compiler-owned type
- `error` is the sole standard failure spelling and `error.new` resolves through the validated
  `std/error` source construct without spelling-based lowering
- `catch _` handles and discards a failure without producing a local, storage slot, provenance
  root, or editor occurrence
- the synthetic prelude exposes `String`, `Vec`, `Iterator`, `Iterable`, and `IntoIterator` as
  fallback names; explicit source names remain stable when the prelude grows
- compiler, distributed standard library, fixtures, specification, implementation documents, and
  generated website describe the same surface
- all 3,544 tests, formatting, warnings-denied Clippy, documentation generation, and diff checks
  pass

## Completed v0.13.0 Phase 0 Checkpoint

- source-defined readonly and readwrite index declarations share one instance operator-member
  model with equality and expose ordinary callable views to all downstream services
- one immutable index plan selects primitive projections, lexical requirements, direct
  declarations, and one-step-coerced declarations; analysis and lowering share specialization
- declared operations execute as static borrow-return calls for scalar and aggregate reads,
  assignment, generic requirements, dynamic fixed-array borrows, and owner-loan enforcement
- `Vec<T>` owns its index behavior in source, including checked move-only replacement and bounds
  behavior, without compiler recognition of collection names or representation
- hover, completion, semantic tokens, definition, references, rename, and source presentation use
  exact semantic identity and never expose compiler-private callable names
- public specification, compiler architecture documentation, standard source, the public example,
  source corpus, and generated website describe the same declaration and selection contract
- all 3,534 repository tests, formatting, warnings-denied Clippy, documentation generation, and
  diff verification pass

## Completed v0.12.0 Stabilization Checkpoint

- the contract audit found and removed one LSP interpolation label fallback; editor presentation
  now resolves the exact trusted `String` and `Format` declaration identities through shared
  compile-unit analysis
- public examples exercise readonly and readwrite Vec indexing through normal slice coercions,
  with no Vec-specific typecheck, ownership, IR, or backend execution path
- candidate identity is `0.12.0` across Cargo metadata, the lockfile, installed `VERSION`, the
  distribution manifest and archive name, `std/nocter.nct`, CLI output, and LSP initialization
- incremental and clean verification each passed all 3,520 tests, formatting, warnings-denied
  Clippy, public examples, source corpus, and the distributed installed-home suite
- the clean run followed removal of 421 candidate build files totaling 687.4 MiB; documentation
  generation produced 131 pages and `git diff --check` passed
- the two-build archive comparison and complete isolated fresh-install matrix passed without
  `NOCTER_HOME`
- the published 3,783,354-byte `arm64-darwin` archive has SHA-256
  `65514f5b5f5bddbbcd883b72026566109302e96203d3702503615ca26f2f4e60` and contains all 28
  standard-library files
- annotated tag `v0.12.0` resolves to publication commit
  `358576fbffd5b90b255a3362f4d82f607a7dd714`
- GitHub resolves v0.12.0 as the latest release with exactly the qualified archive and marks it
  neither draft nor prerelease
- a separate public download reproduced the qualified bytes, size, and SHA-256 and passed version,
  installed-home, locked/offline package, native test, deterministic graph, run, build, direct
  execution, and framed LSP checks

## Completed Test Performance Checkpoint

- the Cargo test profile compiles Nocter at optimization level 1 while retaining debug assertions;
  compiler-heavy integration tests no longer execute an unoptimized compiler hundreds of times
- opt-in JSON phase timing and `scripts/benchmark-check.sh` provide process-cold medians without
  changing normal compiler output or turning machine-dependent elapsed time into a test assertion
- compile-unit resolution builds immutable module indexes once, and opaque-result elaboration
  re-resolves only the reverse import, same-module, and prelude dependency closure of changed
  witnesses instead of every loaded source
- the compile-unit context now owns declaration-source import qualification environments; each
  resolver reuses that immutable result and materializes hidden type surfaces only once per
  declaration source, while a fresh context invalidates the cache after opaque elaboration
- one `TypecheckCompileUnitContext` now computes the callable-provenance fixed point for the whole
  compile unit; source checks share that immutable result instead of rebuilding it once per file
- release `check examples/hello.nct` fell from 18.318 seconds through 8.661 and 1.906 seconds to a
  three-run median of 0.301 seconds; no remaining analysis phase dominates the process-cold check
- public examples no longer repeat successful check/build compilation; all documented runnable
  examples now build and execute, exposing and fixing instance equality method calls composed with
  short-circuit logic
- all 3,520 tests pass in 424.71 seconds, down from 686.78 seconds at the previous checkpoint; the
  largest CLI suite completes 473 tests in 162.10 seconds, distributed-home completes 218 tests in
  53.78 seconds instead of 131.25 seconds, and the public-example suite passes all five contracts
  in 2.06 seconds
- warnings-denied Clippy, formatting, generated documentation, and diff checks pass; the
  documentation build contains 129 pages

## Completed Test Consolidation Checkpoint

- every test layer now has an explicit ownership boundary; semantic matrices stay in compiler
  units while CLI build, CLI run, distributed-home, framed-LSP, corpus, and public-example suites
  retain only contracts that require their integration boundary
- 33 CLI build tests with byte-identical successful run programs were removed; build-only artifact,
  target, output-path, executable-format, and failure contracts remain
- distributed-home coverage fell from 236 Rust tests to 218 by combining private-surface checks and
  removing five builds subsumed by stronger native runs; distinct allocator failure, ownership,
  specialization, cleanup, package, and public-API behaviors remain isolated
- the consolidation removes 51 tests overall and approximately 32 compiler process launches while
  preserving independently identifiable diagnostics in recovering multi-import sources
- the full suite now passes all 3,514 tests in 2,808.33 seconds; distributed-home accounts for
  2,438.41 seconds, down from 4,868.51 seconds before consolidation
- warnings-denied Clippy, formatting, generated documentation, and diff checks pass; the
  documentation build contains 129 pages

## Completed v0.12.0 Phase 2 Checkpoint

- equality and index requirements share the uniform `where (operation): Result` AST, and the old
  equality spelling remains only in a focused rejection test
- one immutable index plan carries direct, generic, and one-step-coerced projections through type
  checking, ownership, specialization, IR, diagnostics, and editor analysis
- readonly and readwrite Vec indexing execute through existing slice coercions and checked slice
  lowering without a Vec-specific semantic or native path
- generic index requirements specialize for direct and user-owned coerced containers; indexed
  borrows retain the original owner loan and ambiguous coercions require an explicit `as`
- deferred indexed integer operands are snapshotted at the common lowering boundary, preserving
  left-to-right evaluation without forcing stable values into temporaries
- all 3,565 tests, warnings-denied Clippy, formatting, public examples, source corpus, generated
  documentation, and diff checks pass; the documentation build contains 128 pages

## Completed v0.12.0 Phase 1 Checkpoint

- fixed instance-owned `operator (&self == other: &Self): bool` declarations and structural
  `where (&T == &T): bool` requirements share one resolved equality plan
- exact, imported, generic, primitive, and one-step readonly-coercion selections execute through
  ordinary static calls; owned operands remain usable and ambiguity is diagnosed explicitly
- `str` equality serves all `str`/`String` combinations; slice, Vec, and iterator equality/search
  use the same contract with exactly-once move-only iterator cleanup
- hover, completion, semantic tokens, definition, references, and framed LSP preserve exact source
  syntax, ranges, and declaration identities without exposing the compiler-private callable name
- all 3,555 tests, warnings-denied Clippy, formatting, public examples, source corpus, generated
  documentation, and diff checks pass; the documentation build contains 127 pages

## Completed v0.12.0 Phase 0 Checkpoint

- the exact `std/fmt.Format` interface replaces the closed compiler formatter table
- standard source conformances cover `str`, `String`, `bool`, and every integer; project-owned
  built-in conformances are rejected by selected-package authority
- one `TypecheckProtocolMethod` model now serves interpolation, iteration, and sequence spread
  through type checking, specialization, buildability, IR, and analysis
- generic and imported nominal conformances execute through ordinary static dispatch; formatting
  borrows existing values and destroys move-only temporaries exactly once
- missing and spoofed conformance cases fail during type checking with exact identities and spans
- all 3,533 tests, warnings-denied Clippy, formatting, public examples, source corpus, generated
  documentation, and diff checks pass; the documentation build contains 125 pages

## Completed v0.11.0 Stabilization Checkpoint

- candidate identity is `0.11.0` across Cargo metadata, the lockfile, installed `VERSION`, the
  distribution manifest and archive name, `std/nocter.nct`, CLI output, and LSP initialization
- incremental and clean verification each passed all 3,527 tests, formatting, warnings-denied
  Clippy, public examples, source corpus, and the distributed installed-home suite
- the clean run followed removal of 409 candidate build files totaling 559.7 MiB; documentation
  generation produced 122 pages and `git diff --check` passed
- the two-build archive comparison and complete isolated fresh-install matrix passed without
  `NOCTER_HOME`
- the published 3,651,844-byte `arm64-darwin` archive has SHA-256
  `d2e1e11cdfdf666b0d3661cf44ad91fb5ffc92bd81bbb853245268a6288eedbb` and contains all 27
  standard-library files
- annotated tag `v0.11.0` resolves to publication commit
  `1a218b3ed5f68a9df38c3391a28b566afe895851`
- GitHub resolves v0.11.0 as the latest release with exactly the qualified archive and marks it
  neither draft nor prerelease
- a separate public download reproduced the qualified bytes, size, and SHA-256 and passed version,
  installed-home, locked/offline package, native test, deterministic graph, run, build, direct
  execution, and framed LSP checks

## Completed Phase 8 Checkpoint

- return-only `some Interface<Name = Type>` has contextual-keyword parsing, exact source ranges,
  canonical formatting, public AST JSON, and focused recovery diagnostics
- authored interface identity, declaration-scoped opaque identity, and the inferred concrete
  lowering witness remain separate; one lowering view serves layout, ABI, ownership, cleanup,
  provenance, buildability, specialization, and IR without leaking witness members
- conformance, associated bindings, generic substitution, optional/fallible outcomes, method
  dispatch, and path-sensitive exactly-once destruction operate through the opaque public type
- hover, completion, inlay hints, signature help, semantic tokens, definition, references, and
  rename preserve the authored contract and associated declaration identities
- `str.lines()` is the distributed standard-library pilot and returns
  `some Iterator<Item = &str>` while retaining its source loan
- all 3,527 tests passed; warnings-denied Clippy, formatting, documentation generation, and diff
  checks passed; the generated website contains 120 pages

## Completed Phase 7 Checkpoint

- one optional runtime live flag now extends the aggregate-local and pending-drop model only when
  a destructor-bearing move-only aggregate crosses path-sensitive control flow
- complete cleanup, partial cleanup, explicit destruction, transfer, initialization, and
  reinitialization share the same flag-aware lowering boundary and real evaluation order
- non-terminal branches, matches, loops, value control flow, and short-circuit conditions no
  longer require aggregate-specific buildability rejection paths
- focused IR tests preserve flag placement and straight-line zero-cost behavior; native tests
  observe exactly-once destruction on executed and skipped paths
- all 3,502 tests passed; warnings-denied Clippy, formatting, documentation generation, and diff
  checks passed; the generated website contains 119 pages

## Earlier Completed Checkpoints

- v0.11.0 Phase 6 separates unique type-family destruction from callable `instance` behavior with
  the independent `destruct TypePattern(&+self) { ... }` declaration
- one `DestructDecl` and `DestructSignature` identity now drives resolver uniqueness, generic
  substitution, type checking, ownership, provenance, cleanup facts, buildability, IR drop glue,
  formatting, AST JSON, and editor traversal
- invalid alias/view/copy targets, repeated generic binders, modifiers, clauses, duplicate
  declarations, and removed instance members receive focused diagnostics without compatibility AST
- the standard library, compiler corpus, public specification, contributor documentation, and
  generated 118-page website use the independent declaration; instances contain methods only
- all 3,497 tests passed; warnings-denied Clippy, formatting, documentation generation, and diff
  checks passed

- v0.11.0 Phase 5 replaces explicit `instance<T>` and `conform<T>` binder prefixes with
  source-backed declaration type patterns and directed `where Binder = Type` refinement
- one structural unifier drives alpha-renamed, repeated, refined, and canonical-name overlap;
  disjoint patterns select exact methods and conformances while overlaps are rejected without
  specialization ranking
- conditional destruction is rejected explicitly so generic ownership and ABI behavior remain a
  uniform property of each nominal type family
- AST JSON, formatting, qualification, type checking, specialization, associated types, LSP
  presentation and occurrences, the standard library, fixtures, specification, contributor docs,
  and the generated website share the new model
- all 3,491 tests passed; warnings-denied Clippy, formatting, documentation generation, and diff
  checks passed; the generated website contains 117 pages

- v0.11.0 Phase 4 replaces overloaded `impl` declarations with structurally separate `instance`
  behavior/destruction declarations and `conform` interface-proof declarations
- distinct AST member enums make associated bindings in instances and drop members in
  conformances unrepresentable; shared method-body consumers use a read-only owner view rather
  than an optional interface discriminator
- resolver, type checking, ownership, provenance, specialization, buildability, lowering,
  formatting, AST JSON, callable identity, and LSP/editor analysis consume the authored
  declaration kind
- the standard library, fixtures, active specification, contributor docs, and generated website
  use the new syntax; obsolete `impl` receives a directional removal diagnostic only
- all 3,482 tests passed; warnings-denied Clippy, formatting, documentation generation, and diff
  checks passed; the generated website contains 116 pages

- v0.11.0 Phase 3 makes generic parameter lists name-only and moves intrinsic copy, interface,
  callable, and equality constraints into one declaration-wide `where` clause model
- nominal requirements are enforced inside declarations and at every specialized type-use
  boundary; obsolete inline and colon-delimited copy forms have no compatibility path
- AST JSON, formatting, resolution, type checking, specialization, LSP presentation, completion
  recovery, standard-library source, fixtures, specification, and contributor documentation share
  the same constraint grammar
- all 3,479 tests passed; warnings-denied Clippy, formatting, documentation generation, and diff
  checks passed; the generated website contains 116 pages
- v0.11.0 Phase 2 adds associated-type capability bounds and one resolved equality relation shared
  by callable and impl `where` clauses, generic checking, conditional conformance, specialization,
  buildability, ownership, ABI classification, and lowering
- `Iterator.Item`, `Iterable.Iter`, and `IntoIterator.Iter` replace public courier parameters;
  trusted collection iteration retains exact interface and associated declaration identities even
  when two interfaces use the same member name
- adapters remove redundant item parameters, `chain` proves `R.Item = L.Item`, and Vec builders use
  equality constraints instead of unchecked result transport
- collection `for`, sequence spread, default methods, adapters, provenance, region constraints,
  move/drop cleanup, native execution, and LSP presentation pass against the distributed home
- `development/compiler/scripts/verify.sh` passed all 3,471 tests, formatting, and warnings-denied
  Clippy; documentation generation produced 116 pages and `git diff --check` passed
- v0.11.0 Phase 1 adds required interface associated types, exact conformance bindings,
  `Self.Item`/`T.Item`/concrete projections, and one recursive normalization service shared by
  signature compatibility, specialization, ownership, sizing, buildability, lowering, and LSP
- associated declarations, bindings, projected uses, and imported occurrences share the interface
  declaration's semantic identity for hover, completion, definition, references, rename, and
  semantic tokens; no `Iterator`, `Item`, `std`, or source-path recognition was added
- focused generic, concrete, nested, callable-`where`, imported, recovery, editor, and native
  execution tests pass; full qualification is recorded in `milestones/v0.11.0.md`
- v0.11.0 Phase 0 introduced resolved generic requirements and intrinsic `copy`; Phase 3 supersedes
  its original inline spelling with the single `where copy T` form
- `Vec.from_slice` and `Vec.try_from_slice` now state their copy precondition at the public boundary;
  the top-level forwarding APIs use inline requirements and no implementation-only limitation
  comment remains
- v0.10.0 stabilization requires the implicit versioned standard package instead of silently
  omitting it, preserves the same exact identity when `std` is the graph root, and orders graph
  namespaces deterministically
- incremental and clean verification each passed all 3,437 tests, formatting, warnings-denied
  Clippy, public examples, source corpus, and the distributed installed-home suite
- documentation generation produced 113 pages; the two-build archive comparison and complete
  isolated fresh-install matrix passed
- the published 3,465,760-byte `arm64-darwin` archive has SHA-256
  `fe47f69b274a23c8d83bd28d9bb28b3e3ee3a43f02bb16ed0151a42345ce61c9` and contains all 27
  standard-library files
- annotated tag `v0.10.0` resolves to publication commit
  `cc866309e6815670e1b7e558d461fcd7415111c4`
- GitHub resolves v0.10.0 as the latest release with exactly the qualified archive and marks it
  neither draft nor prerelease
- a separate public download reproduced the qualified size and SHA-256 and passed version,
  installed-home, locked/offline package, native test, deterministic graph, run, build, direct
  execution, and framed LSP checks

- v0.10.0 Phase 3 replaces `pub(nocter)` with private, descendant-module, ancestor-module,
  package, and universal visibility boundaries resolved from exact `PackageId` and `ModuleId`
- re-exports preserve original declaration navigation while applying their own non-widening
  boundary; imports, members, construction, coercion, diagnostics, and LSP auto-import consume the
  same semantic access model
- `std` is one implicit package selected and version-validated from the active Nocter home; the
  reserved alias never enters user dependencies or locks, and validation precedes generated-lock
  writes
- primitive and trusted runtime authority is independent of source visibility and requires the
  exact selected standard-library package identity
- the distributed library and fixtures use `pub(/)` with no compatibility grammar for
  `pub(nocter)`; all 3,433 tests, formatting, warnings-denied Clippy, generated documentation, and
  diff checks pass

- v0.10.0 Phase 2 adds one source-backed callable identity model for root contracts and focused
  implementation sources, including canonical receiver, parameter, and literal-capture identities
- recursive specialization, provenance, retained mutations, buildability, native lowering, and LSP
  navigation preserve the public contract while using physical body spans for implementation work
- standard-library public surfaces now live in `std/string`, `std/vec`, `std/io`, and `std/iter`;
  focused `.nct` sources hold implementations, while `std/io/buffer` and `std/internal/os` remain
  genuine child modules
- all 2,370 library tests, all 225 installed-home tests, warnings-denied Clippy, documentation
  generation, public examples, source corpus, CLI suites, formatting, and diff checks passed

- v0.10.0 Phase 1 replaces file-defined modules with directory-defined modules rooted at
  `index.nct`; `nocter.nct` now contains package documentation and directives only
- explicit same-module source imports compose one private namespace with idempotent cycles, while
  external paths address directory modules and their root-defined public surfaces only
- one source-layout layer supplies canonical module identity, source membership, ambiguity rules,
  target resolution, and unsaved-buffer matching to the compiler and LSP
- package targets use `module`, public declarations are confined to module roots, and the standard
  library, examples, fixtures, initialization output, and documentation use the new layout
- all 3,405 compiler tests, documentation generation, public example checks, formatting, and
  warnings-denied Clippy passed on the migrated tree

- v0.10.0 Phase 0 centralizes integer width, signedness, range, and ABI-word semantics in one
  descriptor consumed by ABI classification, IR lowering, and arm64 code generation
- all ten built-in integer types now execute through scalar bindings, calls, outcomes, aggregates,
  fixed arrays, slices, generic `Vec<T>`, iteration, arithmetic, comparison, shifts, conversion,
  and interpolation
- resolved element capabilities replace the former `std/vec.nct` filesystem exception; imported
  nested aliases preserve their concrete integer kind through locals, parameters, and aggregate
  fields
- `development/compiler/scripts/verify.sh` passed all 3,398 tests, formatting, and warnings-denied
  Clippy; documentation generation produced 107 pages

- v0.9.0 stabilization centralized built-in type identity and implementation-module authority in
  one registry and removed public `String`/`Vec<T>` borrowed-observation forwarding helpers
- focused regression coverage proves registry uniqueness and rejects external access to private raw
  view bridges while preserving source-owned `str` and `[T]` methods through receiver coercion
- incremental and clean verification each passed all 3,391 tests, formatting, warnings-denied
  Clippy, documentation, public examples, source corpus, and the distributed installed-home suite
- the 3,375,819-byte `arm64-darwin` archive with SHA-256
  `c11f7ea65f49a8061156e47af7621b46b2f86329d464a067a5efc036eecb0cf8` passed two-build content
  equivalence and the complete isolated fresh-install matrix
- annotated tag `v0.9.0` resolves to publication commit
  `8811508f1b3b19d30e5f768097c57e73ebe4bde6`
- GitHub resolves v0.9.0 as the latest release with exactly the qualified archive and marks it
  neither draft nor prerelease
- a separate public download reproduced the qualified size and SHA-256 and passed version,
  installed-home, locked/offline package, native test, deterministic graph, run, build, direct
  execution, and framed LSP checks

- v0.9.0 Phase 3 makes `std/str` and `std/slice` the exclusive source authorities for built-in
  view methods and restricts compiler roles to four exact representation primitives
- one ordinary receiver-conversion plan gives `String` and `Vec<T>` their borrowed view APIs
  without duplicate inherent methods; original methods and explicit conformances keep priority
- semantic expression type facts drive string and slice indexing, byte collection, assignment, and
  native lowering, including explicitly converted views and compound mutable access
- built-in method signatures retain canonical hidden imported dependencies; editor features use
  the selected source identity and canonical concrete receiver instead of synthetic owning methods
- deterministic surface collection, authority diagnostics, ownership and region tests, public
  examples, and the installed-home matrix close source-order, migration, and distribution gaps
- the final verification matrix passed all 3,389 tests, formatting, warnings-denied Clippy,
  documentation tests, generated documentation, public examples, source corpus, and the
  distributed installed-home suite

- v0.9.0 Phase 2 adds validated UTF-8 `get_range`, `strip_prefix`, and `strip_suffix` views plus
  allocation-free `SplitIter` and `LinesIter` state machines behind the stable `std/string` facade
- one exact trusted `BorrowedProjection` role and typed `SetStrSubview` IR instruction preserve
  source provenance without raw-pointer origin reconstruction
- shared byte search now lives in `std/string_search`, so owned and borrowed string algorithms use
  one implementation
- native coverage proves UTF-8 boundaries, owned-split parity, LF/CRLF behavior, iterator adapter
  dispatch, and allocator non-reachability; ownership coverage rejects mutation, move, drop, and
  region escape while allowing static and exact text-only origins
- comparison lowering now materializes scratch-using operands before codegen, preventing one
  indexed operand from overwriting another; specialized borrowed-view adapters transparently lower
  generic `move` markers
- LSP coverage verifies normalized hover, completion, signature help, definition, and semantic
  tokens through the public re-export
- the final `development/compiler/scripts/verify.sh` run passed all 3,370 tests, formatting,
  warnings-denied Clippy, documentation tests, public examples, source corpus, and the distributed
  installed-home suite; documentation generation produced 127 pages

- v0.9.0 Phase 1 makes omitted result origins declaration-stable through one resolved
  zero/one/ambiguous classifier shared by validation, summaries, calls, interface conformance,
  coercions, diagnostics, and editor analysis
- AST, JSON, formatter, hover, completion, signature help, and inlay hints preserve authored
  `from` syntax only; the standard library retains clauses only for genuinely ambiguous inputs
- body summaries remain exact for ordinary callables and fresh owned copies, while the typed
  sequence-literal boundary preserves borrowed element origins from an omitted unique pack
- fallible success origins, trusted static results, callable values, generics, aggregates,
  callbacks, iterators, allocators, imports, and interface implementations share the same elision
  semantics and region-escape checks
- the final verification passed all 3,359 tests, formatting, warnings-denied Clippy, documentation
  tests, generated documentation, public examples, source corpus, and the distributed installed-home
  suite

- source-level result allocation modifiers and callable allocation variance were removed
- `from` is the sole public result-storage relationship and names only caller-managed external
  origins
- public body validation rejects an undeclared external result origin; private body inference stays
  exact without creating public syntax
- fresh result storage remains compiler-owned and propagates through outcomes, aggregates,
  callbacks, generics, interfaces, iterators, retained mutation, and ownership transfer
- unknown bodyless storage-bearing results use type-directed conservative internal storage
- the distributed standard library, formatter, AST JSON, normalized notation, and every LSP surface
  use the same source contract
- public specification pages and compiler-development documentation were migrated to the
  compiler-owned result-storage model
- missing-`from` validation covers every externally callable body form, and interface conformance
  cannot introduce an external result origin absent from its contract
- typed sequence literal packs can be named by `from items`, with fixed and spread element origins
  instantiated from declaration identity
- clean and incremental `scripts/verify.sh` runs each passed all 3,284 tests, formatting,
  warnings-denied Clippy, public examples, source corpus, and the distributed installed-home suite
- the 3,285,691-byte `arm64-darwin` archive with SHA-256
  `080160481adbcb0b7f64ab87903b05814aad13fc16207dcc9602e655675f2d78` passed the complete fresh
  extraction smoke matrix
- annotated tag `v0.7.0` resolves to publication commit
  `966c4a3e398ae534ad84ca5c8a35ae5ff0fcfdc8`
- the public GitHub Release contains exactly the qualified archive and is neither a draft nor a
  prerelease
- a separate public download reproduced the qualified size and SHA-256 and passed version,
  installed-home, locked/offline package, native test, deterministic graph, run, build, direct
  execution, and framed LSP checks
- v0.9.0 Phase 0 moved portable `Reader` and `Writer` contracts into `std/io/core.nct`, added shared
  `read_to_end`, `read_to_string`, and `write_text` defaults, retained stable `std/io` re-exports,
  and simplified the public file-summary package
- imported interface-default specialization now aliases the requested concrete method target to
  its resolved implementation declaration, closing generic default dispatch across source modules
  without protocol-specific forwarding methods
- whole-stream native coverage includes empty and multi-chunk input, partial reads, split UTF-8,
  invalid UTF-8, propagated reader failure, impossible counts, `File`, `BufReader`, user readers,
  and `BufWriter`; LSP coverage verifies concrete hover, completion, and shared definition identity
- the final `development/compiler/scripts/verify.sh` run passed all 3,351 tests, formatting,
  warnings-denied Clippy, documentation tests, public examples, source corpus, and the distributed
  installed-home suite
- v0.8.0 Phase 0 adds type-owned `coerce` declarations using `as`, coherent resolver identities,
  one expected-type `CoercionPlan`, ownership/provenance/region/IR integration, standard-library
  `String` and `Vec<T>` entries, and normalized editor presentation
- typed bindings, simple assignment, callable arguments, struct fields, fixed-array elements, and
  callable returns all record and lower through the same coercion-plan path
- `development/compiler/scripts/verify.sh` passed all 3,310 tests, formatting, and warnings-denied
  Clippy; documentation generation produced 121 pages
- v0.8.0 Phase 1 introduces one conversion selector and immutable `ConversionPlan` for existing
  numeric conversion, capability weakening, contextual coercion, and explicit borrowed-view `as`
- compound expected-type propagation covers branches, outcome projection, typed sequence captures,
  and generic enum payloads; native borrow lowering consumes the same selected plans
- exact `as` hover and definition work through local and imported coercion identities, with precise
  missing-borrow, capability, visibility, and unsupported-pair diagnostics
- packaged `String` and `Vec<T>` explicit/contextual views pass check and native execution
- `development/compiler/scripts/verify.sh` passed all 3,334 tests, formatting, and warnings-denied
  Clippy; documentation generation produced 121 pages
- v0.8.0 Phase 2 preserved result loans through explicit conversion, projection, `if`, `if is`, and
  `match`, including branch-specific pattern environments
- process-level LSP coverage now includes local, re-exported, private, numeric, and incomplete
  explicit-conversion queries with exact ranges
- clean and incremental verification each passed 3,345 tests, formatting, and warnings-denied
  Clippy; documentation generation produced 123 pages
- the 3,319,650-byte `arm64-darwin` candidate archive with SHA-256
  `cb6f0ce6b81e1aa71a65797e21f9f1d05a4164a17cf76427f34955966a63298a` passed the complete fresh
  extraction smoke matrix without `NOCTER_HOME`
- annotated tag `v0.8.0` resolves to publication commit
  `aa678abfec1a643e956a252cfc6a08d8e14ae65e`
- GitHub resolves v0.8.0 as the latest release, containing exactly the qualified archive and marked
  neither draft nor prerelease
- a separate public download reproduced the qualified size and SHA-256 and passed version,
  installed-home, locked/offline package, native test, deterministic graph, run, build, direct
  execution, and framed LSP checks

The published v0.7.0 qualification remains in its immutable
[`releases/v0.7.0.md`](releases/v0.7.0.md) record. The completed v0.8.0 phases are recorded in
[`milestones/v0.8.0.md`](milestones/v0.8.0.md), and publication evidence is frozen in
[`releases/v0.8.0.md`](releases/v0.8.0.md).

## Next Work

Plan the next v0.13.0 phase before implementation. Do not alter the published v0.12.0 archive,
tag, or qualification evidence; published artifacts remain immutable.
