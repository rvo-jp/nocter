# Generics, Interfaces, and Methods

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Generics

Generic declarations use angle brackets:

```nct
struct Buffer<T> {
    value: T
}

func first<T>(items: &[T]): T? {
    if items.len() == 0 {
        return none
    }
    return items[0]
}
```

A generic parameter may carry a finite `+`-separated capability set:

```text
GenericParameters = "<" GenericParameter ("," GenericParameter)* ">"
GenericParameter  = Name [":" Bound ("+" Bound)*]
Bound             = InterfaceBound | CallableContract
InterfaceBound    = Type
CallableContract  = ["&" ["+"]] "func" "(" CallableParameters ")" ":" Type
```

Every nominal bound must resolve to an accessible interface with the declared type arity. Bound
order is formatting information; semantics use specialized interface declaration identities plus
at most one structural callable contract. Duplicate interface identities and multiple callable
contracts are invalid.

```nct
func inspect<T: Readable<i32>>(value: &T): i32 {
    return value.read()
}
```

Generic implementation uses monomorphization. Nocter does not provide runtime generic metadata,
interface objects, `where` clauses, interface inheritance, higher-kinded types, generic associated
types, or general const generics.

## Inherent Implementations

An inherent `impl` associates receiver methods and `drop` with a nominal type. It does not create a
class or introduce inheritance.

```nct
impl WordStats {
    pub method &+self.add_word(): void {
        self.words += 1
    }
}
```

The target must be a nominal `struct` or `enum`; a type alias cannot own an `impl`. Generic impl
parameters are in scope for the target, members, and member bodies.

Functions that directly create the nominal owner belong to its `construct` declaration. Other
associated functions are qualified top-level declarations. Construction behavior is specified in
[Construction Surfaces](19-construction-surfaces.md).

## Receivers

Receiver spelling determines call capability:

```nct
method &self.name(...): Return
method &+self.name(...): Return
method self.name(...): Return
```

- `&self` borrows the receiver readonly.
- `&+self` borrows a writable receiver readwrite.
- `self` consumes the receiver or copies it when its type is `Copy`.
- A newly created owned temporary may be a readwrite receiver for its single method call.
- A borrow derived from a temporary receiver cannot escape the statement.

Methods use `value.method(arguments)`. They are not callable through UFCS-like
`Type.method(&value, arguments)` syntax. Associated functions use `Type.function(arguments)` and
cannot be called as value members.

`self` is the fixed receiver binding. `Self` is type-position syntax denoting the current inherent,
interface, conformance, or construction owner; it is not resolved as an ordinary identifier.

## Interfaces

An interface is a nominal public capability. Its members are explicitly public methods. A member
without a body is required; a member with a body is reusable default behavior derived from the same
interface contract.

```nct
pub interface Counter {
    pub method &+self.next(): i32?

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

An interface cannot declare fields, stored state, associated data, associated types, construction
members, or `drop`. A default method does not establish conformance and cannot access members absent
from its declaring interface contract.

## Explicit Conformance

Conformance is declared with a mandatory body-bearing implementation:

```nct
impl Printable for User {
    method &self.print(): i32 {
        return 0
    }
}
```

The implementation body owns every required member implementation. Members omit `pub` because the
interface declaration owns visibility. A default may be omitted or overridden by a same-name
member. An inherent method never establishes or overrides interface conformance.

Conformance rules are:

- the interface and target resolve to exact nominal identities
- the target is a nominal `struct` or `enum`
- every bodyless interface method has exactly one matching implementation member
- extra methods, associated functions, literals, `drop`, and construction members are invalid
- receiver capability, generic parameters, parameter and result types, outcome layers, and external
  result provenance participate in signature compatibility
- parameter names do not participate in compatibility
- a result provenance implementation may promise a narrower, longer-lived origin set; a concrete
  storage-independent result may omit an interface origin that cannot apply to that result, while
  a storage-carrying result cannot introduce an undeclared origin
- matching members without an explicit conformance declaration do not conform

Generic conformance parameters may carry bounds. A conditional conformance exists for a concrete
target only when every specialized bound is satisfied:

```nct
impl<T, I: Iterator<T>> Iterator<T> for TakeIter<T, I> {
    method &+self.next(): T? from self {
        if self.remaining == 0 {
            return none
        }
        self.remaining -= 1
        return self.source.next()?
    }
}
```

Identical normalized target/interface patterns are rejected rather than ranked. Nocter does not
perform overlap specialization.

## Method Lookup

Concrete receiver lookup collects accessible inherent methods, explicit conformance members, and
applicable interface defaults. The call is valid only when one candidate remains. Declaration and
import order never resolve ambiguity.

For a bounded generic receiver, lookup searches only the parameter's declared capability set. The
call is checked against the specialized interface signature, and every reachable concrete
instantiation must provide explicit conformance. Lowering statically selects the conformance member
or specialized default body; there is no vtable or runtime interface lookup.

If two bounds declare the requested name, lookup is ambiguous even when their rendered signatures
match. The compiler never falls back to an inherent method merely because it shares the missing
bound method's spelling.

## Callable Bounds

Built-in callable contracts may appear in a capability set and are invoked with ordinary call
syntax. Callable ownership and repeated-call capability are specified in [Callable Values and
Interface Default Methods](18-callables-default-methods.md).

## Unsupported Composition Syntax

Embedding syntax such as `...Type` and `pub ...Type` is not part of the current language. Nocter
does not provide inheritance, mixins, trait implementation reuse, automatic delegation, or implicit
interface conformance. A future composition proposal must define ownership, initialization,
visibility, collision, partial-move, and conformance interaction before adoption.
