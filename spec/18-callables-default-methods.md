# Callable Values and Interface Default Methods

This chapter defines callable values, closure expressions, and reusable interface-default methods.

## Composition Roles

Nocter keeps capability and reusable stateless behavior separate from stored composition.

- an `interface` declares a capability; a method without a body is required and a method with a
  body is a reusable default
- stored composition syntax is not part of the current language

A default method adds no fields, layout, or implicit conformance. It is available only after the
receiver has an explicit conformance to that interface.

## Interface Methods

An interface may mix required and default methods:

```nct
pub interface Iterator {
    pub type Item
    pub method &+self.next(): Self.Item?

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
pub method self.map<U, F>(transform: F): MapIter<U, Self, F> from self | transform where F: &+func(Self.Item): U {
    return MapIter<U, Self, F> {
        source: move self,
        transform: move transform,
    }
}
```

Method lookup considers inherent methods and members or defaults from interfaces to which the
receiver explicitly conforms, or from the bounds of a generic receiver. Two applicable declarations
with the same name are ambiguous across those categories. Declaration or import order never selects
one. The selected declaration is statically specialized and called directly.

A required method is implemented in the body of `conform Interface for Type { ... }`. A conformance
member with the same name as a default is its explicit override. Inherent methods neither satisfy
required methods nor override defaults. A default method cannot itself establish conformance.

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

Closure parameters and explicit captures are comma-delimited segments and accept one trailing comma
before their `)` or `;` terminator on any layout. The formatter removes a single-line trailing
comma.

Parameter and result types are inferred from an expected callable contract when that contract is
unambiguous. An annotation may state a parameter or result type when inference needs it:

```nct
(value: i32): bool { value > 0 }
```

The body is an ordinary block. Its tail expression is the result. `return` exits the closure body.

### Closure Control-Flow Boundary

Every closure body is a separate callable control-flow boundary:

- `return`, `return value`, and `return none` return from the closure, never from the surrounding
  function, method, or closure.
- Postfix `?` propagates failure or absence through the closure's own inferred or expected result
  type. It cannot propagate directly through an enclosing callable.
- `break` and `continue` may target only a loop lexically inside the same closure body. A loop
  surrounding the closure expression is not a target.
- Early exit drops live closure-body locals and statement temporaries under the ordinary cleanup
  rules. It does not exit or clean up an enclosing caller scope.
- Callable contracts carry no nonlocal-return, nonlocal-loop-exit, or hidden propagation effect.

```nct
loop {
    let callback = () {
        break // error: no loop in this closure body
    }
}
```

When a closure is passed to a generic callable, contextual checking may infer unknown callable
parameters from the closure result and propagate that substitution to the outer call. The call
still follows the uniform rule that callable type arguments are never written explicitly. See
[Callable Type-Argument Inference](08-generics-interfaces-embedding-methods.md#callable-type-argument-inference).

## Explicit Captures

Captures appear before a semicolon in the parameter list:

```nct
(&threshold; value) { value > threshold }
(&+count; value) {
    count += 1
    value
}
(move prefix; value) { prefix.len() + value }
```

- `&name` stores a readonly borrow
- `&+name` stores a readwrite borrow and therefore requires a writable source place
- `move name` transfers the value into the closure environment

Every reference to an outer local binding must name an explicit capture. Captures initialize once
from left to right. The closure owns moved captures and drops them in reverse field order. Borrowed
captures retain their source loans for the closure value's last use. A closure carrying region-
derived storage cannot escape that region.

The anonymous closure environment follows ordinary structural copyability:

- a capture-free closure is copyable
- a readonly `&name` capture stores a copyable `&T` and preserves copyability
- a readwrite `&+name` capture stores a non-copyable `&+T` and makes the closure move-only
- an owned capture contributes its captured value type; a move-only owned capture makes the closure
  move-only
- the complete closure is copyable exactly when every stored capture is copyable
- invocation capability does not affect this result

```nct
let threshold = 10
let predicate = (&threshold; value: i32) {
    value >= threshold
}

let copied = predicate
inspect(predicate) // valid: predicate remains initialized
```

A readwrite capture is the boundary case:

```nct
var total = 0
let accumulate = (&+total; value: i32) {
    total += value
}

let copied = accumulate // error: the closure contains &+i32
let owned = move accumulate
```

An owned move-only capture likewise makes the complete closure move-only:

```nct
let prefix = String.copy("item: ")
let format = (move prefix; value: i32) {
    "${prefix}${value}"
}

let copied = format // error: the closure owns String
let owned = move format
```

## Callable Capability

Closure values have anonymous concrete types. Built-in structural callable contracts let generic
code state how it may invoke such a value:

```nct
func inspect<F>(callback: F, value: i32): bool where F: &func(value: i32): bool {
    return callback(value)
}

func transform<F>(callback: F, value: i32): i32 where F: &+func(value: i32): i32 {
    var callable = move callback
    return callable(value)
}

func finish<F>(callback: F, value: i32): i32 where F: func(value: i32): i32 {
    return callback(value)
}
```

- `&func(Input): Output` permits repeated invocation through readonly access
- `&+func(Input): Output` permits repeated invocation through readwrite access; the called place
  must be writable
- `func(Input): Output` permits one consuming invocation; the called value is moved by the call

These capabilities describe invocation access, not value copyability. A closure may satisfy a
readonly repeated-call contract while remaining move-only because its environment owns a
move-only value. Conversely, copying a capture-free closure does not grant a consuming invocation
contract that its body does not satisfy.

Parameter names are optional. A single eligible named parameter is inferred as the result origin,
for example `&func(text: &str): &str`. When a result may retain one of several parameters, their
names are required by an explicit clause such as
`&func(left: &str, right: &str): &str from left | right`. A generic parameter may have interface
bounds and one callable contract, but multiple callable contracts are ambiguous and rejected.

Fresh result storage and execution allocation are inferred behind callable boundaries. They do not
change callable capability or structural callable compatibility. A callable `from` clause remains
part of the structural contract because it names caller-managed origins retained by the result.

The invocation surface is identical for all three capabilities: `callback(arguments)`. There are
no user-visible `call`, `call_mut`, or `call_once` methods. Closure calls are statically specialized
to their generated target.

Callable contracts currently appear as generic bounds. They do not define a sized stored type or
an erased parameter ABI. The language does not define an erased callable object,
heap-boxed closure, code-pointer ABI, vtable, or runtime interface dispatch.

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

The iterator chain includes `map`, `filter`, `take`, `skip`, `chain`, `enumerate`, `count`, `last`,
`fold`, `find`, `any`, `all`, and `to_vec`. Constructing an adapter is lazy and allocation-free.
Advancing an adapter may return storage carried by its source or callback.
`Iterator.next` has only its receiver as an eligible origin, so the source clause is elided.
Adapter construction contracts name every retained input when several are eligible; for example,
`map` returns `from source | transform`. Compiler summaries preserve fresh storage through callback
calls without adding variance to `&+func(T): U`. Scalar-only operations such as `count`, `any`, and
`all` discard result-storage provenance. `to_vec` allocates in the current context and retains
element provenance from its source internally.

`map` preserves exact size when the mapped source is exact. `filter` does not, because its
predicate determines how many elements remain. Callback evaluation occurs once per visited item
in source order.

## Unsupported Features

The current language does not include interface inheritance, erased callable types, dynamic
dispatch, implicit capture, asynchronous closures, generators, parallel iterators, comparator
sorting, extension declarations, or implicit conformance.
