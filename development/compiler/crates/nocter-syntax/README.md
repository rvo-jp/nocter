# nocter-syntax

## Responsibility

Transform one immutable source file into lexical tokens, diagnostics, and a lossless immutable syntax
tree for an explicit parse goal.

## Contract

The crate consumes `nocter-source` and the closed language vocabulary. It publishes token and node
identities, source-preserving trees, documentation trivia, structural queries, literal decoding,
syntax-owned subtree completeness, and a reusable parse product that binds only to equal normalized
source text. It also publishes a canonical declaration-syntax surface that excludes body contents,
trivia, and source identities without interpreting a declaration. Each pruned executable block is
published separately as exact normalized body bytes keyed by its stable declaration-surface
locator. Its source-syntax provider
contract lets callers choose direct or revisioned parsing without exposing either mechanism to
package or discovery code. It does not resolve names or apply semantic rules.

## Internal Responsibilities

- lexing and token subdivision
- event-based parsing and flat tree construction
- syntax diagnostics and missing/error elements
- structural navigation and documentation extraction
- exact node-completeness queries for recovery consumers
- validated rebinding of source-independent parse work into one current `SourceMap` identity domain
- source-neutral declaration-syntax canonicalization with explicit body pruning
- exact per-body source surfaces under stable declaration locators
- bidirectional body-local node/token locators for source-neutral semantic recipes
- the shared direct/reusable source-syntax provider boundary

## Invariants

- Every syntax token retains its lexical-token identity and exact normalized range.
- Parser recovery preserves authored structure without inventing semantic success.
- A consumer never infers subtree completeness from file-wide diagnostic presence.
- Bounded ambiguity is parsed once transactionally rather than reparsed after lookahead.
- Reusing parse work rewrites every embedded source identity and rejects unequal normalized text.
- Body edits and body-local diagnostics do not change the declaration-syntax surface; declaration
  shape, body presence, and declaration-local diagnostics do.
- Editing one body changes only that body's exact surface; unchanged sibling bodies retain equal
  locators and bytes.
