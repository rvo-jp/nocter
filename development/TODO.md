# Nocter Development Handoff

## Current Task

v0.11.0 Phase 8 is in progress. Implement the static opaque result type plan in
`milestones/v0.11.0.md` through its complete qualification gate, then stop at that checkpoint.

## Required Phase 8 Work

- add return-only `some Interface<Name = Type>` syntax with exact source ranges and contextual
  keyword behavior
- keep authored interface identity, declaration-scoped opaque identity, and inferred concrete
  lowering witness as separate facts
- integrate advertised conformance and associated bindings with ordinary type checking while
  routing layout, ABI, ownership cleanup, provenance, buildability, and IR through one lowering view
- preserve the opaque public contract across formatting, AST JSON, imports, diagnostics, and every
  LSP presentation/navigation surface
- migrate one practical distributed standard-library iterator API and pass the complete
  qualification matrix

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

Plan the next v0.11.0 phase before changing source syntax or expanding the standard-library
surface. Reuse the completed requirement, associated-type, and iterator foundations; do not add
name recognition, textual constraints, or API-specific solver paths. Keep the published v0.10.0
archive, tag, and release evidence immutable.
