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
runtime callable value, or introduce indirect dispatch. Different concrete witnesses cannot flow
into one binding or one control-flow merge unless the destination explicitly uses an erased
callable type.

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

The `notrap` guarantee is an independent part of the same structural contract. It appears after
`noalloc`, when both are present, and before `blocking` or the callable capability:

```nct
notrap func validate(value: i32): bool
noalloc notrap &func(&Record): bool
any notrap &+func(Job): void!
```

`notrap` promises that immediate execution cannot reach a Nocter safety trap. The proof includes
explicit trap and unreachable operations, checked integer arithmetic and shifts, bounds-checked
indexing, postfix `!`, argument-spread length checks, selected operators and coercions, callbacks,
source-backed callees, closure bodies, and implicit destruction. A recoverable `T!` result is
allowed because returning `error` is not a trap. Explicit `process.abort` and `process.exit` calls
are terminating process operations rather than safety traps and are also allowed; neither runs
source cleanup.

An unqualified source-backed helper may be used when whole-program checking proves its reachable
body trap-free. A bodyless primitive, abstract method, or callable value must carry `notrap` before
a caller may rely on that guarantee. Primitive declarations are checked against compiler-owned
runtime-role evidence, so a trap primitive cannot make a false `notrap` claim. The proof does not
cover allocation-failure termination, operating-system termination, hardware failure, or violation
of compiler/runtime representation invariants.

Trap freedom weakens in one direction: a `notrap` callable may satisfy an otherwise identical
unqualified contract, but an unqualified callable cannot satisfy a `notrap` requirement. Body
inference supplies proof evidence and never silently adds `notrap` to the callable's source-visible
type. `notrap` is currently an immediate-execution guarantee. Combining it with `async` is invalid
because `future T` does not yet retain a trap-freedom contract for later driving.

Generic operator, borrow-coercion, and expansion requirements carry the same immediate `noalloc`
and `notrap` guarantees when those guarantees matter to the generic body:

```nct
noalloc notrap func same<T>(left: &T, right: &T): bool
where noalloc notrap (&T == &T): bool {
    return left == right
}
```

The structural requirement is the generic body's execution contract for that selected operation.
Concrete specialization verifies the actual operation and any implicit coercions against it; body
checking and execution analysis do not rediscover the implementation from source text. Omitting a
modifier is deliberate weakening, not inferred evidence that the operation may allocate or trap.

Synchronous waiting is a separate callable effect. A structural callable that may synchronously
wait writes `blocking` after any `noalloc` and `notrap` guarantees and before its capability:

```nct
blocking func(): void
noalloc blocking &func(Request): Response!
blocking &+func(&+[u8]): usize!
```

An otherwise identical nonblocking callable may be used where a `blocking` callable is accepted;
the reverse conversion is invalid. Every named callable exposes blocking behavior only through the
explicit modifier, including a source-backed private helper. This keeps its callable-value type and
direct-call behavior identical without making declaration types depend on later body analysis.
Closure bodies are checked against the structural callable contracts required at their use sites.
`blocking` is incompatible with `async`: every value of type `future T` must remain safe to drive
without synchronously waiting for external progress.

The invocation surface is identical for all three capabilities: `callback(arguments)`. There are
no user-visible `call`, `call_mut`, or `call_once` methods. Closure calls are statically specialized
to their generated target.

### Erased Callable Types

Prefixing a complete callable type with `any` creates a distinct sized type with runtime callable
erasure:

```nct
type Handler = any &func(Request): future Response!
type Observer = any noalloc &func(Event): void
type Mutator = any &+func(Event): void
type Finalizer = any func(Job): Result
```

The enclosed callable contract is unchanged. Capability, parameter and result types, result
provenance, `noalloc`, `notrap`, and `blocking` remain part of compatibility and invocation. `any &func`
permits repeated readonly invocation, `any &+func` permits repeated invocation through a writable
place, and `any func` permits one consuming invocation. All use the ordinary
`callback(arguments)` surface.

An erased callable is an owning, move-only value regardless of the hidden environment's structural
copyability. It may be used in fields, variant payloads, aliases, generic arguments, parameters,
locals, named results, and supported outcome layers. Its destruction destroys every still-live
owned capture exactly once and releases the compiler-owned environment storage. Captured borrows
retain their ordinary provenance and cannot escape their source merely because the environment is
erased.

Erasure occurs only at a contextual expected-type boundary that explicitly names `any`, including
an annotated initializer, an erased-callable argument, a named erased result, or an `as any ...`
conversion. The source must provide one concrete callable witness compatible with the complete
destination contract. Erasure may weaken `noalloc`, `notrap`, or nonblocking guarantees in the same
direction as an ordinary callable conversion, but cannot strengthen them. No untyped closure, ordinary
callable annotation, generic inference result, or control-flow merge silently selects erased
representation.

Constructing an erased callable allocates compiler-owned environment storage and moves the concrete
environment into it. This construction is therefore rejected inside a `noalloc` body even when the
finished erased callable's invocation contract is `noalloc`. Calling that finished value performs
only the effects admitted by its enclosed contract; construction allocation is not an invocation
effect. APIs that do not need heterogeneous storage should continue accepting statically witnessed
generic callables so invocation remains directly specialized and construction does not allocate.

The fixed target representation and indirect-call ABI are defined by
[ABI and Layout](../platform/abi-and-layout.md#erased-callable-layout). Erasure is specific to
callables and does not introduce `any Interface`, implicit nominal conformance, or general dynamic
dispatch.

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
belong to the compiler-checked [iteration contract](../../std/iter/index.nct) and its
[behavior guide](../../std/iter/README.md). They are ordinary applications of the
callable and interface rules in this chapter, not additional callable syntax.

## Unsupported Features

The current language does not include implementation inheritance, general interface-object dynamic
dispatch, implicit capture, asynchronous closures, generators, parallel iterators, comparator
sorting, extension declarations, or implicit interface implementation. Interface prerequisite
contracts are defined by [Generics and Interfaces](generics-and-interfaces.md#interface-prerequisites).

## Future Direction: v2.0.0 Value Access Callables

The [Value Access Model](value-access.md) separates the callable binding's operation, environment
invocation, explicit input operations, value result, result origins, and edited-state relation.
These future rules replace the earlier outer `&`/`^` and result-permission callable forms.

| Type | Invocation of the supplied environment |
| --- | --- |
| `func(T): R` | Read, repeatable |
| `mut(T): R` | Update, repeatable with valid state |
| `once(T): R` | Receive a value instance and consume it, at most once per instance |

`mut` is not a purity claim and does not select explicit input operations. `once` does not require
an invocation: an uninvoked value can be destroyed. Copy evidence can supply separate callable
instances; it does not allow consuming the same instance twice. Mutable/update captures prohibit
Copy. An immutable value binding can move to a taking input; ordinary mut invocation instead
requires mutable/edit authority over its environment.

```text
func update(action: func(edit Buffer): void, edit buffer: Buffer): void
func repeat(edit action: mut(i32): void, value: i32): void
func finish(take action: once(take Job): Product, take job: Job): Product
```

Callable input operations remain in the structural type even with names omitted. Every ordinary
result R delivers a value. Annotating, forwarding, implementing an interface or erasing preserves
all input modes, result/state origins, reachable write footprints, capture/dependency schemas
and cleanup obligations. Origin
names bind input positions; names themselves are not type identity. Invocation environment access
is prepared before explicit inputs; validate hidden capture overlaps in the complete bundle.

Each invocation has fresh symbolic input extents. A callback permitted to run with arbitrary
short-lived read inputs cannot store such an input for a later call unless its public environment
post-state contract permits it and the caller establishes a sufficiently long-lived source.
Returning `void` does not erase this obligation. The environment's explicit post-state clause follows `mut`, as in
`mut from input (input: Buffer): void`; the result's separate clause stays after its type.
The state relation includes writes to captured outside targets. Their new dependencies persist
after the closure is destroyed, even when the environment no longer retains an input descriptor.
A bodyless contract cannot infer either from a hidden implementation. The
[origin rules](memory-and-regions.md#edited-state-origins) define defaults and all-exit obligations.
`func`/`once` do not admit a mutating-environment clause.

A result origin can refer to a named input or the callable's implicit `self` environment, using the
existing `from` relation. Multiple eligible sources require an explicit upper bound. Access into
environment storage differs from the outside dependencies already carried by a capture. Consuming
invocation cannot deliver access into a capture that its cleanup destroys. It can transfer an
owned capture or forward an already dependent descriptor when the outside roots remain valid.
At a callable boundary, a checked invocation adapter may supply `func` to `mut`, or `func`/`mut`
to `once`, with exact explicit input operations and compatible data/result contracts. `func` to
`mut` preserves the environment state and must satisfy the receiving post-state bound. Taking
adaptation receives its own environment value under Copy-or-move, invokes it with the required
read/edit access, and completes normal environment cleanup before handing off the answer. Reject
any answer depending on that destroyed environment storage. Forwarded carried dependencies remain.

Check the complete origin/state/effect/cleanup relation before selecting and freezing the adapter.
This is a recorded boundary operation, not type equality, private-body lookup or a second dispatch
attempt. `mut` cannot become `func`; `once` cannot become a repeatable contract, even if a body
appears to use fewer rights. A failed proof is an error, never a search for another target.

Ordinary captures are inferred from checked body operations. To deliberately capture a value,
use the name-only segment before `;`:

```text
let reader = (file; path: Path) {
    file.read(path)
}
```

A listed name constructs an environment value under the same source-level Copy-or-move rule as
`let next = file`. Repeated names are invalid. Unlisted captures use inferred read/update/value
operations; no escape use silently upgrades a read capture into a move. A value-capture list
cannot copy an exclusive capability. Capture ownership alone does not choose `once`: the body can
read an owned capture repeatedly. Inferred update captures suspend the parent's conflicting access.

Explicit capture entries are evaluated left to right at closure formation. Earlier executed
moves are not rolled back after a later formation failure; constructed captures are cleaned in
reverse order. An inferred capture activates at formation, not at its first invocation. All
captures retain the dependencies needed by future invocation/output/destruction. Capturing access
to a local does not permit the closure to escape that local's lifetime.

A fresh closure constructs a value. `let second = callback` copies with evidence or moves a
non-Copy callable; it no longer creates an implicit read alias. Use `read alias = callback` for
that operation. `var` permits a locally stored mutable callback to be invoked/updated. A function
parameter's operation independently determines whether it can read/update/take the callable.

An expected callable type supplies omitted closure input operations, output type and origin/state
bounds before checking the body. Explicit `edit`/`take` operations must agree with that contract;
a parameter data-type annotation alone does not assert an operation. This is contextual completion
of an anonymous implementation, not re-inference of a named declaration's default.

Without an expected callable contract, every unmarked closure input is read, just as in a named
function. The body may infer data types, captures, environment invocation and result shape; it
cannot silently change an unmarked input to edit/take because its operations need those rights.
A stand-alone identity of non-Copy data therefore needs `(take value: File) { value }`. An inline
`(value) { value }` can instead inherit take from a previously selected expected callable contract.
A read identity with source-level Copy evidence can return a snapshot normally. Insufficient data
information requires an ordinary annotation. Later uses cannot retroactively select input/capture
operations or invocation kind. Erased contracts preserve the same facts without run-time name
lookup or authority repair.

### Named Function Witnesses

A named function used as a value selects that declaration, not an overload set or a runtime name.
It has a concrete statically selected witness. A callable annotation constrains its invocation,
ordered input operations/data and result data; it does not erase the witness or change those
inputs from the body. Two distinct witnesses do not become the same data type merely because
their signatures agree. A conditional may retain one common witness; heterogeneous joining needs
the separately specified erased representation.

A named function has no captured environment. Its exact value payload carries the selected code
identity, has Copy evidence, and has no outside storage/access dependencies. These facts concern
that concrete witness, not every value satisfying `func`, `mut` or `once`. Its invocation view
remains independent of Copy: adapting a named witness to `mut` still requires writable access to
invoke that view; adapting it to `once` receives a separate instance under ordinary value supply.
Copy can supply another instance, while a dropped source remains unavailable.

A local annotation can select the permitted invocation adapter while preserving the concrete
witness. Aliases can check an already matching view but cannot construct an adapted environment.
Neither a hidden original declaration nor the absence of captures may upgrade a restricted view
back to a stronger invocation contract. Invoke the selected view and validate the complete
prepared bundle before explicit inputs or the environment are consumed.
