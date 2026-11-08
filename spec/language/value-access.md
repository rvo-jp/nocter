# Value Access Model

## Future Direction: v2.0.0

This chapter owns the adopted v2.0.0 value-binding foundation. It is not part of the current
released contract; current ownership remains in [Ownership, Borrowing, and Drop](ownership.md).
Ordinary data types describe values. Binding and parameter operations describe how those values
are accessed. Capability values and callable contracts retain the access/dependency information
needed for safety; that information is not removed from the type system.

This direction replaces the earlier future type-side `&T`/`^T`, explicit `&place`/`^place`, and
permission-inferred `let` rules. They are not optional aliases. The
[bounded private-unit fragment](syntactic-grammar.md#bounded-value-binding-fragment) selects a
smaller source scope; it does not define full-language conformance, target realization or a
general soundness proof. Other chapters state the extent of their future
migration; an unchanged released production does not silently become a v2.0.0 production.

### Values and Scoped Aliases

| Declaration | Meaning |
| --- | --- |
| `let value = expression` | Construct a fixed value binding |
| `var value = expression` | Construct a mutable value binding |
| `read alias = place` | Read existing storage through a scoped alias |
| `edit alias = place` | Update existing writable storage through a scoped alias |

`let` and `var` construct values under the same Copy-or-move rule. Both can deliver their values
elsewhere; only `var` permits ordinary updates/replacement. A `let` value cannot be reinitialized
after transfer/drop. A `var` slot can be reinitialized by a complete replacement. Optional type
annotations constrain the data type, never change the binding operation.

```text
var count = 10
let snapshot = count       // independent i32 value
count = 20                 // snapshot is still 10
let file = open(path)?
let next = file            // move-only value; file is now unavailable
read inspect = next       // next stays responsible for the file
```

`read`/`edit` evaluate an initialized place and its dynamic projections once. They introduce no
new owner, hidden clone, retargetable reference slot or runtime allocation. Their optional type
annotation checks the referent's data type. Assigning through `edit` updates its original place;
it does not retarget the alias. `read` cannot update or extract a non-Copy referent. Neither alias
can be explicitly dropped as an owner. A fresh temporary/call result is not a place for these
binding forms: receive its value with `let`/`var` instead. A returned capability descriptor is a
value and follows that rule too.

Creating `edit` requires initialized writable storage or existing edit authority. Creating
`read` requires initialized readable storage. A parent update may delegate read or update access;
a parent read cannot delegate update. Aliases cannot be stored or returned as plain data T.
Persistent/returned access uses a capability value with a checked dependency contract. Indexing can
also expose an existing element place through its separately declared
[projection protocol](equality-and-indexing.md#future-direction-v2-0-0-index-projections); it does
not make an ordinary function return an alias.

### Value Construction and Copy

Every value-producing boundary delivers a value: `let`/`var` initialization, replacement,
aggregate component construction, ordinary block/return results, selected outcome payload
construction and `take` argument supply. Resolve the data type and capabilities, then apply:

1. An already constructed value transfers to the destination, preserving its dependencies.
2. An initialized readable place with source-level Copy evidence supplies a separate snapshot.
3. Otherwise an initialized owning place eligible for extraction transfers its value and becomes
   unavailable. Ordinary read/edit aliases cannot supply this extraction.
4. Otherwise reject the boundary. Do not silently create an alias, invoke a clone, infer Copy from
   later specialization, or choose the operation from whether the source has a later use.

A fixed owner is eligible for transfer, subject to access/initialization checks. Copyable inputs
remain available after supplying values, including `take` arguments. Copy is structural, has no
user hook, allocation or resource acquisition, and does not mean that a large value is cheap.
A permitted Copy can be optimized away only while preserving snapshot semantics.

Nominal structs opt in with `copy struct`; private Copy fields alone do not make an ordinary
nominal Copy. Scalars, unit, payloadless enums and supported raw boundary values permit Copy.
Tuples, fixed arrays and optionals require Copy for every component, independently of active
branch or length. Generic code needs declared evidence. `error`, fallible outcomes and futures
remain move-only. Copy types cannot have user destructors. A closure's Copy evidence depends on
its captures, not its invocation kind. Exclusive update capabilities are non-Copy even when their
element type is Copy.

Copy removes only the need to read the old containing place. Dependencies carried inside the
copied payload and external storage requirements survive. Copying a read-view descriptor does not
copy its referent. For an existing owner, selection of Copy versus move is fixed by the checking
context; monomorphization cannot rewrite an unconstrained generic move into a non-invalidating Copy.

```text
func identity<T>(take value: T): T {
    return value
}
```

The generic body transfers its received value. A caller knowing `T = i32` can supply a snapshot to
`take`; that does not change the body's operation. A generic read input without Copy evidence
cannot be returned as a plain T. A separate capability result can expose checked access instead.

### Input Operations and Receivers

| Input | Contract |
| --- | --- |
| `value: T` | Read caller storage; leave caller ownership intact |
| `edit value: T` | Update caller storage; leave it initialized on outward recoverable exits |
| `take value: T` | Receive a value and its transfer/cleanup responsibility |

Read is the sole default input operation for named functions, methods, constructors, interface
requirements and structural callable types, independently of Copy, representation, naming or
body use. Do not add explicit `read value: T` as an equivalent input spelling. Plain `self` reads
the receiver. Ordinary value-producing boundaries remain Copy-or-move; a colon followed by T
states the data type and does not impose one universal operation on every declaration position.

This default lets an observation retain its argument without constructing another value, including
when the argument is non-Copy. Receiving an independent value remains explicit with `take`.
A constructor that stores an input as a non-Copy component must declare that input `take`; its
name or constructor role cannot silently change the default. A read input cannot be returned as
an independent T without Copy evidence. Report that attempted value construction locally rather
than changing the public input contract from the body. These are semantic operation rules, not a
requirement to pass every read input by machine pointer or a guarantee of better performance.

Read inputs do not acquire snapshot semantics even for Copy T. A `take` input receives its own
locally writable slot; updates to it do not update the caller's original slot. This is the
explicit full-value operation, not a reason to make every owning local mutable. A fixed local can
be delivered to `take`: any recipient's mutation applies to that recipient's slot. A local
snapshot `var copy = input` can similarly be updated without granting authority over `input`.

Methods use the same operations: `method self.len()`, `method edit self.append(...)`, and
`method take self.finish(...)`. No expanded `self: T` receiver alias or symbolic receiver form
is supplied alongside these. Receiver operation and explicit input operations are independent.

```text
impl Buffer {
    func create(capacity: usize): Self!
    method self.len(): usize
    method edit self.append(value: u8): void
    method take self.finish(): Product!
}

var buffer = Buffer.create(64)?
buffer.append(42)
let product = buffer.finish()?
```

Calls uniformly omit input/receiver assertions. Lookup selects one operation before access
checking. Missing authority is an error, not permission to select another body, clone a value,
or add a call-site marker. Keep complete signatures in both public contract and implementation.

Read accesses may overlap reads. Overlapping read/update, update/update and transfer/access may
not be simultaneously active. Proved-disjoint projections may be used independently. Delegation
restricts/suspends a parent while a child needs that authority; it does not duplicate exclusivity.
An update must remain initialized on success, recoverable failure and cancellation. Removing a
value through an update requires an exchange with a ready replacement; an unfilled fallible gap
is invalid. Completed changes need not be rolled back.

### Values Can Carry Dependencies

The compiler distinguishes:

- R: access to a particular existing place, needed by a scoped alias or an operation;
- D: access relationships carried inside a value, including views, records and closures;
- S: external storage/allocator/region validity requirements.

A value and its cleanup responsibility do not imply empty D/S. A result T delivers a value,
including when that value is a descriptor accessing outside storage. A read capability may be
Copy; an exclusive update capability is non-Copy. Their exact standard-library names and members
belong to checked library declarations, not to new built-in `View`/`EditView` type constructors.

Aggregates, optionals, generic substitution and erasure preserve component dependencies. Private
fields cannot hide required dependencies from the public value contract. A destructor's accesses
keep dependencies live through that cleanup, not merely through the last ordinary read. Moving
an inline self-reference does not become valid just because the enclosing type owns its data;
stability needs a checked representation/relocation contract.

An alias can end after its last required use and dependent cleanup. Propagate those requirements
through branches/loops and delegated children. Weaken retained update demand to read demand when
only reads remain; do not change declarations or reselect operations. An unused intermediary can
release its own demand and reparent children to a surviving authority, preserving identities,
permissions, dependencies and pending call reservations. Never extend a root's lifetime to make
reparenting succeed. Ending access does not move observable resource destruction earlier.

### Result Origins and Retained Update State

Ordinary T results deliver values; there is no separate result permission or `yield`/`view`
return syntax. The [origin rules](memory-and-regions.md#future-direction-v2-0-0-value-access-origins)
constrain outside dependencies. A single eligible source may be elided. Ambiguous public origins
need an explicit `from` upper bound, resolved without a private body.

An edited destination can gain dependencies even when the result is `void`. A clause after its
type bounds the outside origins of its complete post-call value:

```text
func remember(edit holder: Holder from buffer, buffer: Buffer): void
```

This allows, but does not promise, retaining access into `buffer` in `holder`. Retaining old
holder-derived dependencies as well needs `from holder | buffer`. It is not an origin requirement
on the incoming holder. It grants no permission beyond the inputs and no ownership
of buffer. The caller validates the resulting dependency state before releasing any roots. The
same mechanism applies to callable environments; the callable contract must retain their
post-state relation. No result is not proof of no retained dependency.

State changes can target storage reached through a capability or capture, not just the descriptor
being edited. Preserve the checked write footprint and post-state of those referent owners too.
For example, if a closure stores a view in an outside holder, destroying the closure releases its
capture access but does not erase the view now stored in that holder. A guard releasing update
access similarly cannot erase dependency changes made to its protected owner's payload. Public
capability/callable schemas must describe these reachable-state transitions; opaque wrappers and
readable shared-state APIs cannot hide them as descriptor-local mutation. No extra application
annotation is required merely because of implementation layout.

A complete contract distinguishes access to an input's storage from dependencies carried by its
value. It also distinguishes environment storage from dependencies carried by captures. Forwarding
an owned view can preserve outside dependencies without retaining its dying descriptor slot.
Taking an ordinary buffer cannot return a view into that buffer when callee cleanup destroys it.
Copying the taken buffer at the caller does not redirect the view into the caller's different slot.

### Preparation, Invocation and Evaluation Temporaries

Evaluate the callable/receiver and explicit inputs exactly once, left to right. A direct read or
edit place is reserved while later operands are evaluated, then its operation activates at
invocation. A direct non-Copy `take` source is reserved for transfer. A Copy `take` operand is
snapshotted when that operand is prepared. The snapshot is not reread after later operands.
Primitive scalar operations likewise preserve each prepared scalar value.

Reservations prevent destruction, transfer, replacement and conflicting writes, but are not
already-active read/edit calls. This permits `buffer.append(buffer.len())` when the inner read
finishes before edit activation. A computed/explicit local `edit` alias is already active and
keeps its own restrictions; it is not an invocation-time reservation. A read input of Copy type
still accesses original storage: a later conflicting write is not permitted merely because a
snapshot could have been made.

After preparation, validate the complete bundle, including captures and carried capabilities.
Commit transfers and activation without an intervening user callback, destructor or fallible
operation. Required allocation/validation precedes commit. Later preparation failure releases
reservations, cleans constructed prepared values in reverse order, and does not consume a merely
reserved owner. Earlier executed moves in nested value construction and other effects are not
rolled back. Once invoked, received values belong to the callee even if it fails.

A read call may use a fresh non-Copy value without cloning. Its evaluation temporary survives
through the enclosing statement, loop condition or block-result evaluation. Install/stage a
result before cleaning remaining evaluation temporaries in reverse construction order. A result
cannot escape while still depending on a temporary that is cleaned. Moving a temporary into an
owner transfers its cleanup obligation. Cancellation unwinds nested evaluation/call/lexical frames
in actual order. A terminal trap without unwinding does not acquire cleanup from these rules.

### Return, Replacement and Cleanup

Evaluate a return once under the ordinary value-construction rule, stage its value/dependencies,
perform callee cleanup, and then hand off. Protect returned capabilities through cleanup and
reparent any delegated access without an unprotected interval. Reject access into a destroyed
local, temporary, consumed parameter/environment or revoked guard. Do not suspend a normal
function, postpone a guard destructor, extend a region, or add reference counting to repair it.
Return the actual guard/owner when the resource must survive.

Assignment is RHS-first under [assignment and flow](bindings-and-initialization.md#future-direction-v2-0-0-assignment-and-flow).
Old-value cleanup must be valid with the replacement's retained dependencies. Recoverable failure,
`throw`, `?` and cancellation preserve exactly the initialized resources of their path. Conditional
cleanup is not permission to read maybe-initialized data or skip a required user destructor.

A field can move when no live access prevents it. If a user destructor needs the complete parent,
all paths reaching it must restore required fields, including failure/cancellation paths. Do not
blanket-ban such moves, call a destructor with missing fields, or silently skip it. Without a
user destructor, partial cleanup destroys only initialized owned parts.

For a record without a user destructor, moving a field makes that selected value unavailable,
not its initialized siblings. A containing value is incomplete while any of its fields is
unavailable: it cannot be read, copied, transferred or passed as a complete input. Operations on initialized disjoint fields remain possible, subject
to access conflicts. These are logical places; identical physical offsets do not merge fields.

A writable owner can restore a moved field with a complete replacement. Replacing a partially
initialized field first cleans its remaining initialized parts and then installs the complete
new value. Moving an entire containing value removes its field containers too; writing a deeper
field does not reconstruct that missing parent. Reconstruct the missing parent as a complete
value instead. Fixed owners cannot restore fields merely because they could previously move them.

Implicit cleanup of a completed record without a user destructor visits its remaining initialized
fields in reverse declaration order, recursively applying each field's cleanup contract and
skipping moved fields. It does not read an incomplete parent or invent initialization for absent
parts. If every field has moved, cleanup releases the empty containing slot without disposing any
field again. This completed-value rule is separate from cleaning a failed construction, whose
successfully initialized operands are cleaned in reverse actual initialization order under the
construction contract.

An explicit `drop name` ends a definitely present owning container, including a fixed or Copy
binding, and makes it unavailable. For a record without a user destructor it cleans remaining
initialized parts even after partial moves, including the all-fields-moved case. It is cleanup,
not reading or supplying a complete value; it neither requires restoring moved fields merely to
end ownership nor chooses a non-invalidating Copy. A user destructor's checked prerequisites
still apply and cannot be bypassed. A wholly moved/dropped container is absent and cannot be
dropped again. An alias cannot destroy an outside referent. Unit and scalar payloads have no
destructor work; their logical initialization still matters.
The opaque error owns its private immutable failure graph under its
[error contract](../../std/error/README.md), not a special exemption from these operations.

### Generic, Outcome, Async and Optimization Boundaries

Function types, interface witnesses and generic forwarding retain input operations, invocation
kind, result origins, edited-state relations, effects and cleanup obligations. Call-site inference
and specialization cannot invent capabilities omitted by a public contract. Names/source
locations are presentation, never semantic operation or dependency identities.

Optional/fallible tags do not erase carried dependencies. `?`/`await` deliver value payloads under
their owning contracts; they cannot extract a move-only payload through read/edit access. Successful
error-valued results use ordinary `return`; `throw` constructs failure with the single error
payload. Explicit `some(...)`, ordinary optional lifting and `()` keep their adopted meanings.

Deferred calls retain needed input/capture roots until drive/output/cancellation no longer needs
them. Constructing a frame does not make references independent. Resource cleanup order and drive
safety survive backing-store/frame optimizations under the
[future storage rules](memory-and-regions.md#future-direction-v2-0-0-storage-optimization).
No authored `noalloc`, `notrap` or `blocking` annotation is restored. No rule here promises a
physical copy, allocation, universal optimal code, or a measured speedup.
