# Nocter Development Handoff

## Current Task

v0.10.0 Phase 0 is complete under [`milestones/v0.10.0.md`](milestones/v0.10.0.md). It replaced
module-path native-build exceptions with resolved runtime type capabilities and promoted every
built-in integer type to ordinary native execution. v0.9.0 remains published and audited under
[`releases/v0.9.0.md`](releases/v0.9.0.md).

## Completed Checkpoint

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

v0.10.0 Phase 0 is complete. Do not broaden its scope retroactively; choose the next milestone from
practical standard-library requirements. The published v0.9.0 archive and its evidence are
immutable.
