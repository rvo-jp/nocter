# nocter-discovery

## Responsibility

Discover the complete physical source and module graph for one package compilation request.

## Contract

Discovery consumes a resolved package graph, immutable filesystem view, selected target, and
toolchain contract. It publishes reached sources, syntax trees, module ownership, source-visibility
edges, imports needed for traversal, and one closed compile input. It does not lower declarations or
interpret body semantics.

## Internal Responsibilities

- module and source catalog construction
- `see` visibility graph traversal
- target-aware import traversal
- source/syntax snapshot retention for recovery

## Invariants

- Physical placement determines module ownership; `see` controls direct source visibility only.
- Complete `see`, `use`, and target-gate nodes survive unrelated syntax errors.
- Discovery never uses source or traversal order to choose between equal candidates.
- Later stages receive the closed graph and cannot rediscover files.
