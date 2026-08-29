# nocter-computation

## Responsibility

Own revision-pinned demand evaluation, automatic query dependency recording, deterministic result
reuse, cycle detection, and computation accounting without owning compiler semantics.

## Contract

Domain owners define opaque keys, immutable values, deterministic fingerprints, and pure query
providers. The crate records which queries each provider reads and reuses a prior value only after
all recorded dependencies retain the same fingerprints. It does not interpret files, packages,
modules, declarations, types, source projections, diagnostics, or editor requests.

## Internal Responsibilities

- staged input changes published by atomic revision commit
- type-erased storage behind typed input and query APIs
- ordered automatic dependency capture
- clean-dependency validation and value-fingerprint propagation
- recursive-query cycle reporting
- execution and reuse counters for conformance tests

## Invariants

- Query providers cannot manually declare downstream invalidation.
- A cached value is visible only through its defining query type and exact stable key bytes.
- Generation-local semantic IDs are not accepted implicitly as stable keys.
- An input update and a query evaluation cannot overlap through the safe API.
- An unchanged result fingerprint stops invalidation propagation.
- Internal query failure is not stored as a successful domain value.
