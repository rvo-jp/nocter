# Execution Contract Design

This document owns the compiler-internal boundary between authored callable promises, checked
execution facts, and consumers of those facts. Public meanings of `noalloc`, `blocking`, `async`,
and `future T` remain in `spec/`.

## Four Distinct Questions

The compiler must not collapse these questions into one flag set:

1. What did the declaration promise to its caller?
2. What happens while the callable is invoked?
3. For a deferred callable, what happens while the produced computation is driven or cancelled?
4. What happens when values retained by either scope are destroyed?

For an immediate callable, invocation executes the body and the first two execution scopes coincide.
For an asynchronous callable, invocation constructs an owning future while the body belongs to the
future's later drive scope. Returning `future T` from an ordinary function does not by itself make
that function an asynchronous body, and a later stage may not infer scheduling from result shape.

## Authorities

### Authored Contract

`nocter-model` owns dependency-light structural callable guarantees. `nocter-declarations` owns a
named declaration's resolved execution mode, result identities, and those guarantees. Declaration
lowering is the only syntax-to-contract projection.

An authored contract may be weakened through an explicit checked conversion. Inferred facts never
strengthen it. Presentation reads the authored contract, so an implementation that happens not to
allocate does not acquire a displayed `noalloc` promise.

### Checked Execution Facts

`nocter-checking` owns implementation facts. It derives them from checked operations, frozen
dispatch, closure identities, and ownership-owned cleanup schedules. The fact domain is a positive
least-fixed-point lattice: a root begins without a positive allocation or blocking fact and gains
one when a direct operation or reachable execution edge proves it possible.

Missing semantic identities are integrity failures. They do not become conservative effect values;
conservative values are valid only for explicit external-contract edges whose implementations are
unavailable by design.

### Primitive Facts

`nocter-runtime-contract` owns closed primitive-role facts. Standard source declares a public
contract; target validation binds that declaration to one primitive role and compares the two.
Neither checking nor target validation derives primitive behavior from a function name, module
path, return type, or generated instruction sequence.

## Execution Edges

Relation collection produces typed edges rather than asking fixed-point evaluation to interpret
checked operations again:

- an immediate direct call reaches the selected callable root;
- an immediate closure call reaches the selected closure root;
- a structural or dynamically selected call reaches its authored external contract;
- an asynchronous call performs its explicit invocation plan and does not execute the deferred
  body at that point;
- cleanup reaches every ownership-selected drop root;
- unknown destruction reaches one explicit external contract edge.

Each edge retains its checked source locator for diagnostics. The fixed-point solver consumes only
roots, direct facts, and typed edges.

## Product Boundary

The final checked program owns one immutable execution-fact table beside provenance and loans.
Those three relations share the canonical checked-body catalog but remain independent analyses.
Target selection, specialization, MIR, machine lowering, and editor presentation cannot walk bodies
or syntax to reproduce execution facts.

## Future Extension

A later `realtime` contract must be added as a profile over this model, not as an unrelated checker.
It will require additional fact domains such as bounded work, abort behavior, and synchronization,
but must reuse the same execution scopes, edges, primitive authority, destruction integration, and
fixed-point ownership. If a proposed guarantee cannot fit those contracts without a special-case
path, the execution model must be corrected before the syntax is accepted.
