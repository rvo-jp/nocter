# Callable Values and Interface Default Methods

This chapter defines the callable and reusable-method surface implemented by the completed v0.3.0
Phase 10 milestone. It does not alter the released v0.2.0 language boundary.

## Composition Roles

Nocter keeps capability, reusable behavior, and stored composition separate.

- an `interface` declares a capability; a method without a body is required and a method with a
  body is a reusable default
- embedding owns a stored component and therefore participates in layout, ownership, provenance,
  and cleanup

A default method adds no fields, layout, or implicit conformance. It is available only after the
receiver has an explicit conformance to that interface.

## Interface Methods

An interface may mix required and default methods:

```nct
pub interface Iterator<T> {
    pub method &+self.next(): T?

    pub method self.count(): usize {
        var source = move self
        var total: usize = 0
        loop {
            source.next() otherwise { return total }
            total += 1
        }
    }
}
```

Only methods without bodies are conformance requirements. A default body is checked once in the
interface generic scope, with `Self` constrained by that exact interface declaration. It may use
the interface's required methods, other unambiguous default methods, and ordinary visible APIs.

Methods may declare generic parameters after the method name:

```nct
pub method self.map<U, F: CallMut<T, U>>(transform: F): MapIter<T, U, Self, F> {
    return MapIter<T, U, Self, F> {
        source: move self,
        transform: move transform,
    }
}
```

Method lookup first considers an applicable inherent method. Otherwise it considers default
methods from interfaces to which the receiver explicitly conforms, or from the bounds of a generic
receiver. Two applicable defaults with the same name are ambiguous. Declaration or import order
never selects one. The selected default declaration is statically specialized and called directly.

An inherent method with the same compatible signature is an explicit override. It also satisfies a
required method of the same interface. A default method cannot itself establish conformance.

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

Parameter and result types are inferred from an expected callable contract when that contract is
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
statically specialized; Phase 10 does not define an erased callable object, heap-boxed closure,
code-pointer ABI, vtable, or runtime interface dispatch.

A closure that consumes captured state may be called only through a consuming capability. Iterator
adapters require a mutable repeated callback, so consuming a capture from their callback body is a
compile error.

## Iterator Chains

Iterator default methods support chains such as:

```nct
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

`map` preserves exact size when the mapped source is exact. `filter` does not, because its
predicate determines how many elements remain. Callback evaluation occurs once per visited item
in source order.

## Deferred Features

Phase 10 does not add interface inheritance, associated types, erased callable types, dynamic
dispatch, implicit capture, asynchronous closures, generators, parallel iterators, comparator
sorting, extension declarations, or implicit conformance.
