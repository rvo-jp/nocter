# Value Access Model

## Future Direction: v2.0.0

This chapter defines the adopted v2.0.0 value-access rules. They are not part of the current
released contract. Current ownership rules remain in [Ownership, Borrowing, and Drop](ownership.md).
This future chapter owns operation permissions, Copy materialization, access bindings, and result
handoff; it does not introduce a second current-language borrowing syntax.

Ordinary parameter/result placement, receiver abbreviations, callable invocation, and local
read/transfer inference are defined here and in the linked future callable section. The
[type-scope rules](values-and-types.md#future-direction-v2-0-0-access-qualifier-scope) distinguish
outcomes of access from access to stored outcomes. Complete implementation signatures are
retained under the [future module contract](modules.md#future-direction-v2-0-0-complete-implementation-signatures).
Full composition and module migration remain work. This does not reopen the selected lifetime,
initialization, reservation, or cleanup obligations. No complete compiler implementation or
soundness proof is claimed.

### Type-Side Declarations and Inferred Locals

An ordinary declaration writes its permission with its type:

```text
func open(id: u64): ^File!
func append(buffer: &Buffer, value: u8): void
```

Unqualified T provides input/read, `&T` update, and `^T` ownership. `&T` does not regain its
released readonly meaning. Declarations and interface contracts remain authoritative; calls
repeat neither parameter nor result permission markers.

An unannotated local infers the qualified result of the initialization operation, not the stored
permission of a named source variable:

```text
let first = Buffer.create(64)? // ^Buffer, supplied by the factory's successful result
let next = ^first             // ^Buffer, explicitly transferred; first is unavailable
let inspect = next            // Buffer, read access; next remains the owner
```

An explicit update-access expression supplies U:

```text
let editable = &next          // &Buffer, delegates update access to next
```

This is a separate example from keeping `inspect` live. `&place` requires definitely initialized
writable owned storage or an existing U access. It cannot upgrade I, target a fresh temporary,
copy a capability, or transfer ownership. Grouping preserves the operation. Reading the resulting
`editable` into another unannotated local still supplies I. Type-side `&T` declares a contract;
expression-side `&place` performs delegation. `let &editable` is not another spelling.

Reading an existing place supplies I, even when the place is owned, writable, or Copy. This
includes an identifier, field, element, or range selected without an explicit transfer. `^place`
instead supplies X by transferring an initialized owner. A call supplies its selected result
permission; binding a U-returning call therefore infers U without another marker. Parentheses
preserve the operation rather than turning a read into a transfer. Successful `?` extraction and
`await` preserve the selected payload/output permission, not ownership of the outer carrier.

An explicit `: T`, `: &T`, or `: ^T` annotation requests that permission and data type, subject to
source eligibility. It cannot manufacture authority or replace a missing source-transfer operation.
No `: ^_` placeholder is mandatory for inferred owned results. Independent I answers supplied by
a declared read-result contract retain I unless an explicit destination requests a checked Copy.
Numeric, Boolean, and unit literals and newly computed primitive scalar values construct fresh
X values; they do not access an existing place. A literal denoting static storage instead follows
that storage contract without introducing an allocation. Inference does not inspect later uses,
Copy specialization, or an optimizer's storage choice to change an already checked binding. `var` remains removed.

The distinction is between initialization operations, not two interchangeable styles of the
same operation. The type qualifier `^T` states an ownership contract; the expression `^place`
performs a transfer. There is no alternative inferred move spelling `let next = first`.

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
| `value: &T` | Update | Update the original writable place; leave it initialized on recoverable exits |
| `value: ^T` | Ownership transfer | Receive ownership and dispose of or forward each live owned part once |

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
func append(buffer: &Buffer, value: u8): void
func enqueue(job: ^Job): void

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
let read = first        // read access, including when first is Copy
let second: ^T = first  // explicit Copy request; requires Copy evidence for T
let third = ^first      // transfer; first is unavailable even when T is Copy
```

`^place` transfers an existing owned place into standalone storage/return expressions. A fresh
owned result already supplies ownership and needs no transfer prefix. General transfers do not
become implicit because the source has no later use.

### Result Contracts and Local Bindings

A callable declares result permission beside the result type, independently of its input modes:

```text
func bytes(buffer: Buffer): [u8]
func edit_bytes(buffer: &Buffer): &[u8]
func copy_bytes(buffer: Buffer): ^Vec<u8>
```

Permission is part of the contract, not the lookup name or an overload discriminator. Changing it
changes the API contract. Calls omit result assertions. Locals infer what their initialization
operation supplies:

```text
let input = bytes(buffer)
let editable = edit_bytes(buffer)
let owned = copy_bytes(buffer)
```

These are separate examples; simultaneous overlapping input/update access is still invalid.
The resulting permission determines assignment and transfer:

| Resulting local permission | Assignment and ownership |
| --- | --- |
| I: read | No update, replacement, or transfer of the referent |
| U: update original storage | Update/replace the original; no unfilled ownership extraction or retargeting |
| X: own a writable value | Update, replace, transfer, and valid reinitialization of its own slot |

An I binding to an existing place/access retains a read relationship regardless of Copy. An
independent read/Copy answer has no outside referent of its own. An explicitly I/U destination
cannot retain a fresh X temporary as an invisible owner; an unannotated binding instead receives
that X value openly as its own owner. An explicit I destination can materialize a separate Copy
answer from a fresh Copy value when proved, preserving its D/S dependencies and finishing the
source temporary's cleanup. This retains no hidden owning parent and does not promote I to X.
Fresh noncopyable X cannot satisfy that I binding.

A U binding requires an existing writable owned place or supplied U access. Its original place
and required structure stay protected for the access's live range. A bare name naming a U binding
still supplies a read when used to initialize another unannotated local; it does not duplicate an
exclusive capability. An explicit U destination or U-result handoff delegates access, restricting
the parent while the child is live.

`&place` establishes that delegated access when the expression is evaluated, rather than reserving
it until a surrounding operation completes. Its target and carried dependencies remain protected
through its live uses and dependent cleanup. An active U child excludes overlapping access through
its parent; an I child permits compatible reads but prevents parent update/transfer. Ending a child
restores only the surviving parent's rights, subject to its other live children. No lexical block,
explicit lifetime name, or separate release statement is needed for an ordinary access whose last
use and required cleanup have ended. This does not move a resource destructor earlier.

Infer the permission still required by later uses and cleanup. After an access's last update
requirement, compatible read access may suffice for its remaining reads. This does not change its
declared type or reselect operations: a later U argument, U result, explicit update delegation or
write keeps the update requirement live. Explicit `&place` still requires exclusive eligibility
at its evaluation point, even if its answer will only be read. No runtime upgrade is introduced.

When an intermediate access has no remaining use or cleanup obligation, reparent its live children
to the surviving authority for the same root. Preserve each child's permission and dependencies.
An unused U intermediary does not keep exclusive access merely because a live I child was created
through it: that child still prevents update/transfer, while compatible reads can resume. A U child
continues to exclude overlapping reads after reparenting. Retain an intermediary needed by a later
operation or prepared call at the permission that call requires. A pending I request does not
retain an otherwise unused U intermediary: preserve its identity reservation and read authority
without activating an input early. Never upgrade I, duplicate U, or keep a destroyed root alive.

An X binding receives a fresh owned result or an explicitly transferred owner. An explicitly X
destination may also materialize a permitted Copy from a readable value, retaining payload and
storage dependencies. Thus `let next = ^previous` transfers an existing owner, while
`let snapshot: ^T = previous` requests a checked Copy. A move-only source in the latter form is
an error. Destination ownership never supplies an omitted source-transfer operation.

```text
let limit: i32 = 100
let count = 0
count += 1
count = 10
```

I bindings cannot be assigned. Assignment to U updates its identified caller-owned place, even
for whole-value replacement; it never retargets the access. Assignment to X replaces its own value
and performs required old-value cleanup. After transfer or explicit drop, X can be reinitialized
by assigning a complete replacement; failure before that replacement is ready leaves the slot
uninitialized. Maybe initialized slots still require definite-initialization checks and conditional
cleanup. A failed replacement does not itself clean or install a destination value, but it does
not undo explicit moves or other side effects already performed by the right-hand expression.
U must remain valid on every outward recoverable edge.

There is no separate immutable-but-transferable owned local form. Update permission follows X
ownership, as it does for X parameters. Passing temporary I access still restricts the called
operation without freezing every future operation on its owner.

Binding I/U access does not manufacture a freely storable reference. Access remains bounded
by its owners and cannot be stored as an independent T. Persistent relationships require an
explicitly owned dependent computation/capability
or an owned container with a checked index/range, rather than an implicit clone or lifetime repair.

### Generic Evidence and Origin Preservation

A generic body uses its declared capabilities. An unconstrained T is potentially move-only.
Specializing that body with a Copy type does not reselect a checked transfer, convert an I
binding to X, erase an origin, or change a public result mode. The caller can
perform a separate permitted Copy operation when its own checking context proves Copy.

For example, a generic I accessor can return access to an I parameter. Its checked body remains
an accessor when specialized with an integer. A caller whose initializer knows the integer type
can request an X binding and materialize an independent integer answer. An owning generic identity instead always uses
ownership transfer:

```text
func identity<T>(value: ^T): ^T {
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
as one non-fallible transition. Reservations are protections, not already-active I/U permissions.
A source's active children must permit the requested operation at commit; a child that finishes
during operand preparation need not prevent reserving the address. Actual replacement, transfer
and conflicting writes remain forbidden while reserved. An already-active computed access keeps
its own restrictions during preparation. No user callback, allocation, or destructor occurs between the
first and last committed ownership transfer. Required allocation and place validation precede
that transition.

Failure during later argument preparation releases reservations and cleans constructed temporaries;
it does not consume an earlier merely reserved owned place. Earlier side effects and separate
ownership operations are not rolled back. Failure after invocation leaves transferred ownership
with the callee and leaves U inputs valid with their completed changes.

### Evaluation Temporaries

An I call input may read a fresh X value without requiring Copy. Keep that explicitly evaluated
temporary alive through the enclosing statement, loop condition, or block-result evaluation,
including use of the call's answer. This is temporary storage, not an invisible owning local.
After the destination is installed or the result staged, release remaining evaluation temporaries
in reverse construction order, before leaving the containing block. A retained result cannot
outlive a temporary it accesses. Copying a scalar answer can remove that referent; binding access
cannot extend the temporary's lifetime.

Moving a temporary into a local, returned owner, or X call input transfers its cleanup obligation.
Preparing an I input does not transfer that obligation to the callee. Cancellation destroys only
the successfully prepared owned arguments, in reverse preparation order, then releases other
temporaries in the exited evaluation frames. A merely reserved named owner remains intact.
Nested evaluation, call, and lexical frames unwind in their actual nesting order. Traps that
terminate without unwinding do not acquire source cleanup from these rules.

For the bounded first fragment, `bool`, `i32`, and `void` have no contained access or external
storage requirements and need no resource destructor. The opaque built-in `error` follows its
[owned snapshot contract](../../std/error/README.md): it owns its private immutable failure graph,
has no access into input views or caller-managed region dependency, and releases that graph when
destroyed. It remains move-only. These are exact data contracts, not consequences of Copy or X;
no such independence is inferred for a future aggregate, closure, future, or generic value.

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

Moving an owned field also changes the parent's initialization state. A user destructor that
requires the complete parent may run only with that state restored on every path reaching it,
including failure and cancellation. Do not silently skip that destructor or call it with missing
fields. This is a path-sensitive obligation, not a blanket prohibition on moving fields from
every type with a destructor: a verified reinitialization before all affected exits can satisfy
it. Without a user destructor, ordinary partial cleanup disposes only the remaining initialized
parts. A complete-value transfer cannot carry an uninitialized required field.

### Boundaries with Other Contracts

Result modes survive generic forwarding, interface matching, and callable abstraction. Environment
invocation and output permission are distinct; see the
[future callable contract](callables.md#future-direction-v2-0-0-value-access-callables).

Optional/fallible result tags and future ownership do not turn a non-owning success payload into
an owner. Their complete v2.0.0 composition must preserve the result access and carried
dependencies defined here. This chapter does not silently replace current outcome/future syntax,
standard-library contracts, or target ABI rules. Those owning contracts must be migrated coherently
before the complete v2.0.0 language boundary takes effect.
