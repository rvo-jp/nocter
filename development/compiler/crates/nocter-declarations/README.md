# nocter-declarations

## Responsibility

Own the immutable, syntax-independent declaration graph and its namespaces, visibility, callable,
requirement, standard-role, and target metadata.

## Contract

The crate supplies domain types, builders, and validated immutable products. It knows semantic
identities and normalized declaration relationships but not source text, syntax trees, editor
coordinates, or checking internals.

## Internal Responsibilities

- declaration and member arenas
- module, import, and prelude namespaces
- callable guarantees, callable provenance, constant, and requirement contracts
- canonical interface dependency paths, `Self`-inheritance closure, and effective member identities
- visibility and path contracts
- accepted/recovery admission shapes

## Invariants

- Builders reserve and define every identity exactly once before freeze.
- Namespace lookup consumes frozen tables rather than iterating declarations.
- A declaration identity never contains a source range or rendered name.
- Authored callable guarantees are declaration data; consumers do not rediscover modifiers from
  syntax.
- Invalid or incomplete graphs cannot be constructed as accepted programs.
- An accepted immutable program may create owned checking branches without rebuilding declaration
  decisions; every branch preserves semantic IDs and the type-authority lineage.
- Interface prerequisite cycles and effective member collisions cannot cross the accepted-program
  boundary.
- Dependency-cycle validation follows every interface predicate; member inheritance follows only
  contextual `Self impl Interface` predicates.
