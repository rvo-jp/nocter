# nocter-syntax

## Responsibility

Transform one immutable source file into lexical tokens, diagnostics, and a lossless immutable syntax
tree for an explicit parse goal.

## Contract

The crate consumes `nocter-source` and the closed language vocabulary. It publishes token and node
identities, source-preserving trees, documentation trivia, structural queries, literal decoding,
and syntax-owned subtree completeness. It does not resolve names or apply semantic rules.

## Internal Responsibilities

- lexing and token subdivision
- event-based parsing and flat tree construction
- syntax diagnostics and missing/error elements
- structural navigation and documentation extraction
- exact node-completeness queries for recovery consumers

## Invariants

- Every syntax token retains its lexical-token identity and exact normalized range.
- Parser recovery preserves authored structure without inventing semantic success.
- A consumer never infers subtree completeness from file-wide diagnostic presence.
- Bounded ambiguity is parsed once transactionally rather than reparsed after lookahead.
