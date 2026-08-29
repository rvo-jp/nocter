# nocter-discovery

## Responsibility

Discover the complete physical source and module graph for one package compilation request.

## Contract

Discovery consumes a resolved package graph with its package-root catalog, immutable filesystem
view, selected target, and toolchain contract. It publishes reached sources, syntax trees, module
ownership, source-visibility edges, imports needed for traversal, and one closed compile input. It
requests syntax through a narrow source-syntax provider and validates the returned source identity;
it does not know whether parsing was direct or reused. It does not lower declarations or interpret
body semantics.

## Internal Responsibilities

- module and source catalog construction
- `see` visibility graph traversal
- target-aware import traversal
- source/syntax snapshot retention for recovery
- source ingestion and validated source-syntax provider calls

## Invariants

- Physical placement determines module ownership; `see` controls direct source visibility only.
- Complete `see`, `use`, and target-gate nodes survive unrelated syntax errors.
- Discovery never uses source or traversal order to choose between equal candidates.
- Package-boundary validation extends and reuses the graph's exact root catalog; it does not probe
  the same directory through a second source authority.
- Later stages receive the closed graph and cannot rediscover files.
- A syntax provider cannot change discovery topology or attach a tree from another source identity.
