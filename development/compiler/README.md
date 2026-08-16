# New Nocter Compiler

This directory is the implementation root for the specification-first Nocter compiler rewrite.
The lexical and syntactic grammar gate is closed. The completed Phase 1 workspace owns normalized
source storage, lexical projection, an immutable syntax arena, parser diagnostics, and the complete
G001-G033 recognition boundary from source roots through declarations, types, blocks, statements,
patterns, and expressions.

## Authority

The compiler derives public behavior from [`spec/`](../../spec/README.md). Missing language rules
block implementation; they are not inferred from the archived compiler, old tests, released
binaries, or historical implementation documents.

## Isolation

The new compiler must never depend on, copy, execute, or compare itself with the compiler preserved
by commit `f6c08da3`. Existing standard-library implementation details are also not bootstrap
semantics. Public standard-library contracts come from the specification and will receive new
implementations after the required language foundation exists.

## Planned Dependency Direction

```text
source
  -> syntax
  -> semantic core
  -> analysis and checked program
  -> executable program
  -> MIR
  -> machine program and code generation
  -> CLI and editor adapters
```

Later stages cannot import syntax representations to reconstruct earlier decisions. Source ranges
remain outside semantic identity, and runtime linkage is a one-way output projection.

The Cargo workspace must begin with source and syntax responsibilities only. Its parser fixtures
derive from the [grammar conformance plan](../docs/grammar-conformance.md); semantic crates cannot
be introduced to make an unresolved syntax choice.

## Current Crates

- `nocter-source` owns source identities, CRLF normalization, normalized byte spans, and line
  projection.
- `nocter-syntax` owns lexical tokens, exact reserved keywords and punctuation, comment metadata,
  joint-token facts, string/interpolation boundaries, lexical and parse diagnostics, and the
  lossless syntax tree. Its parser covers the complete normative grammar, including token-only
  ambiguity decisions, continuation-newline ownership, body-result classification, control-header
  brace ownership, and bounded malformed-source recovery.
- `nocter-model` owns typed semantic ID domains, the canonical compile-unit symbol table,
  normalized parameter-origin sets, and interned structural types. It has no crate dependencies;
  source spans, syntax nodes, and rendered type names cannot enter its identities or interning
  keys. An interface-owned `Self` has a canonical interface-identity placeholder distinct from
  explicit generic parameters and nominal applications; conformance specialization can therefore
  substitute it without inventing an implicit binder.
- `nocter-declarations` owns the immutable declaration-program spine: exact package-and-module
  identities, normalized visibility boundaries, package targets, imports, every declaration and
  member domain, generic requirements, bodies, opaque results, and the compile-unit type store. A
  two-pass reservation builder supports recursive headers, then validates every reference and
  owner edge before freezing. It depends only on `nocter-model`.
- `nocter-source-index` owns the separate immutable projection between semantic entities and exact
  syntax-node or syntax-token origins. It indexes the same bindings independently by semantic
  identity and by source coordinate; semantic stages do not depend on it.
- `nocter-declaration-lowering` owns the one-way syntax-to-declaration boundary. Its input is an
  explicit package graph and module/source topology supplied by discovery; it never probes the
  filesystem. It validates declared-package and single-file layouts, canonicalizes package and
  module order, and requires one discovery-owned source-or-module target for every authored `use`.
  It validates that source composition stays private, same-module, and root-reachable, permits
  idempotent source cycles, rejects module import cycles, and never reinterprets canonical paths to
  recover a missing edge. It then constructs the compile-unit symbol table, inventories every
  declaration and member with its exact syntax owner, allocates stable topology identities, and
  records their source projections. The temporary surface inventory also enforces the root-source-
  only API boundary before semantic reservation. A canonical-header pass joins eligible public
  bodyless contracts
  to exactly one private implementation body without resolving names or types; both source forms
  therefore enter reservation through one representative identity. The reservation pass then
  allocates every recursively referenceable typed ID—including associated types—in canonical
  surface order. Header preparation resolves exact declaration names and normalized visibility,
  creates declaration sites, rejects deterministic namespace collisions, and only then projects
  named entities from their exact name tokens rather than whole declaration ranges. Generic
  preparation allocates binder identities from their already-reserved owners, carries immutable
  lexical scopes into members, reuses repeated declaration-pattern binders, rejects explicit
  duplicates and nested shadowing, and projects every authored binder occurrence. Joined contract
  and implementation sources share one generic identity sequence.
  Authored import preparation builds one visibility-bearing namespace per module. Direct
  declarations, private imports, scoped/public re-exports, selected aliases, and module namespaces
  use the same table and collision rule. Dependency modules are completed before importers;
  selected names must be accessible, re-exports cannot widen their targets, and source imports add
  no semantic import identity. Exact module paths and selected-name tokens project back to their
  resolved semantic entities. The compiler-selected standard prelude is a separate fallback table:
  authored names shadow it, it never becomes an implicit re-export, standard-package modules do
  not receive it, and source code cannot import the compiler-managed prelude explicitly.
  Header type binding then converts every type occurrence into a flat syntax-independent arena.
  It resolves module selections, authored and prelude names, generic identities and arity, `Self`
  ownership, fixed-array lengths, and structural-callable origin names exactly once. Alias
  applications and associated selections remain explicit bound nodes until the normalization pass
  has the requirements needed to resolve them. That pass expands generic aliases through an
  explicit evaluation stack, rejects expansion cycles, substitutes canonical binder identities,
  resolves `Self` and associated names, and interns structural results without introducing alias
  or name-based selection kinds. Declaration target patterns use the same module,
  symbol, arity, and source-projection context and bind their bare argument names directly to the
  generic identities already allocated for that declaration. Nominal interface and structural
  callable capabilities also reuse this path resolver and flat type arena; capability syntax
  cannot establish an alternate lookup or callable-provenance path. Generic predicates and
  associated-type bounds are then bound into one closed requirement representation. Directed
  pattern refinements, general equalities, capabilities, copy, operators, borrow coercions, and
  expansion retain semantic IDs and bound types only. Their normalized forms use the same type
  evaluator, so capability and predicate types cannot diverge from declaration types. Structural
  callable parameter spellings disappear after named origin candidates become canonical parameter
  positions. Opaque results use a dedicated binding path for their interface application,
  associated bindings, captured generic identities, outcome layers, and canonical opaque type;
  they do not become a callable-header exception. The parser now represents mandatory
  interface-member `pub` with the same `Visibility` node used by every other declaration, so this
  boundary requires no interface-specific visibility recovery. Declaration-surface traversal is
  non-recursive, keeping the complete boundary safe for the parser's 5,000-layer type contract.
  Header definition then allocates fields, parameters, receivers, requirements, and bodies in
  canonical order and completes every reserved declaration arena slot. Public contracts and
  private implementations retain one callable identity but receive distinct source roles.
  Authored result provenance is stored separately from the inference state that checked body
  analysis will produce. The compiler-selected standard package and its scalar, string, error, and
  slice attachment modules are recorded as exact semantic IDs; freeze-time validation never grants
  built-in authority from a path spelling. The completed builder validates all owner edges and
  declaration shapes before returning an immutable `DeclarationProgram` and independent
  `SourceIndex`; the syntax-owned surface inventory cannot cross that boundary. Production callers
  enter through `lower_compile_unit_declarations`, which owns the complete pass order from surface
  collection through graph freezing. Individual passes remain public only as independently
  testable compiler boundaries and cannot be reordered by a production caller.
  Source-backed failures use one `SourceDiagnostic` envelope for a stable code, primary origin,
  related notes, and correction guidance. A stage-specific diagnostic retains the semantic rule
  identity separately from that presentation envelope. Compiler-state inconsistencies remain typed
  internal errors and are never assigned a language diagnostic code merely because they crossed the
  production facade. Module-surface diagnostics select only authored root-versus-implementation
  violations; malformed syntax snapshots and incomplete discovery edges stay internal.

Accepted fixtures through G033 have human-readable node-shape snapshots. Accepted, rejected, and
semantic-boundary fixture groups all verify exact lexical-token projection; error recovery cannot
silently discard a token.

The remaining Phase 2 increment is a source-backed semantic diagnostic boundary. Declaration
validation must identify the exact semantic subject so diagnostics can project it through
`SourceIndex` without duplicating validation in lowering. Freeze-time authored-rule violations now
do this with stable `E0200`-`E0212` codes, primary and related declaration sites, and correction
guidance; malformed compiler-produced graphs remain a separate integrity-error class. The earlier
surface, contract, header, generic, import, and type-binding passes still require the same
diagnostic projection. No crate yet owns checked body semantics.

## Verification

Run from `development/compiler/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
