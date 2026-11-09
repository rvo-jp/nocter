# Equality, Indexing, and Core Operators

## Core Primitive Equality

Initial primitive `==`/`!=` compare matching `bool`, `i32`, or `void` payloads and construct a
Boolean value. Reading an operand does not transfer its owner. Unit equality is true and inequality
false after evaluating both operands in source order. Do not remove their effects/access validation.
Opaque `error` has no primitive equality. User-defined, compound and interface equality use their
own full-language contracts; this bounded foundation does not derive structural equality for them
or change the published domain.

## Scalar Enum Equality

A scalar enum whose original complete declaration selects the payloadless policy supports
intrinsic ==/!=: matching
nominal types compare their variant identities, without consuming either operand. Aliases retain
that identity. There is no implicit numeric conversion or declaration-order comparison, and no
comparison between distinct nominal enums with equal tags/names. A `copy enum` payload family
does not thereby gain structural equality; an opaque enum's Copy grant exposes no hidden scalar
schema or equality operation. Those types need their separately contracted comparison operation.
Seeing a private completion of an opaque family does not select another intrinsic equality policy.
This selected extension is not implemented by the bounded primitive-equality fragment above.

## Index Projections

An index operator exposes an existing element place, rather than returning an ordinary value.
Its declaration names the element data type; the receiver form selects read or edit projection:

```text
impl Items<T> {
    operator (self[index: usize]): T from self
    operator (edit self[index: usize]): T from self
}
```

These are two fixed index-operation identities, not permission-ranked overloads. Context selects
the required read/edit projection before checking authority and then selects that one declared
operation. A missing edit authority/operator is an error; it does not fall back to another body.
A read use requests the read projection; an edit alias/input, replacement or compound target
requests the edit projection. A value-producing use requests a read projection followed by the
ordinary value supply check. The projected place itself carries access authority, not
ownership of the container element; non-Copy extraction needs a checked exchange/removal API.
There is no automatic copy, hole in the container or silent conversion to an owning element.

```text
let snapshot = numbers[i]   // Copy numeric element into a value
read observed = items[i]   // retain read access to the existing element
edit working = items[i]    // retain update access to the existing element
items[i] = replacement     // RHS first, then edit projection and replacement
```

Each operator body supplies an initialized place with `return place` (or its final place result),
checked against the projection kind and origin bound. This is the defined result protocol of an
index operator, not an alternative result permission on `func` or `method`. A computed fresh value
is not a projected place. Nested ordinary value construction still follows value-supply.

The index operator evaluates receiver/index inputs once in their selected order. It identifies
and stages the place/access, runs normal operator-local cleanup, and hands off the checked
projection. It does not suspend or later resume, and introduces no `yield` or implicit coroutine.
The place and any guard/structure needed for its validity must survive that cleanup. Reject a
projection into a dead local, consumed index input or revoked local guard; do not postpone cleanup
to make it valid. Child access remains rooted in the surviving receiver/input, with conflicts and
last use handled by the same access graph as `read`/`edit`.

A projected element may itself contain dependencies. Copying its descriptor retains them; an
exclusive capability cannot become Copy through indexing. Root and range identities must be
precise enough for disjointness, and every potentially invalidating resize/relocation conflicts
while needed access remains. Unknown invalidation is conservative, not hidden address repair.

Primitive tuple/field/fixed-array selections retain their owning/subplace rules and are not
invented operator calls. [Tuples and fixed arrays](tuples-and-arrays.md#primitive-component-selection)
own those structural selections and array extraction/partial initialization. An owning fixed-array
element can transfer under those rules; this does not turn a user index projection into an owner.
General library index schemas/ABI and native integration remain unqualified.

## Current-Item Projections

The joint zero-input form `cursor[]` selects an existing current-item place. Its declarations
are `operator (self[]): T from self` and `operator (edit self[]): T from self`, separate read/
edit identities. They use the same initialized-place return, authority, origins, local cleanup,
last-demand and invalidation rules as index projections above, without an index input.
Computed fresh values still cannot satisfy a place result, and update cannot fall back to read.

This is not an ordinary method returning an alias, dereference prefix, implicit iterator call or
unit-index shorthand. `cursor[()]` remains a one-input index operation. An implementation needs
a checked valid place on normal completion; a protocol name/advance result does not authorize
unchecked projection or remove required effects. [Traversal](iteration.md#current-item-places)
uses the operation without introducing yield or coroutine bodies. Other capability values may
explicitly declare it under ordinary type-owned behavior. Native/root/schema qualification is
still required before acceptance in an executable product.

For both projection families, T is the referent data type. An optional/fallible T refers to a
carrier in storage; it does not give the operator a value-producing failure envelope. The body
cannot throw merely because the projected datum contains `!`. Project/handle that existing
carrier under its ordinary access protocol, or use an ordinary value-returning API for failure.
