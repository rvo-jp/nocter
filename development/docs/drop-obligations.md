# Drop Obligation Design

This document defines the compiler-internal ownership cleanup model. Public
move, initialization, and drop rules remain in
[Values and Types](../../spec/02-values-types.md),
[Ownership, Borrowing, and Drop](../../spec/05-ownership-borrowing-drop.md), and
[ABI and Layout](../../spec/09-abi-layout.md).

## Three Separate Responsibilities

Nocter keeps three concepts separate:

1. The typechecker place-state forest answers whether a source place is
   initialized, moved, dropped, partially initialized, or maybe initialized at
   a control-flow join. It is keyed by source-level root bindings and field
   paths.
2. `AggregateDrop` describes a type's immutable cleanup shape: direct drop
   glue, struct fields, fixed-array elements, or active enum payload fields.
3. `DropObligation` describes which part of one runtime storage location is
   currently initialized and must be cleaned up.

A type's drop shape never records whether a particular value is live. A source
place state never encodes ABI offsets. A runtime obligation never decides
whether source code is allowed to move a place.

## Runtime Obligation States

The lowering model currently uses:

- `Inactive`: the storage owns no initialized value.
- `Complete`: the complete value described by `AggregateDrop` is live.
- `ArrayPrefix { initialized }`: elements in `0..initialized` are live. Cleanup
  tests the runtime count and drops matching elements in reverse index order.

The initialized count advances only after one element has been written
completely. A fallible initializer therefore observes the count from before
its destination element became live.

Obligations may belong to named local slots or hidden temporary slots. Hidden
replacement and return slots participate in propagation cleanup while they are
being constructed, then transfer their bytes to the published destination and
release their temporary obligation.

## Construction Invariants

- Reserve obligation-state ABI locals before error payload and expression
  temporaries so their local word ranges cannot overlap.
- Register a temporary obligation before lowering any initializer that can
  exit the current function.
- Publish initialization progress only after the corresponding bytes form a
  complete value.
- On replacement failure, clean the partial replacement first and the
  still-live old destination second. On replacement success, drop the old
  destination before publishing the replacement.
- On return failure, clean the tracked return temporary. Copy it to the return
  ABI location only after construction succeeds.
- Cleanup order is the reverse of ownership acquisition order.

## Required Extensions

The existing states deliberately do not pretend to solve non-prefix ownership.
Future promotions extend the same obligation model:

- Call evaluation needs an ownership scope that retains completed owned
  argument temporaries until the call begins. If a later argument exits, the
  scope drops earlier arguments in reverse evaluation order.
- Struct construction needs per-field obligations. A field becomes live only
  after its initializer completes; an exit drops live fields in reverse
  initialization order. A struct with direct user drop glue cannot expose a
  partially initialized value to that glue.
- Field extraction needs path masks over the drop shape and must reject partial
  moves through any ancestor whose direct destructor requires the whole value.
- Indexed array extraction needs a sparse live set rather than an initialized
  prefix.
- Move-only `Vec<T>` needs its initialized length to drive recursive element
  drop glue before releasing its raw buffer.

Buildability must keep rejecting a source form until its entire ownership
scope follows these invariants. Adding an expression-shape exception without
the matching obligation lifetime is not a valid promotion.
