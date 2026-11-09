# Value Access Model

This chapter owns the adopted v2.0.0 value-binding foundation within the
[development specification](../README.md).
Ordinary data types describe values. Binding and parameter operations describe how those values
are accessed. Capability values and callable contracts retain the access/dependency information
needed for safety; that information is not removed from the type system.
The [type-system foundation](values-and-types.md#type-system-foundation) separates data identity,
boundary compatibility, inference and evidence for an actually produced value.

Ordinary type-side `&T`/`^T`, prefix `&place`/`^place`, and permission-inferred `let` are not
alternative source forms. Postfix place^ selects transfer under the rules below. The
[private handoff](../../architecture/pipeline/value-access-declarations.md) owns implemented scope
and remaining limits; source design alone is not target realization or a general soundness proof.
An unchanged released production does not silently become a v2.0.0 production.

## Values and Scoped Aliases

| Declaration | Meaning |
| --- | --- |
| `let value = expression` | Construct a fixed value binding |
| `var value = expression` | Construct a mutable value binding |
| `read alias = place` | Read existing storage through a scoped alias |
| `edit alias = place` | Update existing writable storage through a scoped alias |

`let` and `var` construct values under the same value-supply rule. Both can deliver their values
elsewhere; only `var` permits ordinary updates/replacement. A `let` value cannot be reinitialized
after transfer/drop. A `var` slot can be reinitialized by a complete replacement. Optional type
annotations constrain the data type, never change the binding operation.

```text
var count = 10
let snapshot = count       // independent i32 value
count = 20                 // snapshot is still 10
let file = open(path)?
let next = file^           // explicit transfer; file is now unavailable
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
[projection protocol](place-projections.md#index-projections); it does
not make an ordinary function return an alias.
Optional/fallible payloads expose existing storage under the
[scoped outcome protocol](outcomes.md#scoped-payload-projections), including local recovery.
That protocol preserves the carrier and its active tag for read/edit access; ordinary value
elimination follows the supply rule and cannot acquire ownership through an alias.

## Value Construction and Copy

Every value-producing boundary delivers a value: `let`/`var` initialization, replacement,
aggregate component construction, ordinary block/return results, selected outcome payload
construction and `take` argument supply. Resolve the data type and capabilities, then apply:

1. An already constructed value transfers to the destination, preserving its dependencies.
2. A marked place^ supplies an explicit transfer from an initialized eligible owning place,
   making that source unavailable at the consuming operation's commit point, even for Copy data.
3. An unmarked initialized readable place supplies a separate snapshot only with source-level
   Copy evidence. An existing non-Copy owner needs ^; it does not move implicitly.
4. Otherwise reject the boundary. Do not silently create an alias, invoke a clone, infer Copy from
   later specialization, or choose the operation from whether the source has a later use.

A fixed owner is eligible for transfer, subject to access/initialization checks. Unmarked Copy
inputs remain available after value supply; marked transfers do not. Copy is structural, has no
user hook, allocation or resource acquisition, and does not mean that a large value is cheap.
A permitted Copy can be optimized away only while preserving snapshot semantics.

Nominal structs opt in with `copy struct`; private Copy fields alone do not make an ordinary
nominal Copy. [Enums](structs-and-enums.md#copy-contracts) expose their scalar or opted-in
`copy enum` policy; a private completion cannot give an opaque root an unadvertised Copy rule.
Scalars, unit and supported raw boundary values permit Copy.
Tuples and optionals require component Copy evidence, independently of a runtime active branch.
[Fixed arrays](tuples-and-arrays.md#repetition-and-copy) require element Copy evidence unless
their exact length is proven zero. Generic code needs evidence in its own checking context;
an unknown count or a later specialization does not supply it. `error`, fallible outcomes and futures
remain move-only. Copy types cannot have user destructors. A closure's Copy evidence depends on
its captures, not its invocation kind. Exclusive update capabilities are non-Copy even when their
element type is Copy.

Copy removes only the need to read the old containing place. Dependencies carried inside the copied
payload and external storage requirements survive. Copying a read-view descriptor does not copy its
referent. For an existing place, its written marker and source-context Copy evidence select supply
once; monomorphization cannot turn a marked transfer into a non-invalidating Copy.

```text
func identity<T>(take value: T): T {
    return value^
}
```

The generic body transfers its received value. A caller knowing `T = i32` can supply a snapshot to
`take`; that does not change the body's operation. A generic read input without Copy evidence
cannot be returned as a plain T. A separate capability result can expose checked access instead.

A data type known from a callable contract, structural value, operator or compatible branch
remains known even if evaluating an operand exits before producing that value. Normal completion
is a separate control fact, not permission to erase the known type or manufacture a runtime
value. Inference and structural type/field validation use that known type; execution, ownership
and cleanup follow only reached operations. A control expression with no known result shape
cannot supply a record field type merely because its continuation is unreachable.

## Explicit Transfer Sources

Postfix `place^` selects transfer from an existing owning place. It is mandatory when an
existing non-Copy value is supplied as a value, and retires the selected source even for Copy T.
An unmarked Copy place still snapshots. A fixed owner may transfer; a var can later restore its
absent value by complete replacement. A field/owning fixed-array element retires only that
subplace; transferring the containing value retires the complete value and its field containers.

```text
let next = file^
let copied = count
let transferred = count^
let product = buffer^.finish()?
consume(make_value())
```

These are schematic uses with appropriate declarations. A fresh call/literal/constant recipe,
already produced result or computed component needs no marker; applying ^ to that value or to
another transfer result is invalid. Grouping preserves the original place/transfer category.
There is no type qualifier, prefix form, caller-side edit assertion or overloaded ^ hook.

The source must be complete, initialized and eligible for transfer under access, dependency,
relocation and ancestor-destructor rules. A read/edit alias, user index/current projection,
static place or non-owning scoped outcome projection cannot gain extraction rights through ^,
even when its data is Copy. A copied view descriptor still carries its referent dependencies;
transferring its owning holder does not transfer ownership of those referents. No root is
retargeted, extended or silently cloned. Incomplete owners can instead receive their existing
direct drop/remaining-field cleanup, which is not complete-value transfer.

`take` describes receipt/cleanup at the destination. The suffix describes retirement at the
source. Passing a fresh value or unmarked Copy snapshot to take does not need ^. Conversely,
read/edit can receive `place^` through a caller-owned evaluation temporary: commit transfer
there, lend the requested access, and retain the temporary only for its ordinary evaluation
extent. A marked source supplies a value, not a scoped alias initializer or assignment target. The
marker does not select a callee, input mode, callable view or overload.

### Preparation and Commit

A direct marked call input/receiver evaluates its place/path/index once and reserves transfer,
including for Copy T. It does not retire the source merely when the caret token is evaluated.
After every operand/backing check succeeds, validate the complete bundle and commit transfer
with invocation. Later operand reads can finish before that point; conflicting writes, another
transfer or a simultaneous surviving input alias cannot invalidate the reserved source.
No intervening user callback, destructor or failure is inserted into commit. If preparation
fails recoverably or is cancelled, release the reservation without consuming that source.

```text
send(data^, load_options()?)
```

Assuming load_options exits recoverably before send invokes, the reserved data has not moved.
Transparent parentheses do not make this eager. An unmarked Copy taking operand instead
snapshots at preparation as before; the explicit source retirement never becomes Copy after
specialization. Argument data wrappers/adapters prepared by the ordinary boundary retain the
same commit, with actual backing/validation ready beforehand.

Other value-producing consumers keep their established boundaries. A local/RHS, return, throw,
explicit capture or aggregate component commits its transfer when that value is constructed or
staged. Thus `(file^, fail()?)` has already transferred its first component when the second exits.
An explicit inner call, outcome elimination or data-field selection of a transferred whole owner
must acquire its value before proceeding; an outer call cannot undo that completed operation.
Member lookup for `buffer^.finish(arguments)` instead retains its direct marked receiver until
that invocation commits. No later liveness, Copy instantiation or private-body lookup changes
these recorded producer categories or commit points.

Postfix order is left to right: `record.field^` transfers that field; `record^.field` first
acquires the complete record and then produces a computed component. The latter's remainder
keeps its ordinary evaluation cleanup. `value^?`, `value^!`, `return value^`, `throw reason^`
and `await pending^` use the same rule. Await acquires a marked future when await starts, before
drive/suspension; its ordinary cancellation behavior remains. Scope cleanup and explicit
`drop name` need no caret because they dispose a value rather than supply one elsewhere.

### Anonymous Carriers and Sequences

Once a carrier is already received/produced, its internal payload/tuple-pattern/cursor transfer
can hand off values under its checked protocol without tokens on synthesized slots. It must
not silently move another named source. Named take parameters, handler locals, pattern leaves
and captures are existing owners again, so their later source-level transfers need ^.

An existing pack is an argument sequence, not ordinary data. In sequence capture, loop source
or expansion positions, `pack^` explicitly transfers all its owning components, including Copy
keys/values, under the same per-entry formation/preparation rule. Its metadata count remains;
transferred component values do not. An unmarked value acquisition needs component Copy evidence.
Read/edit packs cannot transfer caller components through this suffix. Bare-name data contexts
still cannot turn pack or pack^ into a first-class array/tuple.

For `...array^`, the marker transfers the complete array into the expansion's evaluation owner;
it is not an alternate spelling for moving individual original elements. At a direct built-in
call expansion reserve that whole transfer until outer invocation commit; acquired anonymous
components then supply the receiving pack. Unmarked direct built-in components follow their
own declared input operation, and a taking non-Copy component needs an explicit source transfer
or an already produced owner. General user-source acquisition remains its own inner invocation.

## Field Selection from Values

A field selected from existing storage remains a place. Transparent grouping does not change
that operation. A field selected from a computed record value instead delivers a value: evaluate
the record expression once, resolve the complete field chain through its declared schemas, and
use the selected component's Copy evidence to choose a snapshot or an owning extraction. These
are distinct source categories, not a call-site choice or permission-based overload.

```text
let result = make_pair().first
let count = (Counts { first: 1, second: 2 }).first
```

A Copy selection leaves the computed record intact; a non-Copy selection transfers only the
selected component. The remainder stays owned until the existing evaluation frame's cleanup,
which applies the same remaining-field rules as named storage. A chain such as
`make_outer().inner.first` selects that full path without first moving every containing record.
It neither reruns `make_outer` nor changes logical field identity because of grouping.

The selected value may be bound, returned, stored, passed to `take`, or lent to a read/edit input
through its caller-owned evaluation temporary under the ordinary input protocol. It is not a
scoped local-alias initializer or assignment target. Name an intentional owner when access must
survive that evaluation frame; do not extend a temporary's lifetime to satisfy those operations.
Selecting a non-Copy field from a grouped read/edit alias remains an invalid
extraction from that existing place; grouping cannot launder permission into ownership.

## Input Operations and Receivers

| Input | Contract |
| --- | --- |
| `value: T` | Read supplied storage without taking its owner |
| `edit value: T` | Update supplied storage; leave it initialized on outward recoverable exits |
| `take value: T` | Receive a value and its transfer/cleanup responsibility |

Read is the sole default input operation for named functions, methods, constructors, interface
requirements and structural callable types, independently of Copy, representation, naming or
body use. Do not add explicit `read value: T` as an equivalent input spelling. Plain `self` reads
the receiver. Ordinary value-producing boundaries remain value-supply; a colon followed by T
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
let product = buffer^.finish()?
```

Calls uniformly omit input/receiver assertions. Lookup selects one operation before access
checking. Missing authority is an error, not permission to select another body, clone a value,
or silently insert a transfer. Postfix ^ selects source transfer, not an input-mode assertion.
Keep complete signatures in both public contract and implementation.

Read accesses may overlap reads. Overlapping read/update, update/update and transfer/access may
not be simultaneously active. Proved-disjoint projections may be used independently. Delegation
restricts/suspends a parent while a child needs that authority; it does not duplicate exclusivity.
An update must remain initialized on success, recoverable failure and cancellation. Removing a
value through an update requires an exchange with a ready replacement; an unfilled fallible gap
is invalid. Completed changes need not be rolled back.

## Values Can Carry Dependencies

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

## Result Origins and Retained Update State

Ordinary T results deliver values; there is no separate result permission or `yield`/`view`
return syntax. The [origin rules](memory-and-regions.md#value-access-origins)
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

## Preparation, Invocation and Evaluation Temporaries

Evaluate the callable/receiver and explicit inputs exactly once, left to right. A direct read or
edit place is reserved while later operands are evaluated, then its operation activates at
invocation. A direct marked source is reserved for transfer regardless of Copy or destination mode.
An unmarked Copy `take` operand is snapshotted when that operand is prepared. The snapshot is not
reread after later operands. Primitive scalar operations likewise preserve each prepared scalar
value.

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

A read or edit input/receiver may use an already produced value through a caller-owned
evaluation temporary, without cloning. Read lends read access; edit lends update access to
that temporary. This does not turn the expression into an assignable place or local alias
initializer. A direct existing place still needs its own authority: never snapshot a fixed/
read place or construct an adapter/wrapper merely to repair an edit input. Grouping preserves
that direct category. Explicit value construction instead keeps its already selected copies,
moves and effects before lending the resulting temporary.
Already formed capability/capture accesses remain active under their own schema; storing the
value as an operand does not downgrade those accesses to deferred reservations.

Its evaluation temporary survives
through the enclosing statement, loop condition or block-result evaluation. Install/stage a
result before cleaning remaining evaluation temporaries in reverse construction order. A result
cannot escape while still depending on a temporary that is cleaned. Moving a temporary into an
owner transfers its cleanup obligation. Cancellation unwinds nested evaluation/call/lexical frames
in actual order. A terminal trap without unwinding does not acquire cleanup from these rules.

Implementation coverage of this protocol belongs to the
[private handoff](../../architecture/pipeline/value-access-declarations.md); general dependency,
effect, cancellation and native consumers need their own admission evidence.

## Return, Replacement and Cleanup

Evaluate a return once under the ordinary value-construction rule, stage its value/dependencies,
perform callee cleanup, and then hand off. Protect returned capabilities through cleanup and
reparent any delegated access without an unprotected interval. Reject access into a destroyed
local, temporary, consumed parameter/environment or revoked guard. Do not suspend a normal
function, postpone a guard destructor, extend a region, or add reference counting to repair it.
Return the actual guard/owner when the resource must survive.

Assignment is RHS-first under [assignment and flow](bindings-and-initialization.md#assignment-and-flow).
Old-value cleanup must be valid with the replacement's retained dependencies. Recoverable failure,
`throw`, `?` and cancellation preserve exactly the initialized resources of their path. Conditional
cleanup is not permission to read maybe-initialized data or skip a required user destructor.

An existing owning field can transfer with ^ when no live access prevents it. If a user destructor
needs the complete parent, all paths reaching it must restore required fields, including
failure/cancellation paths. Do not blanket-ban such moves, call a destructor with missing fields, or
silently skip it. Without a user destructor, partial cleanup destroys only initialized owned parts.

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

[Structural aggregates](tuples-and-arrays.md) apply the same distinction to tuple fields and
fixed-array elements, including exact conditional remaining-element cleanup after dynamic moves.
This does not grant owning extraction through a read/edit or user index projection.

An explicit `drop name` ends a definitely present owning container, including a fixed or Copy
binding, and makes it unavailable. For a record without a user destructor it cleans remaining
initialized parts even after partial moves, including the all-fields-moved case. It is cleanup,
not reading or supplying a complete value; it neither requires restoring moved fields merely to
end ownership nor chooses a non-invalidating Copy. A user destructor's checked prerequisites
still apply and cannot be bypassed. A wholly moved/dropped container is absent and cannot be
dropped again. An alias cannot destroy an outside referent. Unit and scalar payloads have no
destructor work; their logical initialization still matters.
The opaque error owns its private immutable failure graph under its
[error contract](../library/error.md), not a special exemption from these operations.

## Generic, Outcome, Async and Optimization Boundaries

Function types, interface witnesses and generic forwarding retain input operations, invocation
kind, result origins, edited-state relations, effects and cleanup obligations. Call-site inference
and specialization cannot invent capabilities omitted by a public contract. Names/source
locations are presentation, never semantic operation or dependency identities.

Optional/fallible tags do not erase carried dependencies. Ordinary `?`/`await` deliver value
payloads under their owning contracts; scoped outcome projections instead retain checked access
to an active payload. Neither can extract a move-only payload through read/edit access. Successful
error-valued results use ordinary `return`; `throw` constructs failure with the single error
payload. Explicit `some(...)`, ordinary optional lifting and `()` keep their adopted meanings.

Deferred calls retain needed input/capture roots until drive/output/cancellation no longer needs
them. Constructing a frame does not make references independent. Resource cleanup order and drive
safety survive backing-store/frame optimizations under the
[future storage rules](memory-and-regions.md#storage-optimization).
The [async contract](asynchronous-computations.md) distinguishes completed edit bounds from
pending cancellation and ready-output disposal. Cancellation keeps complete initialization but
does not assert that an unstarted body established its completion post-state.
No authored `noalloc`, `notrap` or `blocking` annotation is restored. No rule here promises a
physical copy, allocation, universal optimal code, or a measured speedup.
