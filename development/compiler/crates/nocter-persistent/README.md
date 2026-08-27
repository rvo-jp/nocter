# nocter-persistent

## Responsibility

Provide dependency-free persistent vector and ordered-map storage for compiler authorities that need
cheap immutable branching.

## Contract

The crate exposes generic structural storage only to reviewed semantic owners. It has no knowledge
of Nocter types, compiler phases, recovery policy, or source identities.

## Invariants

- Cloning a collection shares unchanged roots.
- Updates copy only bounded structural paths.
- Iteration visits tree nodes linearly and does not restart lookup for every element.
- Domain-specific lineage and commit policy remain outside this crate.
