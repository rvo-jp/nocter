# nocter-declarations

## Responsibility

Own the immutable, syntax-independent declaration graph and its namespaces, visibility, callable,
requirement, standard-role, and target metadata. Own the separately indexed value authority that
is paired only by an accepted or recovery aggregate, never embedded in graph/type metadata.

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
- Graph/type integrity and declaration-value integrity are validated independently. The public
  builder publishes only an accepted aggregate containing both complete authorities.
- Namespace lookup consumes frozen tables rather than iterating declarations.
- A declaration identity never contains a source range or rendered name.
- Authored callable execution and guarantees are declaration data; consumers do not rediscover
  modifiers from syntax or result shapes.
- Invalid or incomplete graphs cannot be constructed as accepted programs.
- `DeclarationProgram` contains graph/type metadata only. Declaration records never embed
  evaluated constant or static payloads. Every accepted, rejected, and checking branch carries the
  same immutable value table beside that program.
- An accepted immutable program may create owned checking branches without rebuilding declaration
  decisions; every branch preserves semantic IDs and the type-authority lineage.
- Interface prerequisite cycles and effective member collisions cannot cross the accepted-program
  boundary.
- Dependency-cycle validation follows every interface predicate; member inheritance follows only
  contextual `Self impl Interface` predicates.
