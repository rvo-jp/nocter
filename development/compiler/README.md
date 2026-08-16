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
  keys.
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
  has the requirements needed to resolve them. Declaration target patterns use the same module,
  symbol, arity, and source-projection context and bind their bare argument names directly to the
  generic identities already allocated for that declaration.

Accepted fixtures through G033 have human-readable node-shape snapshots. Accepted, rejected, and
semantic-boundary fixture groups all verify exact lexical-token projection; error recovery cannot
silently discard a token.

The next Phase 2 increment will normalize aliases and associated selections into the structural
type store while resolving requirements. It will then define every declaration arena slot and
discard the temporary syntax-owned inventory and all mutable builders. No crate yet owns checked
body semantics.

## Verification

Run from `development/compiler/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
