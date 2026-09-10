# Callable Values and Interface Default Methods

This chapter defines callable values, closure expressions, and reusable interface-default methods.

## Composition Roles

Nocter keeps capability and reusable stateless behavior separate from stored composition.

- an `interface` declares a capability; an ordinary method is required and a method explicitly
  marked `default` supplies reusable behavior
- stored composition syntax is not part of the current language

A default method adds no fields, layout, or implicit implementation. It is available only after the
receiver explicitly implements that interface.

## Interface Methods

An interface may mix required and default methods:

```nct
pub interface Source {
    pub type Item
    pub method &+self.next(): Self.Item?

    pub default method self.count(): usize {
        var source = move self
        var total: usize = 0
        loop {
            source.next() otherwise { return total }
            total += 1
        }
    }
}
```

An interface may require another interface before declaring its own members:

```nct
pub interface SizedSource where Self impl Source {
    pub method &self.remaining_len(): usize
}
```

A bound `I impl SizedSource` therefore permits members inherited from `Source` without repeating
`I impl Source`. Concrete implementations remain explicit and must satisfy every prerequisite.
The complete prerequisite rules belong to
[Generics, Interfaces, and Methods](generics-and-interfaces.md).

Only methods without the `default` modifier are interface implementation requirements. A default body is checked once in the
interface generic scope, with `Self` constrained by that exact interface declaration. It may use
the interface's required methods, other unambiguous default methods, and ordinary visible APIs.

`noalloc` is an independent callable guarantee. A required interface method may publish it, and a
default method may publish it only when its complete body satisfies the transitive allocation proof:

```nct
pub interface Cursor {
    pub noalloc method &self.position(): usize
    pub noalloc default method &self.is_start(): bool {
        return self.position() == 0
    }
}
```

The inherent method selected for a `noalloc` requirement carries the same guarantee. An unqualified
interface method is conservatively allocation-capable at abstract call sites even when one current
implementation happens not to allocate.

Methods may declare generic parameters after the method name:

```nct
pub noalloc default method self.map<U>(transform: &+func(Self.Item): U): some Source { .Item = U }
from self | transform {
    return TransformSource.new(move self, move transform)
}
```

Method lookup considers inherent methods and members or defaults from interfaces which the receiver
explicitly implements, or from the requirements of a generic receiver. Two applicable declarations
with the same name are ambiguous across those categories. Declaration or import order never selects
one. The selected declaration is statically specialized and called directly.

A required method is implemented by one exact inherent method selected by an explicit
`impl Interface` member. A matching inherent method overrides a same-name default. The `impl`
member, not the method's presence alone, establishes the interface implementation.

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

The complete capture, parameter, result, and grouping boundary is defined under
[Closure Expressions](syntactic-grammar.md#closure-expressions).

Closure parameters and explicit captures are comma-delimited segments and accept one trailing comma
before their `)` or `;` terminator on any authored layout. In canonical output, the semicolon is
the capture-segment terminator and replaces a trailing capture comma. The formatter removes a
single-line parameter trailing comma and retains a multi-line parameter trailing comma:

```nct
let callback = (
    &source,
    move prefix;
    value,
    index,
): bool {
    ...
}
```

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
[Callable Type-Argument Inference](generics-and-interfaces.md#callable-type-argument-inference).

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

Every reference to an outer local or parameter binding must name an explicit capture. A capture
names exactly one binding in an enclosing callable body; module declarations, fields, projections,
and arbitrary expressions are not capture targets. Capture names are unique, and a capture name
cannot collide with a closure parameter name.

Captures initialize once from left to right. `&name` and `&+name` perform the ordinary borrow
operation at the capture position, including writability and exclusivity checking. `move name`
performs the ordinary ownership transfer from that binding. A failure to satisfy any capture does
not create a partially accepted closure expression.

The environment stores `&T`, `&+T`, or owned `T` according to the authored capture. Inside the
closure body, however, the captured name denotes the projected captured place:

- `&name` exposes a readonly place of type `T`
- `&+name` exposes a readwrite place of type `T`
- `move name` exposes the owned environment place of type `T`

This projection is part of closure capture binding; it is not a general implicit dereference for
borrow values. The readonly form cannot be assigned, the readwrite form follows ordinary mutation
and exclusivity rules, and neither borrowed form permits moving the referenced owner.

The closure owns moved captures and drops their still-initialized values in reverse capture order.
Borrowed captures retain their source loans through the last use of every live closure value
derived by copying or moving the environment. Copying a readonly-capture closure therefore extends
the same source loan through all copies; it does not create independent source storage. A closure
carrying region-derived storage cannot escape that region.

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

## Callable Types

Closure values have anonymous concrete types. Built-in structural callable types let source state
how it may invoke such a value without treating invocation as nominal interface implementation:

```nct
func inspect(callback: &func(value: i32): bool, value: i32): bool {
    return callback(value)
}

func transform(callback: &+func(value: i32): i32, value: i32): i32 {
    return callback(value)
}

func finish(callback: func(value: i32): i32, value: i32): i32 {
    return callback(value)
}
```

- `&func(Input): Output` permits repeated invocation through readonly access
- `&+func(Input): Output` permits repeated invocation through readwrite access; the called place
  must be writable
- `func(Input): Output` permits one consuming invocation; the called value is moved by the call

These forms describe invocation access, not value copyability. A closure may satisfy a
readonly repeated-call contract while remaining move-only because its environment owns a
move-only value. Conversely, copying a capture-free closure does not grant a consuming invocation
contract that its body does not satisfy.

Callable annotations are statically witnessed. A local initializer or call argument selects one
exact concrete closure or callable witness, and that identity remains part of the enclosing
callable specialization. The annotation does not erase the environment, allocate a box, create a
code-pointer pair, or introduce indirect dispatch. Different concrete witnesses cannot flow into
one local binding or one control-flow merge. A future erased callable requires a separate explicit
type and ABI.

Callable annotations are accepted for callable parameters and initialized local bindings. They are
not sized data-bearing types and cannot appear as nominal fields, variant payloads, type aliases,
generic arguments, or named non-opaque results. An opaque result may retain a hidden callable
witness as part of its compiler-selected concrete representation.

Parameter names are optional. A single eligible named parameter is inferred as the result origin,
for example `&func(text: &str): &str`. When a result may retain one of several parameters, their
names are required by an explicit clause such as
`&func(left: &str, right: &str): &str from left | right`. `impl` never precedes a callable type;
that keyword is reserved for nominal interface implementation.

Fresh result storage and execution allocation are inferred behind callable boundaries. They do not
change callable capability or structural callable compatibility. A callable `from` clause remains
part of the structural contract because it names caller-managed origins retained by the result.

The `noalloc` guarantee, when present, is also part of the structural callable contract. It appears
before the capability as `noalloc func`, `noalloc &func`, or `noalloc &+func`. A proven
allocation-free closure may satisfy either the guaranteed contract or an otherwise identical
unqualified contract. Erasing the guarantee is one-way; an unqualified callable value cannot be
used where `noalloc` is required merely because its hidden witness once had that property.

Synchronous waiting is a separate callable effect. A structural callable that may synchronously
wait writes `blocking` after any `noalloc` guarantee and before its capability:

```nct
blocking func(): void
noalloc blocking &func(Request): Response!
blocking &+func(&+[u8]): usize!
```

An otherwise identical nonblocking callable may be used where a `blocking` callable is accepted;
the reverse conversion is invalid. A bodyless callable, primitive, or interface requirement has no
implementation proof and therefore exposes blocking behavior only through the explicit modifier.
A source-backed private helper or closure may have its blocking effect inferred from its complete
body, but that proof cannot be recovered after conversion to a `blocking` callable contract.
`blocking` is incompatible with `async`: every value of type `future T` must remain safe to drive
without synchronously waiting for external progress.

The invocation surface is identical for all three capabilities: `callback(arguments)`. There are
no user-visible `call`, `call_mut`, or `call_once` methods. Closure calls are statically specialized
to their generated target.

Callable annotations do not define a uniform stored layout or an erased parameter ABI. Hidden
witness specialization preserves the concrete environment layout. The language does not define an
erased callable object, heap-boxed closure, code-pointer ABI, vtable, or runtime interface dispatch.

A closure that consumes captured state may be called only through a consuming capability. Iterator
adapters require a mutable repeated callback, so consuming a capture from their callback body is a
compile error.

## Standard Iteration

The standard library uses callable values and interface defaults to provide lazy iterator chains:

```nct
let output = values
    .into_iter()
    .map((value) { value * 2 })
    .filter((value) { value >= 10 })
    .take(8)
    .to_vec()
```

The exact interfaces, adapters, operations, allocation behavior, provenance, and exact-size rules
belong to the compiler-checked [iteration contract](../../development/std/iter/index.nct) and its
[behavior guide](../../development/std/iter/README.md). They are ordinary applications of the
callable and interface rules in this chapter, not additional callable syntax.

## Unsupported Features

The current language does not include implementation inheritance, erased callable types, dynamic
dispatch, implicit capture, asynchronous closures, generators, parallel iterators, comparator
sorting, extension declarations, or implicit interface implementation. Interface prerequisite
contracts are defined by [Generics and Interfaces](generics-and-interfaces.md#interface-prerequisites).
