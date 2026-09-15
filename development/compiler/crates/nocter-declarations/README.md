# nocter-declarations

## Responsibility

Own the immutable, syntax-independent declaration graph and its namespaces, visibility, callable,
requirement, standard-role, and target metadata, plus the separately indexed evaluated values that
complete constant and static declarations.

## Contract

The crate supplies domain types, builders, and validated immutable products. It knows semantic
identities and normalized declaration relationships but not source text, syntax trees, editor
coordinates, or checking internals.

## Internal Responsibilities

- declaration and member arenas
- one identity-indexed declaration-value table separate from constant and static metadata
- module, import, and prelude namespaces
- callable execution, guarantees, provenance, constant, and requirement contracts
- canonical interface dependency paths, `Self`-inheritance closure, and effective member identities
- visibility and path contracts
- accepted/recovery admission shapes

## Invariants

- Builders reserve and define every identity exactly once before freeze.
- Constant and static metadata complete together with their evaluated values; the public builder
  cannot publish either half independently.
- Namespace lookup consumes frozen tables rather than iterating declarations.
- A declaration identity never contains a source range or rendered name.
- Authored callable execution and guarantees are declaration data; consumers do not rediscover
  modifiers from syntax or result shapes.
- Invalid or incomplete graphs cannot be constructed as accepted programs.
- Declaration records never embed evaluated constant or static payloads. Every complete, rejected,
  and checking branch carries the same immutable value table beside the graph.
- An accepted immutable program may create owned checking branches without rebuilding declaration
  decisions; every branch preserves semantic IDs and the type-authority lineage.
- Interface prerequisite cycles and effective member collisions cannot cross the accepted-program
  boundary.
- Dependency-cycle validation follows every interface predicate; member inheritance follows only
  contextual `Self impl Interface` predicates.
