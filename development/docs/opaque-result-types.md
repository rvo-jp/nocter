# Static Opaque Result Types

This document owns the compiler architecture for v0.11.0 Phase 8 `some Interface` results. Public
syntax and semantics belong in the language specification after the phase implementation is
qualified.

## Three Separate Facts

An opaque result has three facts that must not collapse into one representation:

1. the authored contract contains an interface identity and named associated-type bindings
2. the public semantic identity belongs to the declaring callable and its generic substitution
3. the lowering witness is the one concrete type returned by every reachable result path

Ordinary type checking sees the first two facts. Layout, ABI, destruction, and native lowering may
request the third through a dedicated lowering view. Returning the witness directly from general
type conversion would leak its inherent methods and fields into user programs.

## Identity

`OpaqueResultIdentity` is based on the semantic callable declaration span, not its spelling, source
path, import alias, re-export, or selected body file. A generic call extends the identity with the
same canonical substitution used for callable specialization. Two declarations never produce the
same opaque type accidentally, even when both witnesses normalize to the same concrete type.

## Witness Elaboration

Witness inference runs after names and callable signatures resolve but before consumers require
concrete layout. It evaluates the types of explicit value returns and a callable body result in the
defining generic environment. Outcome syntax contributes its success payload. All reachable
candidate types must normalize to one concrete type.

Elaboration records a fact instead of rewriting the authored result. Recursive inference cycles,
no-value bodies, unresolved candidates, and different normalized candidates are diagnostics. The
witness then proves the advertised conformance and each associated binding through the existing
conformance and projection-normalization services.

## Public and Lowering Views

The public view supports:

- equality by opaque identity
- advertised interface method lookup and default-method specialization
- associated projection lookup from named bindings
- conservative move-only ownership
- source presentation as the authored `some Interface<...>` contract

The lowering view supports:

- concrete size, alignment, scalar/aggregate ABI classification, and return transport
- concrete destruction and nested field cleanup
- monomorphized call and method targets
- storage-capability and provenance traversal

Consumers must choose a view explicitly. Name resolution, completion, and ordinary member lookup
cannot call the lowering view. ABI, buildability, and IR code must not reconstruct a witness by
examining a return statement.

## Outcome and Provenance Composition

`some Interface?` and `some Interface!` use the existing optional and fallible outer layers. The
opaque witness describes the success payload only. `from` is neither inferred from nor implied by
the interface contract; it continues to describe storage retained by the returned value. Region
and provenance analysis may inspect the witness shape internally, but diagnostics and editor text
retain the authored contract.

## Editor Boundary

The `some` token is a keyword occurrence, the interface name targets the interface declaration,
and every associated binding name targets its associated declaration. Hover and signature
presentation use the visible interface spelling and normalized public binding types. The witness is
not shown in hover details, completion, inlay hints, or diagnostics outside the defining body.

## Verification

Focused tests cover parsing, recovery, formatting, AST JSON, contextual-keyword behavior, identity,
generic substitution, witness agreement, conformance, associated bindings, ownership, provenance,
outcomes, ABI, cleanup, imports, same-module bodies, method chains, and all editor features. Native
tests use observable destructors to prove that opaque transport adds neither boxing nor duplicate
cleanup. Distributed tests exercise the public standard-library facade from an installed home.
