# Memory, Regions, and Allocators

## Status

Nocter provides deterministic ownership, explicit fallible allocation, lexical regions, storage
provenance, and a statically propagated current allocation context for `String`, `Vec<T>`, and
`RawBuffer`. Typed literals, sequence spread, and collection iteration build on that foundation.

## Memory Model

Nocter does not use garbage collection. Its memory model combines:

- owned values and explicit moves
- readonly and readwrite borrows
- source-level non-lexical loan ranges
- deterministic drop at scope exit
- allocator-backed owned storage
- lexical allocation regions
- compile-time escape checking

Owned values are dropped when their scope ends unless ownership has moved. A storage-dependent
value may move only to a destination outlived by every storage origin it carries.

## Three Separate Concepts

Allocator, allocation context, and region are not synonyms.

- An **allocator backend** obtains and releases bytes at runtime.
- An **allocation context** selects an allocator backend, failure policy, and storage origin for an
  allocating operation.
- A **region** is a lexical child lifetime with its own allocation context and release boundary.

Borrow loans are separate again: they restrict access to source places while a borrow-like result is
live. Allocator provenance does not replace loan checking, and a runtime allocator handle is not a
source-level lifetime annotation.

## Default Allocation Context

Every executable starts with a program-lifetime allocation context backed by the standard aborting
system allocator. An allocating callable receives the current context as a compiler-propagated
capability.

The current context is not:

- a mutable process-global variable
- a thread-local lookup
- selected by searching for a standard-library name
- reconstructed by the backend from source syntax

The compiler records an execution allocation requirement for each callable and statically passes
the context only where required. Source functions infer that internal requirement from their bodies
and callees. Trusted bodyless standard-library declarations carry compiler metadata attached to
declaration identity.

Changing a helper body so that it allocates may change its compiler-owned execution requirement.
The whole compile unit contains the source and summaries required to propagate that fact. Execution
allocation and fresh storage retained by a result have no source-level annotation.

## Value Storage Contracts

Callers see only external storage relationships they must preserve:

```nct
func len(text: &str): usize
func copy(text: &str): String
func view(text: &String): &str
func copy_with(allocator: &+Allocator, text: &str): String from allocator
```

`from X` reports a receiver, parameter, allocator capability, local value, or `static` origin that
bounds a storage-bearing value. It may qualify a callable result, parameter, receiver, or annotated
local binding:

```nct
func inspect(owner: &Owner, value: &str from owner): void
method (&self from owner).inspect(owner: &Owner): void
let part: &str from text = parser.next()?
```

The clause constrains the complete value, including storage-bearing fields and elements. It is not
part of the value's data type, does not change layout or ABI, and does not declare a lifetime or
origin name. Every origin must be an existing value binding in that contract's scope. `from owner`
means that every caller-managed storage origin retained by the qualified value is contained by
`owner`; storage-independent and static values satisfy the contract without borrowing `owner`.

For callable headers, the receiver and all parameters are reserved before any clause is resolved.
A receiver may therefore name a later parameter and parameters may name one another without making
meaning depend on source order. Mutually contained values form one equality component. A direct
self-bound such as `value: &str from value` is invalid because it adds no constraint.

Result clauses are normally omitted. When exactly one receiver, parameter, allocator capability,
or argument pack can supply the result storage, that origin is inferred. Fresh storage and static
storage require no caller-managed source place and therefore require no clause.

A callable with multiple eligible inputs may omit `from` only when its body proves that the result
retains none of them. A bodyless declaration cannot supply that proof, so an ambiguous
storage-bearing result must explicitly name its retained upper bound. `copy_with` above has both an
allocator and text input, but its owned result retains only the allocator origin. Explicit clauses
remain legal for unambiguous APIs, although canonical source omits them.

For a result `T!`, the clause describes only the successful `T` value. Error storage remains part of
compiler-owned escape analysis and does not force a `from` clause onto every fallible API. Omitting
`from` does not promise allocation-free execution or storage independence from the active lexical
region. The independent `noalloc` callable contract below provides the allocation guarantee;
`realtime` is not part of the current language.

At invocation, every constrained argument is checked against the actual source values named by its
contract before the call can execute. Inside the body, each parameter retains its own exact
symbolic origin; the declared containment graph is used only when proving a destination contract.
This preserves precision when a result returns the constrained value itself.

`from` constrains construction of a new value. It cannot add an unrelated origin to an existing
mutable owner in place. A provenance-changing operation consumes the owner and returns its new
state explicitly:

```nct
method self.push(value: &str): Self from self | value
```

Named origin parameters, abstract outlives predicates, variant-dependent public provenance, and
movable direct self-references are not part of the language.

Body-backed summaries infer fresh storage and exact path-sensitive origins from implementations.
Bodyless abstract callables use their written clause or the shared zero/one-origin elision rule.
Trusted allocation and process-lifetime primitives use semantic roles attached to declaration
identity. None of these internal facts causes formatter or editor signatures to synthesize source
syntax that the author did not write.

An explicit result clause on an associated type remains authoritative through generic dispatch.
For example, `Self.Item from self` retains the invocation's receiver loan even while `Self.Item` is
abstract. Specializing an unrelated generic result `T` to a borrow-carrying concrete type does not
acquire that temporary receiver loan; it carries only the loans already stored in the input value.

## Allocation Failure Policies

The standard library provides aborting and recoverable allocation capabilities over one
implementation. Their exact declarations, termination behavior, error codes, and failure atomicity
belong to [Allocation and Failure](../../std/mem/README.md).

The ambient allocation context selects the ordinary aborting capability. Recoverable allocation
requires an explicit library API whose declared result is fallible; changing the current context
alone never changes an expression between `T` and `T!`.

## Standard-Library Boundary

Allocator APIs live in `std/mem`. `Allocator`, `TryAllocator`, `Layout`, and `RawBuffer` are ordinary
standard-library names, not compiler built-ins.

The compiler's special behavior is limited to:

- allocation-effect and storage-origin metadata on trusted declarations
- statically propagating the current allocation context
- the `region name using allocator_place { ... }` language construct
- escape checking and ordered region cleanup

The exact declarations, layout validation, raw-buffer invariants, and aborting-versus-recoverable
adapter behavior belong to the compiler-checked
[`std/mem` contract](../../std/mem/index.nct) and its
[Allocation and Failure](../../std/mem/README.md) guide.

## Lexical Regions

Syntax:

```text
region name using allocator_place {
    statements
}
```

Example:

```nct
region scratch using arena {
    let source = read_file("main.nct")?
    let tokens = lex(&source as &str)?
    consume(&tokens as &[Token])
}
```

`allocator_place` must resolve to an established aborting allocator or allocation-context place. It
is not an arbitrary effectful expression. The parent is evaluated and validated before the child
region is entered.

Entering the statement:

1. creates a fresh lexical region identity
2. derives a child runtime allocator from the selected parent
3. binds the immutable region handle to `name`
4. makes the child allocation context current for allocating calls in the body

The child is a non-movable compiler-owned resource. It retains the selected parent's allocator
header, owns an independent allocation-list head, and presents the ordinary allocation-context
header to calls. The lexical current context is selected statically at each call and destruction
boundary; entering a region does not mutate a process-global or function-local ambient pointer.

The region name is an ordinary lexical binding name for lookup and diagnostics. The compiler does
not infer semantics from spellings such as `scratch`, `temp`, or `arena`.

Rules:

- The region handle exists only inside the body.
- It cannot be reassigned, moved out, returned, or explicitly dropped by user code.
- Allocator capabilities derived from the handle carry the same region origin.
- Owned storage allocated through the current child context carries the region origin.
- Borrows, views, iterators, raw pointers, and aggregates derived from that storage preserve the
  origin.
- A value can leave only when every component is proven independent of the child region.
- Pure integers, booleans, and copy aggregates containing only independent fields may leave.
- Unknown provenance cannot escape.
- A nested child is shorter than its parent and may receive parent-derived values.
- A child-derived value cannot flow into its parent or an unrelated region.

At every normal exiting edge, live values owned inside the body are dropped in reverse ownership
order before the child allocator releases its storage. This applies to fallthrough, `return`,
`break`, `continue`, and `?` propagation.

On `arm64-darwin`, each nonempty region allocation owns a mapping whose private prefix retains the
previous mapping and the complete mapping byte count. Release walks that list and unmaps every
entry. A failed unmap is a compiler-runtime invariant failure and terminates instead of continuing
with a partially released region.

Calling a `never` function does not cause implicit cleanup. Allocation failure on the standard path
terminates immediately without region release; the operating system reclaims process resources.

## Escape Examples

Invalid owned escape:

```nct
func load_text(allocator: &+Allocator): String {
    region scratch using allocator {
        let text = String.copy("temporary")
        return move text // error: text storage belongs to scratch
    }
}
```

Invalid indirect escape:

```nct
struct ResultView {
    text: &str
}

func load_view(allocator: &+Allocator): ResultView {
    region scratch using allocator {
        let text = String.copy("temporary")
        return ResultView { text: &text as &str }
        // error: ResultView carries a view into scratch
    }
}
```

Valid independent result:

```nct
func count_bytes(allocator: &+Allocator): usize {
    region scratch using allocator {
        let text = String.copy("temporary")
        return text.len()
    }
}
```

## Borrow Origins and Elision

Nocter does not expose Rust-style named lifetime parameters. The compiler tracks storage origins
through values, value-position `from` contracts, and callable summaries.

Elision and inference rules:

- A borrow-like result with one borrow-like input is tied to that input.
- A method result may be tied to its borrowed receiver when that is its only declared origin.
- A concrete body can establish that a result is fresh or static, or retains none of several
  otherwise eligible inputs.
- A result with multiple possible inputs is constrained by all of them at the caller.
- A trusted bodyless declaration must provide compiler-owned origin metadata.
- An untrusted bodyless declaration with an ambiguous borrow-like result is invalid.

A borrow returned through a call remains a loan of the original caller place through the returned
value's last source-level use. Return validation and ordinary NLL use the same callable provenance
summary.

Named lifetime or origin parameters may be reconsidered only when public APIs require abstract
relationships that cannot be expressed by constraints between existing values.

### Explicit value provenance

An identity-based `from` clause expresses a public value relationship without adding lifetime
names:

```nct
pub method &self.get(key: &K): &V? from self
func choose<T>(left: &T, right: &T, first: bool): &T from left | right
```

An identifier after `from` names a receiver, ordinary parameter, typed argument pack, or visible
local whose semantic value can carry storage provenance. This includes borrows, owning values,
generic values, and allocator capabilities. A pack origin represents the storage carried by its
elements; the ephemeral pack container itself still cannot escape the callable body.
`static` denotes program-lifetime storage. Source-level `from current` is not valid; fresh ambient
result storage is compiler-owned and therefore needs no public origin name. Concrete public bodies
are checked against the explicit or elided origin set; bodyless interface methods use the same
value-provenance rules. Origin identity follows resolved values rather than their formatted names.
`static` remains accepted in explicit source, but canonical APIs omit it because callers preserve
no source place for program-lifetime storage.

Value provenance applies both to source-level borrows and to pointer-backed owning aggregates.
Raw pointers remain outside borrow checking, but an owning `String`, `Vec<T>`, or user-defined
buffer still carries the allocation context responsible for its storage.

The execution allocation-context requirement remains compiler-owned and inferred. It is distinct
from the source-visible `noalloc` guarantee: a callable may need the current context only to destroy
existing storage without allocating new storage.

## No-allocation Callable Contracts

`noalloc` is an optional callable guarantee. It asserts that no execution path of the callable can
request new storage from a Nocter allocator:

```nct
pub noalloc func byte_count(text: &str): usize {
    return text.len()
}

instance Text {
    pub noalloc method &self.byte_count(): usize
}

noalloc drop Handle(&+self) {
    release_without_allocation(self)
}
```

An unqualified callable does not promise that allocation occurs. It simply exports no
allocation-free guarantee. Removing an allocation from an unqualified implementation does not
change its source contract; adding an allocation to a `noalloc` implementation is a contract
error.

The guarantee covers ordinary current-context allocation, explicit `Allocator` allocation, and
recoverable `TryAllocator` allocation. Recoverable failure does not make an allocation operation
allocation-free. It also covers allocation reached transitively through direct calls, selected
methods, operators, coercions, typed literals, callbacks, closure bodies, interface defaults,
implicit destruction, and compiler-registered primitives.

The following operations do not by themselves violate `noalloc`:

- stack-frame and local-value storage selected by the target ABI;
- moving, borrowing, reading, or mutating already existing storage;
- releasing existing storage without requesting replacement storage;
- a target or operating-system operation that does not request storage through a Nocter allocator;
- returning an owning value whose existing storage is moved from an input;
- returning a static view or a storage-free value.

`noalloc` therefore does not imply absence of deallocation, system calls, blocking, traps,
recoverable failures, external side effects, or a hidden allocation-context ABI lane. Those facts
must not be inferred from this guarantee.

The `noalloc` guarantee is transitive over every operation reachable through checked control flow.
A recursive call group is allocation-free only when all of its direct operations and calls leaving
the group are allocation-free. An unknown bodyless callable without a `noalloc` contract may
allocate. Target optimization and dead-code elimination do not strengthen the source guarantee.

A `noalloc` body may call an unqualified source-backed helper when the complete checked program
proves that helper allocation-free. An abstract interface method, bodyless primitive, or callable
value has no implementation proof at that boundary and must carry `noalloc` explicitly before a
`noalloc` caller can rely on it. A primitive may declare the guarantee only when its closed
compiler registry entry certifies the same effect.

Callable types carry the guarantee before their invocation capability:

```nct
noalloc func(i32): bool
noalloc &func(&T): bool
noalloc &+func(&T): bool
```

A proven `noalloc` closure or callable is compatible with an otherwise identical unqualified
callable contract; this erases the guarantee. The reverse conversion is invalid. Once erased by an
unqualified callable binding, a later use cannot reconstruct the guarantee from a hidden witness.
Generic code that invokes a callback from a `noalloc` body therefore requires a `noalloc` callable
parameter contract.

An interface method marked `noalloc` may be implemented only by an inherent method carrying the
same guarantee. A `noalloc default method` is checked against its body and may rely only on
allocation-free operations. Public contract/private body matching requires the modifier on both
declarations; neither side may silently strengthen or weaken the other.

Implicit destruction participates in the same proof. A generic `noalloc` body may destroy a type
parameter only when its requirements establish allocation-free destruction, including the trivial
case of `copy T`, or when ownership leaves the body without destruction. The current language does
not add a separate authored drop-effect requirement; an otherwise unknown generic destructor is
conservatively treated as possibly allocating.

Result provenance remains independent. A `noalloc` callable may return caller-owned storage, and an
allocating callable may return a result derived only from an input:

```nct
noalloc func identity(value: String): String { return move value }
func view_after_work(text: &String): &str from text
```

## Typed Literal Allocation

Typed literals use the same allocation boundary:

```nct
let values = Vec [1, 2, 3]
let values = Vec [1, 2, 3] using arena

region temp using arena {
    let text = String "hello"
}
```

- Omitting `using` selects the current aborting allocation context.
- `using arena` selects an established aborting allocator/context for that literal.
- A region body changes the current context lexically and transitively for allocating callees.
- A literal allocated in a lexical region carries that region origin.
- Recoverable allocation uses named `try_*` construction rather than changing a literal's result
  type according to allocator policy.
- Bare `"hello"` remains a static `&str` and performs no allocation.

The full literal and sequence rules live in
[Argument Packs, Literal Definitions, and Sequence Spread](literals-and-packs.md).

## Current Non-goals

- aggregate spread or embedding
- ambient recoverable allocation contexts
- fallible `region` statements
- source-level lifetime parameters
- dynamic allocator interface dispatch or arbitrary user allocator plugins
- concurrency or thread-local context semantics
- a native backend other than `arm64-darwin`

## Future Direction: v2.0.0 Value Access Origins

The [Value Access Model](value-access.md) separates access authority from carried origins.
`from` is an upper bound over resolved input/environment identities, not a lifetime name,
ownership grant, or reason to postpone destruction. Resolve the complete input set before these
relations. A source identifies both eligible direct storage and carried dependency slots; the
checked relation preserves their distinction and access strength. A name alone does not authorize
reading/updating every part of that source.

Ordinary results deliver values. Independent values need no caller-place origin. A capability or
dependent result needs a bound for each externally dependent slot. One eligible source may be
elided; multiple sources need a stable explicit public bound, such as `from first | second`.
Eligibility follows the public type/dependency schema, not arbitrary matching type names or a
private method body. Index projections additionally declare a direct-place result protocol,
independently of the element data type. Ordinary scalar value results cannot acquire outside
access merely because they have inputs.
`static` admits program-lifetime origins; an independent value also satisfies that upper bound.
It is not a promise about the physical address or allocation strategy of independent owned data.

Owned locals, evaluation temporaries, consumed inputs and consumed environment storage that end
on return cannot be escaping direct roots. Dependencies they already carry into genuinely
surviving outside storage can be forwarded. Copy removes access to the source descriptor slot,
never carried dependencies/storage requirements. Returning a view into a taken caller-Copy
buffer is still invalid if the callee's separate buffer dies; no retargeting to the caller occurs.
Local guards that revoke access are roots even when the raw backing bytes survive.

### Edited-State Origins

A clause on an edit input bounds its complete post-call dependency state, on normal and outward
recoverable exits. For example:

```text
func remember(edit holder: Holder from buffer, buffer: Buffer): void
func append_view(edit holder: Holder from holder | buffer, buffer: Buffer): void
func clear(edit holder: Holder from static): void
```

`remember` permits only buffer-derived outside dependencies in the resulting holder; if it also
retains old holder-derived dependencies, it needs `holder | buffer`. `clear` promises no
caller-dependent origins afterward, not merely no result. Such a promise must hold on failure
and cancellation too. A body cannot silently weaken it. The clause describes the edited value's
post-state, not a requirement that the incoming holder already has those origins.

The complete relation includes mutated referents reachable through an edited value's capability
slots, not merely the value's descriptor fields. A captured update target has its own post-state
and surviving owner identity. Applying an abstract callback/guard relation updates that state in
the caller's dependency graph. Dropping the callback/guard releases only its access/owned resources;
it cannot erase dependencies newly stored in an outside owner. Preserve this checked write
footprint and slot mapping through generic forwarding, interface matching, erasure and adapters.
A shared-state API that mutates through a safe protocol must supply the same evidence; read access
to its handle is not proof of an empty write footprint.

Without an explicit edit clause, preserve an upper bound consisting of its incoming dependency
slots and compatible dependencies already carried by supplied input values. The latter supports
ordinary insertion of a delivered dependent value into a container; actual Copy/transfer must
still be authorized. It does not admit creating a new view into another input's direct storage.
Such retention requires an explicit origin clause. Do not infer a direct-retention promise from
an inaccessible body or from the caller's long-lived argument.

Contracts distinguish carried read/update identities, not just root sets. Receiving an exclusive
capability cannot make its source retain a usable duplicate, and copying a read descriptor does
not shorten its referent requirement. The caller applies the post-state relation on every
recoverable successor, including `void` calls and failures after partial mutation. Conservatively
retain permitted alternatives until dependency-free state is established by a sufficient contract.
Private bodies may prove a narrower internal transition, but cannot narrow a bodyless caller's
public obligations by hidden lookup. An explicit bound can remove old dependencies only when
body verification proves that result on every affected exit.

The same rule describes an updated callable environment. `mut from input (input: Buffer): void`
bounds its post-call state by input; `mut from self | input (...)` also permits previous environment
dependencies. Without that clause the default above admits carried input dependencies, but not
new direct-input views. A `func` environment cannot be edited through ordinary read invocation;
a `once` environment does not survive invocation. Neither gets a mutating-environment clause.
A method receiver with a clause uses the existing parenthesized receiver shape, for example
`method (edit self from self | buffer).remember(buffer: Buffer): void`.

Result origins and edited-state origins are separate relations, even if both name the same input.
Callable `self` means its environment; method `self` means its receiver. Each invocation introduces
fresh symbolic extents for its inputs. Generic/interface/erased contracts preserve these relations
and their quantification instead of assuming one persistent callback-input lifetime.

Wrapping a dependent value in an aggregate/outcome or erasing a callable cannot hide its schema.
Region/storage requirements remain separate from direct access; an allocator dependency is not
an exclusive borrow of the allocator. No origin rule permits hidden cloning/sharing, automatic
region extension, destructor postponement, or resource-lifetime repair. Full library schema and
platform realization remain separate migration obligations.

## Future Direction: v2.0.0 Storage Optimization

These future rules refine v2.0.0 representation freedom. They do not change the released
allocation/region rules above or the current checked standard-library declarations.

### Logical Obligations and Physical Backing

Keep the value's initialization, access roots, carried dependencies, semantic allocator/storage
origins, cleanup, and outcomes separate from its physical placement. Registers, stack slots,
frame fields, or heap blocks are implementation choices only to the extent the owning contract
permits them. Removing backing does not remove a declared origin, allocator affinity, retained
access, or user destructor. In particular, a region-dependent value cannot escape merely because
its current bytes disappeared.

A checked implementation-storage certificate may allow eliding, scalarizing, or reusing backing.
The certificate must preserve the selected language/library contract and relevant effects. A fresh
owned result, a factory's name, or absence of authored modifiers grants no such permission by
itself. Calls with unknown effects retain conservative obligations. No new source attribute,
execution-effect modifier, allocation primitive name, or caller annotation expresses this certificate.

### Allocation Observations

Automatic frame/environment/transport storage does not promise one heap request or the allocation
failure of a particular heap-backed plan. When a valid representation removes an optional private
request, it may also remove termination attributable solely to that request. Do not insert an
allocation probe to preserve a failure that the abstract storage contract did not require.

This permission does not erase exposed resource operations. Preserve recoverable errors, required
failure boundaries, allocator callbacks, budget/accounting effects, capacity promises, backend
identity, supported addresses/ABI, and resource-completion order wherever their contract exposes
them. A known inert path may discharge an obligation; neither `Allocator` nor `TryAllocator` is a
name-based optimization rule. An unused buffer alone does not prove that its request is inert.
Ordinary validation, checked arithmetic, input evaluation, and non-storage errors remain required
even when no backing is materialized.

### Materialization and Reuse

Discharge backing obligations by the earliest contract deadline: before required capacity is
published, a concrete representation is exposed, an opaque consumer needs it, or ownership commits
to an operation requiring that backing. Alternatively prove that the value remains virtual on all
relevant paths and still satisfies the same contract. Do not defer a necessary allocation or wait
into a reserved append, getter, future drive, cancellation, or destructor that does not allow it.
Actual required frame/environment storage and validation precede invocation-time X/U commit.

Reuse requires compatible capacity, alignment, allocator/domain obligations, and no future use of
the overwritten bytes through access, captured dependencies, foreign addresses, or cleanup.
Retain user destruction at its defined semantic point. A plan may retain needed cleanup data
separately; it must not skip destruction or present corrupted/uninitialized fields. Preserve
logical capacity, initialization, failure atomicity, and allocator affinity even if physical
storage is removed or shared between nonoverlapping backing lifetimes.

Revalidate physical-dependency and actual resource-effect obligations after placement changes.
An earlier proof of asynchronous drive safety cannot justify a newly introduced waiting backend
operation. Unknown demand or missing proof selects a valid conservative representation; it does
not introduce GC, cloning, lifetime extension, or unchecked execution.

The standard-library obligations remain owned by
[Allocation and Failure](../../std/mem/README.md#future-direction-v2-0-0-storage-optimization) and
[Vectors](../../std/vec/README.md#future-direction-v2-0-0-storage-optimization). Full interprocedural
certificates, compound-value, cleanup, and platform validation remain required before implementation.
