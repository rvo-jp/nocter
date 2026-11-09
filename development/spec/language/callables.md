# Callable Values

## Value Access Callables

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

A tuple input still has one position. Its [pattern](patterns.md#header-origins-and-correspondence)
binds names to component paths without flattening arity. Callable-type pattern names identify
those paths, not another implementation or discard action. Concrete body-entry bindings and
their cleanup follow the selected declaration, with ordinary effect/initialization evidence.

A [variadic input](variadic-inputs.md) is likewise one formal input with entry/component paths,
not a callable whose type changes with actual arity. Read/edit/take applies to each contributed
component. A pack binding is an argument sequence, not a new ordinary array/capability type;
its indexing, forwarding and component cleanup follow the owning pack protocol. Distinct concrete
callable witnesses are not erased or joined merely because they occupy that sequence.

Each invocation has fresh symbolic input extents. A callback permitted to run with arbitrary
short-lived read inputs cannot store such an input for a later call unless its public environment
post-state contract permits it and the caller establishes a sufficiently long-lived source.
Returning `void` does not erase this obligation. The environment's explicit post-state clause follows `mut`, as in
`mut from input (input: Buffer): void`; the result's separate clause stays after its type.
The state relation includes writes to captured outside targets. Their new dependencies persist
after the closure is destroyed, even when the environment no longer retains an input descriptor.
A bodyless contract cannot infer either from a hidden implementation. The
[origin rules](memory-and-regions.md#edited-state-origins) define defaults, completion bounds
and the separate deferred cancellation envelope.
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

[Inferred effects](execution-effects.md) remain conditional on static witnesses and cleanup,
without authored effect parameters. An immediate invocation that returns a future and the
future's later drive/cancellation are separate phases. Erasure or an invocation adapter must
preserve both phase facts and the output's dependency/write schema; a result type alone is
not permission to infer an async declaration or non-waiting constructor.

Ordinary captures are inferred from checked body operations. To deliberately capture a value,
use the name-only segment before `;`:

```text
let reader = (file; path: Path) {
    file.read(path)
}
```

A listed ordinary data name constructs an environment value under the same Copy-or-move rule as
`let next = file`. Repeated names are invalid. Unlisted captures use inferred read/update/value
operations; no escape use silently upgrades a read capture into a move. A value-capture list
cannot copy an exclusive capability. Capture ownership alone does not choose `once`: the body can
read an owned capture repeatedly. Inferred update captures suspend the parent's conflicting access.

A listed pack name instead captures its count and component values under the
[variadic capture protocol](variadic-inputs.md#closures-and-qualification), using the same
component Copy-or-move rule and ordinary invocation inference. This does not give a bare pack
name an ordinary data type, source allocator API or whole-pack assignment operation.

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

### Bounded Owned-Capture Producers

The private fragment additionally selects a first owned-capture producer:
`(value; parameters): R { body }`. It requires exactly one explicit capture, named and fully
typed ordinary parameters, and an explicit ordinary data result type. The environment and
inputs/result range over the fragment's closed builtin/record data. Nested closures, inferred
outside captures and callable results inside the closure body are outside this first scope;
omitting an annotation does not select an alternate input operation. The complete closure
grammar above remains the intended source design.
Contextual completion of omitted operations/data remains outside this first scope; write the
input operation explicitly when an enclosing callable constraint needs edit/take.

Formation constructs the captured value under the ordinary Copy-or-move rule. The closure body
has its own local inputs, return target and loop scope. The captured name denotes the environment
value; it does not denote the original outside slot. Invocation access is inferred from checked
operations on that environment: observation requires func, update requires mut, extraction or
explicit destruction requires once. Ownership of the capture alone does not require once. Copy
evidence follows the captured data, independently of invocation kind. A mut view requires writable
access to its stored environment; a once view consumes one supplied instance.

The body is checked even when formation is unreachable. Read/update invocation must preserve an
initialized environment on every normal return. A consuming invocation owns and cleans the
environment's remaining payload; a returned component is staged before cleanup. An uninvoked
closure owns the environment's cleanup obligation. Copying a Copy closure snapshots its environment;
moving a non-Copy closure transfers that same obligation. This scope's outside D/S independence
follows its exact closed environment schemas, not ownership or invocation kind.

This source/flow boundary grants no erased representation, executable specialization, ABI or
native capability for captured code. Those consumers must explicitly close the same environment
schema and checked body before admitting it.

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

### Statically Witnessed Callable Inputs

A structural callable parameter introduces a witness bound by the selected call. The body uses the
parameter's declared invocation and input/result contract; it cannot select a different operation
from a known caller or inspect a concrete callback body. Forwarding retains that witness and any
selected invocation adapter. This is static parameterization, not implicit runtime erasure.

```text
func apply(action: func(i32): i32, value: i32): i32 {
    action(value)
}

func increment(value: i32): i32 { value + 1 }
func run(): i32 { apply(increment, 1) }
```

The call binds `apply`'s `action` witness to `increment`; the invocation inside `apply` targets the
formal witness. Forwarding that formal to another function preserves the binding rather than
searching its signature for a candidate. Recursive forwarding retains the same finite declaration
and formal-input identities, without constructing an expanding series of witness names.

A callable parameter's structural contract does not establish Copy. Extracting a value from a
read callable input therefore fails without separate Copy evidence, even when a current caller
supplies a capture-free named function. A taking callback may move or be dropped; an edit callback
may delegate update access. A consuming invocation requires an owning source or Copy evidence.
Cleanup of an owning formal environment remains an obligation tied to its witness, not a claim
that the environment is trivial or independent. The concrete supplied environment must satisfy
that obligation and retain its carried dependencies.

### Statically Witnessed Callable Results

A structural callable result delivers an owning environment value satisfying its invocation and
explicit input/result contract. The compiler retains the returned code witness; the signature
does not select runtime erasure. A result contract alone does not grant Copy. The exact
established producer schema may separately prove Copy. Value supply must establish Copy or an
eligible owning source; the result signature cannot substitute for that evidence. Receiving and forwarding the value follows
ordinary value supply, and reading an owning result keeps it alive for the selected operation.

In the bounded private fragment, reachable return producers establish one named code witness or
one of the function's own input witnesses, possibly through other such results. Explicit and
block-final returns participate in the same relation. Each call substitutes its selected actual
witnesses into that relation; two calls to the same factory can therefore return distinct code
witnesses. Their identical result signatures cannot conflate those identities.

Different reachable return witnesses are incompatible within one function's unspecialized
contract. A recursive relation needs an established code or input witness and cannot choose one
from its signature. A taking callback may transfer its environment into the returned value;
a read callback cannot deliver an owning result without separate Copy evidence, regardless of
a currently known caller. The relation preserves invocation views and ordinary value supply.

```text
func increment(value: i32): i32 { value + 1 }
func make(): func(i32): i32 { increment }
func forward(): func(i32): i32 { make() }
func run(): i32 { let action = forward()
    action(1)
}
```

A taking relay preserves its formal witness without claiming that the formal has Copy:

```text
func relay(take action: func(i32): i32): func(i32): i32 { action }
func decrement(value: i32): i32 { value - 1 }
func example(): i32 {
    let first = relay(increment)
    let second = relay(decrement)
    first(second(10))
}
```

For a produced input-dependent result, a call must establish each witness used by the result
relation. An interrupted application without that witness retains only its known structural
contract under [known-shape rules](values-and-types.md#known-shape-without-a-produced-value).
A result independent of that input can still retain its known witness on interrupted evaluation.

Selected invocation adapters remain part of the result relation. A result may not recover a
stronger invocation view from its concrete producer. Known structural type errors remain errors
on interrupted paths; termination never creates a returned environment or relaxes a result
contract. Result relations close before complete body/resource acceptance and concrete instance
closure. No native capability follows from an unresolved relation.
