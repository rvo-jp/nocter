# nocter-discovery

## Responsibility

Discover the complete physical source and module graph for one package compilation request.

## Contract

Discovery consumes a resolved package graph with its package-root catalog, immutable filesystem
view, selected target, and toolchain contract. It publishes reached sources, syntax trees, module
ownership, source-visibility edges, imports needed for traversal, and one closed compile input. It
requests syntax through a narrow source-syntax provider and validates the returned source identity;
it does not know whether parsing was direct or reused. It also publishes one canonical semantic
topology surface containing only discovery decisions that can affect declaration semantics. It
does not lower declarations or interpret body semantics.

## Internal Responsibilities

- module and source catalog construction
- `see` visibility graph traversal
- target-aware import traversal
- source/syntax snapshot retention for recovery
- source ingestion and validated source-syntax provider calls
- source-neutral canonicalization of package, module, top-level dependency, target, and toolchain
  topology
- one shared canonical source inventory for semantic topology and exact current-source products

## Invariants

- Physical placement determines module ownership; `see` controls direct source visibility only.
- Complete `see`, `use`, and target-gate nodes survive unrelated syntax errors.
- Discovery never uses source or traversal order to choose between equal candidates.
- Package-boundary validation extends and reuses the graph's exact root catalog; it does not probe
  the same directory through a second source authority.
- Later stages receive the closed graph and cannot rediscover files.
- A syntax provider cannot change discovery topology or attach a tree from another source identity.
- Production discovery requires a caller-owned syntax provider; it cannot construct a direct parser
  and bypass compiler-computation source authority.
- A module source always requests the source-file parse goal even when the same physical file was
  already parsed as a package declaration; path and source identity never erase parse-goal
  identity.
- The semantic topology surface contains no `SourceId`, `NodeId`, syntax coordinates, body-local
  import, or source contents. Its ordering and vocabulary come from owning contracts rather than
  Rust enum discriminants or discovery traversal order.
- Every resolution retained by the topology surface is checked against the exact expected syntax
  kind; malformed internal snapshots fail instead of silently changing invalidation behavior.
- The exact current-source surface uses the same canonical inventory and includes every reached
  source byte. It is a generation-local invalidation product, never declaration semantics.
