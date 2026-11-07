# Value Access Model

## Future Direction: v2.0.0

This chapter defines the adopted v2.0.0 value-access rules. They are not part of the current
released contract. Current ownership rules remain in [Ownership, Borrowing, and Drop](ownership.md).
This future chapter owns operation permissions, Copy materialization, access bindings, and result
handoff; it does not introduce a second current-language borrowing syntax.

### Ownership, Access, and Dependencies

An owned value has one initialized owning place responsible for its eventual transfer or cleanup.
Access to that value does not itself create another owner. A place includes a binding, field,
element, or range selected by an expression; distinct places need not have distinct numeric
addresses, particularly for zero-sized values.

The compiler tracks three different relationships:

- Access to a particular existing place, which prevents conflicting operations while that access
  is live.
- Dependencies contained inside a value, such as a closure's access to captured outside data.
- Storage requirements, such as an allocation region that must remain valid.

Owning a value does not imply that the last two sets are empty. Copying a closure does not copy
the data it accesses. Storage requirements do not by themselves grant read or update permission.
The [future storage rules](memory-and-regions.md#future-direction-v2-0-0-value-access-origins)
define the public origin bounds.

### Input, Update, and Ownership Transfer

Parameter and receiver declarations select one permission:

| Declaration | Permission | Responsibility |
| --- | --- | --- |
| `value: T` | Input/read | Leave caller ownership intact; do not update through this access |
| `&value: T` | Update | Update the original writable place; leave it initialized on recoverable exits |
| `^value: T` | Ownership transfer | Receive ownership and dispose of or forward each live owned part once |

These permissions are called I, U, and X in descriptions of the model. Those letters are not
language keywords. A type T is not rewritten into a freely storable `&T` or `&+T` value type.

Inputs may overlap inputs. Overlapping input/update, update/update, and transfer/access cannot
be simultaneously active. Proved-disjoint fields or ranges can be accessed independently.
Delegation is different from two independent accesses: the parent is restricted or suspended
while its child access is live. A parent update can delegate input or update; an input cannot
delegate update. Neither transfers ownership of the caller's place.

U must leave a valid initialized value on success, failure, and cancellation. It need not undo
completed changes. An exchange operation must prepare a replacement before removing the old
owned value; it cannot expose a fallible uninitialized gap. An X parameter instead owns a
writable local slot, including the right to replace or transfer its contents, without another
mutable local declaration.

Calls uniformly derive receiver and argument permissions from the selected declaration:

```text
func append(&buffer: Buffer, value: u8): void
func enqueue(^job: Job): void

append(buffer, 42)
enqueue(job)
```

Do not retain direct argument-mode prefixes or method `.&`/`.^` markers as alternate call styles.
An unavailable permission is an error; it does not select another overload, silently clone a
value, or change the declaration.

### Copying Is a Separate Operation

Implicit copying requires a checked Copy capability. Nominal structs opt in with `copy struct`;
private copyable fields alone do not make an ordinary nominal type Copy. Copying is structural,
has no custom hook or resource acquisition, and does not allocate. Copy does not promise that a
large value is cheap to copy.

Scalar primitives, unit, payloadless enums, and supported raw boundary values permit Copy.
Tuples, fixed arrays, and optionals require Copy for every component type, independently of the
active branch or array length. Generic code needs declared Copy evidence. `error`, fallible
outcomes, and futures remain move-only. A Copy type cannot have a destructor.

A closure can be Copy when its owned captures are Copy and its remaining captures only retain
input access. Every copied closure retains those outside dependencies. Update captures prohibit
Copy. Opaque values expose only capabilities promised by their contracts.

Copy materialization creates a separate owned value. It removes the new value's need to access
the old value's containing place, but preserves dependencies inside the copied payload and its
storage requirements. For example, copying a read-capturing closure can allow the old closure
header to be discarded; it cannot allow the captured outside data to be destroyed.

An ordinary owned initializer, assignment, or owned success return can perform a permitted
Copy. An I call argument still accesses the original place even for Copy T; it does not acquire
snapshot semantics merely because bits could be copied. An X call with an existing owned place
transfers it uniformly for Copy and move-only types. It does not choose a non-invalidating copy.
An I/U access is not ownership of its referent. Supplying it to X requires checked Copy evidence:
preparation materializes a separate owned snapshot and the invocation receives that snapshot.
Without Copy evidence the call is invalid; it cannot extract the caller's original value or
insert a clone. This rule does not require an otherwise unnecessary intermediate local.

```text
let second = first   // independent Copy; rejects a move-only owned source
let third = ^first   // transfer; first is unavailable even when its type is Copy
```

`^place` transfers an existing owned place into standalone storage/return expressions. A fresh
owned result already supplies ownership and needs no transfer prefix. General transfers do not
become implicit because the source has no later use.

### Result Contracts and Local Bindings

A callable declares result permission before its identifier, independently of its input modes:

```text
impl Buffer {
    method self.bytes(): [u8]
    method &self.&edit_bytes(): [u8]
    method self.^copy_bytes(): Vec<u8>
    method ^self.^move_bytes(): Vec<u8>
}
```

An unmarked name provides I, `&name` provides U, and `^name` provides X. The marker is part of
the callable contract, not its lookup name or an overload discriminator. Changing it changes the
API contract. An ordinary call or binding does not repeat it:

```text
let input = buffer.bytes()
let editable = buffer.edit_bytes()
let owned = buffer.copy_bytes()
```

These are separate examples; simultaneous overlapping input/update access is still invalid.
Bindings derive their category at initialization:

| Initializer result | Binding |
| --- | --- |
| Fresh X result or explicit transfer | Own the supplied value, preserving its dependencies |
| Existing owned place without transfer | Own a permitted Copy; reject if Copy is unavailable |
| I result with checked Copy materialization | Own that separate Copy, preserving payload dependencies |
| I result without Copy materialization | Retain input access and its owner dependencies |
| U result | Retain update access, including when the referent is Copy |

When an I result's type is proved Copy at that source checking boundary, an ordinary local
initializer materializes the Copy. Without such evidence it retains access. Do not make the
choice from later uses or an optimizer's knowledge. A U initializer does not copy its referent
and silently lose the promised update relationship.

`let` fixes a local binding. An owned `let` cannot be reassigned or updated as writable owned
storage; an access binding instead retains the permission supplied by its initializer, so a
fixed U binding can update the original referent. Reassigning an owned binding requires `var`. A
transferred `let` cannot be reinitialized. Writable `var` storage can be reinitialized only after
the replacement is fully evaluated and initialization checks succeed.

Assignment to a U access targets its identified caller-owned place, including whole-value
replacement; it does not retarget the access binding. An I access is not an assignment target.
Writing `var` does not grant missing update/ownership authority or introduce an operation for
retargeting an access to another owner. Owned binding reassignment remains a separate case.

Binding an I/U result does not manufacture a freely storable reference. Access remains bounded
by its owners, cannot be stored as an independent T, and cannot acquire ownership from a use-site
assertion. Persistent relationships require an explicitly owned dependent computation/capability
or an owned container with a checked index/range, rather than an implicit clone or lifetime repair.

### Generic Evidence and Origin Preservation

A generic body uses its declared capabilities. An unconstrained T is potentially move-only.
Specializing that body with a Copy type does not reselect a checked transfer, convert an access
binding to an owning binding, erase an origin, or change a public result mode. The caller can
perform a separate permitted Copy operation when its own checking context proves Copy.

For example, a generic I accessor can return access to an I parameter. Its checked body remains
an accessor when specialized with an integer. A caller whose initializer knows the integer type
can materialize an independent integer answer. An owning generic identity instead always uses
ownership transfer:

```text
func ^identity<T>(^value: T): T {
    return ^value
}
```

Copy removes only the access needed to read the source value, not dependencies already carried
inside T. An explicit receiver-origin bound on an abstract associated result remains authoritative
until a checked materialization or other independence operation discharges the particular access.

### Preparation and Invocation

Select one callable and its complete contract before checking operand permissions. Evaluate the
callable/receiver and arguments exactly once, left to right. Preparing an existing place reserves
that place against destruction, transfer, replacement, and conflicting update while later operands
are evaluated. Temporary reads that finish before invocation can remain valid:

```text
buffer.append(buffer.len())
```

When a non-owning Copy argument is materialized for X, take that snapshot during its left-to-right
preparation, preserving payload dependencies and storage requirements. It becomes a prepared
owned temporary; later operands do not change its copied answer. Other live uses of the original
access still retain their normal restrictions. An existing owned X source instead remains
reserved for invocation-time transfer, with no Copy fallback.

After all preparation succeeds, check the complete access bundle and activate updates/transfers
as one non-fallible transition. No user callback, allocation, or destructor occurs between the
first and last committed ownership transfer. Required allocation and place validation precede
that transition.

Failure during later argument preparation releases reservations and cleans constructed temporaries;
it does not consume an earlier merely reserved owned place. Earlier side effects and separate
ownership operations are not rolled back. Failure after invocation leaves transferred ownership
with the callee and leaves U inputs valid with their completed changes.

### Return and Cleanup

An X result returns a fresh owned value, a permitted Copy, or an explicitly transferred existing
owner. An I result exposes eligible surviving storage or a permitted Copy answer; a U result
exposes writable surviving caller storage. A scalar answer such as `len(): usize` can be copied
before cleanup, without retaining access to the source counter.

Ordinary `return` evaluates the result once, protects its referent and dependencies during exit
cleanup, performs normal callee cleanup, and hands the result to the caller. Escaping delegated
access is reparented without an interval that permits conflicting access or creates two exclusive
authorities. A cleanup operation conflicting with the staged result is invalid.

Returning access into an owned local, temporary, or consumed parameter that is destroyed on exit
is invalid. The compiler must not suspend the ordinary callable, delay a local guard's destructor,
extend a region, or insert reference counting to make that return valid. Return an owned guard or
capability when its resource must survive; access through that live owner remains separately checked.
Neither `yield` nor a `view` return form is introduced.

Control-flow joins require definite initialization for every use and conservatively retain all
possible live dependencies. Mutually exclusive branch accesses are alternatives, not simultaneously
active updates; a use after the join must nevertheless be safe for every incoming alternative.
Loops preserve these obligations on every backedge. Conditional cleanup disposes only initialized
owned parts and never grants source permissions to a maybe initialized value.

### Boundaries with Other Contracts

Result modes survive generic forwarding, interface matching, and callable abstraction. Environment
invocation and output permission are distinct; see the
[future callable contract](callables.md#future-direction-v2-0-0-value-access-callables).

Optional/fallible result tags and future ownership do not turn a non-owning success payload into
an owner. Their complete v2.0.0 composition must preserve the result access and carried
dependencies defined here. This chapter does not silently replace current outcome/future syntax,
standard-library contracts, or target ABI rules. Those owning contracts must be migrated coherently
before the complete v2.0.0 language boundary takes effect.
