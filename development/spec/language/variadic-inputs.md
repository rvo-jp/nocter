# Variadic Inputs and Argument Expansion

## Scope and Parameter Identity

This chapter owns ordinary/keyed variadic inputs and built-in argument expansion from another
pack or a fixed array. [Value access](value-access.md) owns supply, reservations, dependencies
and cleanup; [fixed arrays](tuples-and-arrays.md) own array element places. These are adopted
v2.0.0 contracts, not additions to the bounded private compiler.
[Synchronous traversal](iteration.md) separately owns user-defined expansion and for loops.
Typed sequence/string entries, dynamic `[T]` and container APIs are not defined by a pack.

```text
func inspect(... files: File): void
func submit(... take jobs: Job): void
func adjust(... edit counts: i32): void
func fields(... take entries: Key: Value): void
```

These assume independently declared storable data types. A function/method/closure can have at
most one pack, in the final explicit input position. Earlier inputs are fixed inputs. Receivers
are not packs. An ordinary named pack has one name and element type; a keyed pack has one name
and key/value types. A pack header has no tuple pattern or `_` discard binder. A keyed structural
callable type also requires its name; an ordinary structural pack may omit it:
`func(... take Job): void` versus `func(... take entries: Key: Value): void`.
The pack name shares the header binding namespace with fixed-input names and tuple leaves;
it cannot shadow one of them or introduce duplicate body bindings.

The pack is one formal input identity with ordered entry paths, not a family of extra public
parameters or a generic parameter pack. Its entry count is target usize and may differ between
invocations without making another source declaration/callable type. Pack kind, component data,
operation and origin/state relations are part of the callable contract. Names identify that
formal input but do not affect structural callable identity. An ordinary pack of tuple data and
a keyed pack are different callable contracts even when their entry payload shapes agree.
Ordinary element compatibility and retained witness identities still apply. Matching callable
signatures do not authorize heterogeneous code/environment erasure or an unproved dynamic witness
join. Required witness/capability schemas must be closed independently before compiler admission.

## One Operation per Entry

An unmarked pack reads each component; edit updates each existing component; take receives each
component by ordinary value-supply. A keyed contribution has two components, key then value,
and applies the same operation to both. Keys are ordinary data, not implicitly read-only labels.
No operation is inferred from Copy, literals, a body mutation, the key type or later use.
A keyed pack is an ordered sequence of pairs, not a map. Repeated keys remain distinct entries;
there is no automatic comparison, hashing, deduplication or replacement by key.

A same-typed read component aliases supplied storage rather than snapshotting Copy data. Component
data/compatibility is checked under the selected expected input contract, including the ordinary
[contextual outcome rules](outcomes.md#contextual-construction-and-exact-forwarding). A permitted
wrapper constructs a temporary before read supply; it cannot implicitly consume a direct non-Copy
source merely to satisfy read. A direct edit component needs an initialized compatible writable
place, without snapshot/wrapper repair. An already produced component can instead lend update
through its caller-owned evaluation temporary. A take pack receives independent values: snapshots
from unmarked Copy sources, transfers from marked eligible owners, or already produced values. The
receiving take does not silently move an unmarked source. Fresh computed read/edit operands live in
the caller's evaluation frame; they do not become assignment/local-alias places. Ordinary
retention/temporary rules still apply.

Pack storage is not promised contiguous and is not a user-visible allocator request. Read/edit
components may refer to unrelated caller places. Distinct entry paths do not prove that their
actual storage or carried capabilities are disjoint. Validate the complete call bundle, including
ordinary inputs, environment, duplicate contributions and reachable access/write footprints.
Conflicting edit/transfer supply cannot be repaired by switching the pack operation.

## Binding and Body Operations

A pack name denotes its argument sequence, not an ordinary `[T]`, tuple, container descriptor or
first-class data value. These operations are available:

| Source use | Meaning |
| --- | --- |
| `items.len()` | Read the original entry count as usize, without reading element payloads |
| `items[index]` | Select an ordinary entry place, or a keyed entry's logical `(K, V)` place |
| `...items` in a variadic tail | Supply its entries under the receiving pack contract |

These are checked pack operations with recorded identities; an unrelated member named len has
no such privilege. Count and entry order are fixed at receipt. Count still exists after partial
element moves and does not count only initialized survivors. There is no pack append, resize,
whole-pack replacement, bare-name value supply, whole-pack scoped alias or explicit drop of its
name. Store/return a selected element or construct an independently contracted container when
an ordinary data value is needed. Pack capture uses the existing closure capture operation as
specified below; it does not create a source-level pack data type.

Check index with expected usize data, evaluate it once, and validate it against entry count
before selecting. Out-of-range indexing has the same checked trap/required-CTFE distinction as
fixed-array indexing. A keyed entry's `.0` and `.1` select key and value. The logical pair maps
to both original component places; it does not assert that they are physically adjacent or
disjoint. Even when K is usize, the index selects an ordinal rather than looking up that key.
Existing-place replacement stages the complete RHS, then uses ordinary tuple replacement and
old-component cleanup, with old value before old key, preserving every origin and authority.

Read/edit entries remain aliases and cannot supply a non-Copy value. Named take-pack entries are
locally writable owning subplaces. They can move or be restored under the same conditional
initialization rules as structural array/tuple components. A postfix-marked transfer, even for Copy
data, makes that entry or selected pair component unavailable; the pack name/count remains.
Uninitialized components cannot be read, aliased, supplied as values or included in a complete
re-expansion. Replacement can identify a missing component without reading it. A later
reading/accessing selection needs proof of initialized/disjoint storage, using the frozen index and
actual root mapping, not expression text or distinct logical ordinals alone.

The pack itself has no Copy evidence to inspect or clone. Component supply uses the body's
declared/contextual evidence, without gaining Copy from a particular caller. Empty packs have
no component resources, but do not grant Copy to their element data or permit a generic body
to assume every call is empty.

## Calls and Built-in Expansion

Select the call target and its complete input contract first. Supply every fixed prefix input
explicitly, once, in order. Remaining contributions go to the final pack. Zero contributions
are valid when the selected generic/data contract is established; an empty pack cannot guess
an otherwise unknown type argument. There are no default inputs or named fixed arguments.

An ordinary tail accepts ordinary expressions and `...source`. A keyed tail accepts `key: value`
and `...source`. A standalone pair expression is not an alternative direct keyed contribution;
a colon in a fixed-input position is not a parameter-name label. Mixed direct/expanded
contributions preserve written order. Do not silently group a list into one array input or
flatten an ordinary array/tuple argument without `...`.

```text
func forward(... take jobs: Job): void {
    submit(...jobs^)
}

// Separate uses with suitable declared values and writable counts:
inspect(first, second)
adjust(...counts)
fields(key: value, other_key: other_value)
```

The built-in expansion source is an existing pack or a fixed array, including a computed array
value where its category permits the receiving operation. A tuple alone, arbitrary container,
string or unclosed dynamic `[T]` is not a built-in source. This is not an implicit call to a
same-spelled iterator. Other data uses the declared [traversal protocol](iteration.md#general-argument-expansion).
A spread cannot fill fixed prefix
inputs or call a non-variadic target. Those are not runtime arity checks or overload alternatives.

An ordinary receiving pack checks each source entry/array element as one component against its
element type. A keyed receiving pack requires each source entry/array element to have exact
two-component tuple shape, checks its components against K/V under the same input rules, and
supplies key then value separately. A keyed source's
entry already has that shape. Explicit expansion can therefore adapt a sequence of pairs to a
keyed tail, or keyed entries to an ordinary tuple-valued tail; this does not equate the two
callable types or add an automatic callable adapter.

An unmarked existing expansion source lends read/edit components or supplies Copy snapshots to take.
It cannot implicitly transfer non-Copy components. `...array^` instead transfers the whole owning
array; `...pack^` transfers every owned component, key then value, including Copy components, while
retaining pack count metadata. A read/edit pack cannot surrender its caller components. These
sequence operations follow [explicit transfer sources](value-access.md#explicit-transfer-sources).
The receiving operation still determines entry/pair component preparation, without adding another
retirement of a caller source or choosing a different source operation from later use.

Evaluate a built-in expansion source once at its written contribution position, freeze its count and
entry/root mapping, then prepare entries in ascending order. Keyed entries prepare key before value.
A subsequent contribution does not run before that preparation. Unmarked Copy taking operands are
snapshotted then; direct read/edit sources and marked intrinsic transfers are reserved under the
common invocation protocol, including marked Copy data. Read expansion of a Copy array is not a
snapshot of that array.

After preparing each logical entry, validate the accumulated count with checked target-usize
addition before preparing another entry/step or evaluating a later contribution. Key/value is
one logical entry. This also supports producers whose length is known only while traversing.
Count overflow traps before invocation
commit; evaluated required CTFE reports it as a compilation error. No wrapping, new fallible
layer or arbitrary smaller language limit is introduced. Ordinary trap cleanup policy still
applies, and earlier operand effects/nested transfers are not rolled back.

A direct marked intrinsic source commits with the outer call bundle. `...array^` leaves the
entire original array unavailable, even for an empty or Copy array; its evaluation owner supplies
components and cleans the remaining carrier. `...pack^` retires all owned components, retaining
only sequence metadata in the original pack. Unmarked read/edit expansion preserves original
places; unmarked take expansion snapshots Copy components. Complete expansion requires every
contributed component initialized; len alone is not that proof. Computed array construction may
already have transferred outside values before the outer call is ready.

No source-level allocation, user callback, destructor, failure or suspension is inserted between
the final bundle validation and transfer/activation commit. Required actual backing is ready
before that commit. On recoverable exit or cancellation during preparation, release reservations
and clean completed prepared values/temporaries in reverse preparation order; do not consume
merely reserved sources.
Earlier nested moves/effects remain. Preserve outside writes performed by prepared-value cleanup.

## Origins, Cleanup and Deferred Calls

A pack name in from denotes one formal input source with all entry/component paths. Map those
paths to actual caller roots and carried dependencies at each call. Multiple actual entries do
not by themselves require additional annotations when one formal input is the eligible source;
they also do not become one shared root or justify overlapping exclusive access.

A result cannot retain direct storage in received take components that die during callee cleanup.
It may forward carried dependencies into surviving outside roots or return received values.
An edit-pack clause after its element type, or after the keyed value type, bounds every edited
component and its reachable writes, including entries the body never indexes. Read/take packs
have no post-state clause: their from, when present, bounds incoming carried provenance under
the [incoming-origin rules](memory-and-regions.md#incoming-origins), not the source descriptor's
direct read extent. Their delivered data retains its ordinary dependency schema.
Callable element types use the grammar's [origin attachment](syntactic-grammar.md#access-syntax):
group the complete type when the following clause describes the enclosing edited pack rather
than an inner callable result. Default and explicit post-state relations are the ordinary
[origin rules](memory-and-regions.md#edited-state-origins),
not a pack-specific weakening. Empty or void output is no proof of no retained outside state.

At normal/recoverable callee exit, dispose remaining take components in reverse entry order;
within a keyed pair, value precedes key. Skip moved components, preserve destructor preconditions,
and stage results before cleanup. A partially consumed pack has no whole-container destructor.
Read/edit pack release ends delegated access, not caller ownership. Failure-loss warnings and
outside cleanup/write effects apply to discarded remaining components normally.

An async producer receives take components or activates read/edit delegation at construction,
without executing its body or len/index operations. Preserve count, frozen actual mappings,
remaining initialization and required roots in its checked frame schema. Unstarted cancellation
cleans received components; pending cancellation cleans the actual remaining state. Completion
and cancellation use their ordinary distinct origin envelopes. Temporary read arguments do not
survive a later stored future merely because they entered a pack. An immediate await may keep
the valid caller evaluation frame through suspension under the same existing rule.

## Closures and Qualification

An explicit name in the value-capture segment can name a pack:

```text
func make_submitter(... take jobs: Job): once(): void {
    (jobs^;) { submit(...jobs^) }
}
```

This assumes a compatible Job/submit contract and body effects. Capture preserves entry count
and constructs each captured ordinary component, or key then value, by value-supply in order.
It requires complete supplied entries. A marked capture transfers all owned components, even
Copy data; an unmarked capture copies components only with source-context Copy evidence. A
read/edit pack cannot retire caller components by adding a caret. There is no capture-time reservation until
first invocation, hidden caller-owner retention or implicit container/allocator API call.
Earlier executed captures are not rolled back if later formation exits.

The resulting environment owns that captured sequence and its remaining-component cleanup,
including when never invoked. The original and captured packs have separate component storage
and initialization responsibility; forwarded carried dependencies remain shared roots as required.
Reading an owned capture permits func, updating requires mut and consuming requires once under
the ordinary invocation inference. Ownership of the capture alone does not imply once. Complete
environment Copy needs its structural component proof, never a grant from an observed empty
runtime pack. General capture schemas/physical backing still require qualification.

Unlisted capture inference permits read/edit or Copy value capture, without an implicit outside
move. A non-Copy value capture needs an explicit marked pack entry; a caret inside the body
cannot supply that formation-time permission. No escape use upgrades a read capture to ownership. A selected
value capture of the pack has the formation rule above; an access capture preserves its entry/
actual-root mapping and needed extent instead of constructing element owners. Source-visible
Copy evidence, conflicts, origins, destructor effects and cleanup survive both forms.

A closure can declare a final pack using `...name`, an optional edit/take, and optional component
types. An already selected expected callable contract can supply omitted pack kind/data/operation;
explicit parts must agree. A keyed annotation writes both types. Without that context an
unmarked closure pack reads, an unannotated kind is ordinary, and missing unique data needs an
annotation. Later calls cannot retroactively choose a pack kind, element type or operation.
The body can infer captures/invocation/result as for other closures.

Public contract/body pairing repeats the complete named pack header. Structural callable and
interface compatibility preserves pack kind, component data/operations, origins and phase effects
through their existing input-identity mapping; it does not flatten the pack into a concrete
caller's arity. Required const calls use the ordinary const-body domain check and the same
checked evaluator. No separate pack evaluator or C-style varargs ABI is introduced.
Source recognition, dynamic initialization/access proofs, capture schemas, actual
cleanup, evaluator and target realization all require production qualification before admission.
