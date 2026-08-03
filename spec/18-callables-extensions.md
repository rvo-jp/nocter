# Callable Values and Extensions

This chapter defines the adopted v0.3.0 Phase 10 closure and extension surface. It does not alter
the released v0.2.0 language boundary.

## Composition Roles

Nocter keeps capability, reusable behavior, and stored composition separate.

- an `interface` declares a contract and contains no implementation body or state
- an `extension` adds behavior derived from an interface contract without changing conformance,
  identity, or layout
- embedding owns a stored component and therefore participates in layout, ownership, provenance,
  and cleanup

An extension method cannot satisfy an interface requirement. Explicit conformance continues to
require the target's public inherent method with the required signature.

## Extension Declarations

An extension targets a generic parameter constrained by at least one interface:

```nct
extension<T, I: Iterator<T>> I {
    pub method self.count(): usize {
        var source = move self
        var total: usize = 0
        loop {
            let item = source.next() otherwise { return total }
            total += 1
        }
    }
}
```

Phase 10 does not permit unconstrained extensions or extensions that target a foreign nominal type
directly. Extension members are methods with bodies. They cannot declare fields, drop behavior,
associated functions, or interface conformance.

Methods may declare generic parameters after the method name:

```nct
pub method self.map<U, F: CallMut<T, U>>(transform: F): MapIter<T, U, Self, F>
```

An imported extension is considered only after inherent and interface-bound method lookup does not
resolve the call. Every extension constraint must hold for the concrete receiver. Two applicable
extension methods with the same call name are ambiguous; module or import order does not select
one. The selected declaration is statically specialized and called directly.

## Closure Expressions

The canonical closure expression is:

```nct
(value) { value * 2 }
```

Multiple or zero parameters use the same form:

```nct
() { 1 }
(left, right) { left + right }
```

Parameter and result types are inferred from the expected callable contract when that contract is
unambiguous. An annotation may state a parameter or result type when inference needs it:

```nct
(value: i32): bool { value > 0 }
```

The body is an ordinary block. Its tail expression is the result. `return` exits the closure body.

## Explicit Captures

Captures appear before a semicolon in the parameter list:

```nct
(&threshold; value) { value > threshold }
(&+count; value) { count += 1; value }
(move prefix; value) { prefix.len() + value }
```

- `&name` stores a readonly borrow
- `&+name` stores a readwrite borrow and therefore requires a writable source place
- `move name` transfers the value into the closure environment

Every reference to an outer local binding must name an explicit capture. Captures initialize once
from left to right. The closure owns moved captures and drops them in reverse field order. Borrowed
captures retain their source loans for the closure value's last use. A closure carrying region-
derived storage cannot escape that region.

## Callable Capability

Closure values have anonymous concrete types. They participate in generic code through trusted
standard callable interfaces with readonly, mutable repeated, or consuming receivers. Calls are
statically specialized; Phase 10 does not define an erased callable object, heap-boxed closure, or
runtime interface dispatch.

A closure that consumes captured state may be called only through a consuming capability. Iterator
adapters require a mutable repeated callback, so consuming a capture from their callback body is a
compile error.

## Iterator Chains

Standard iterator extensions support chains such as:

```nct
use std/iter/extensions

let output = values
    .into_iter()
    .map((value) { value * 2 })
    .filter((value) { value >= 10 })
    .take(8)
    .to_vec()
```

The Phase 10 chain includes `map`, `filter`, `take`, `skip`, `chain`, `enumerate`, `count`, `last`,
`fold`, `find`, `any`, `all`, and `to_vec`. Adapters are lazy and allocation-free. `to_vec` is an
explicit consuming allocation in the current allocation context.

`map` preserves exact-size iteration when its source is exact. `filter` does not, because its
predicate determines how many elements remain. Callback evaluation occurs once per visited item in
source order.

## Deferred Features

Phase 10 does not add erased callable types, dynamic dispatch, implicit capture, asynchronous
closures, generators, parallel iteration, comparator sorting, unconstrained nominal extensions,
extension properties, or extension-provided conformance.
