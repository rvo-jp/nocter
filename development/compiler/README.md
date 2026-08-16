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

Accepted fixtures through G033 have human-readable node-shape snapshots. Accepted, rejected, and
semantic-boundary fixture groups all verify exact lexical-token projection; error recovery cannot
silently discard a token.

Neither crate owns declaration identity, name resolution, semantic types, or checked semantics.
Those responsibilities begin in Phase 2 and cannot alter an accepted syntax-tree shape.

## Verification

Run from `development/compiler/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
