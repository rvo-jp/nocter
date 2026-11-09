# Memory, Regions, and Allocators

## Value Access Origins

The [Value Access Model](value-access.md) separates access authority from carried origins.
`from` is an upper bound over resolved input/environment identities, not a lifetime name,
ownership grant, or reason to postpone destruction. Resolve the complete input set before these
relations. A source identifies both eligible direct storage and carried dependency slots; the
checked relation preserves their distinction and access strength. A name alone does not authorize
reading/updating every part of that source.

[Tuple header bindings](patterns.md#header-origins-and-correspondence) can name input projections.
Resolve those names to one input identity plus component paths. Elision can conservatively bound
one eligible input subtree without an authored whole-input name; it never merges slot identities
or narrows an edit postcondition's coverage to the leaves actually used.

A [variadic input](variadic-inputs.md#origins-cleanup-and-deferred-calls) likewise denotes one
formal subtree with entry/component paths mapped to the actual roots of each call. A pack's
many actual arguments do not introduce many authored lifetime names or merge their identities.
An edit-pack bound covers every component and reachable write, not only indexed entries.

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
as well as successful completion. Deferred cancellation uses the separate envelope below; it
cannot pretend that an unstarted body completed. A body cannot silently weaken its completion
promise. The clause describes the edited value's post-state, not a requirement that the incoming
holder already has those origins.

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

### Deferred Completion and Cancellation Bounds

For a deferred invocation, an authored edit bound B applies when its body completes normally or
with recoverable failure, after that path's producer cleanup. Entry denotes the edited target's
incoming dependency state at invocation commit. Cancelling an unstarted/pending producer instead
permits Entry union B after cancellation cleanup. This is an upper bound, not a copied value,
rollback, restored dependency or automatic lifetime extension. Keep dependency-slot identities
and read/update strengths, not just an untyped union of names.

The same rule applies to each edited outside owner in the producer's checked write footprint.
It does not replace the cleanup transitions of received values/captured capabilities. Compose
those checked schema relations too, including before first drive; every resulting state of an
explicitly bounded target must still satisfy its envelope. Other reachable owners retain their
own checked transitions rather than acquiring an empty effect from the absence of a named
edit input.
Every actual cancellable/suspension state must establish that envelope after its ordinary
cancellation cleanup. A new direct-input view must still fit B; Entry adds only already incoming
dependencies, not every available input. Access into destroyed frame/child/guard storage is
invalid even if the raw bytes survive. A source-authored cleanup guard may establish the bound;
the compiler cannot insert state reset, rollback or eager body execution as a repair.

With no explicit edit clause, B already preserves Entry and compatible carried input dependencies
under the default above, so cancellation adds no new direct-retention permission. With
`from static`, completion removes caller-dependent origins while unstarted cancellation can
leave the original ones. A caller that awaited completion and a caller that cancelled therefore
have different state evidence. No new cancellation qualifier or hidden incoming precondition
is required. Public bodies cannot silently narrow that conservative cancellation bound.

Ready-output disposal additionally composes the completed B state with the output's own checked
cleanup/write footprint. A guard-like output may later update an outside owner; its schema cannot
be erased by wrapping it in future T. The whole future cancellation relation includes both
pending-producer cancellation and completion followed by unconsumed-output disposal. Likewise,
after await the completion bound does not erase capabilities/dependencies transferred into the
live output. Missing result-cleanup evidence is an admission gap, not an empty footprint.

Immediate callables returning futures remain immediate: apply their own post-state bound on
return, and preserve the returned future's separate transition schema. A mutable callback's
immediate environment bound is not converted into a deferred one merely because its result is
future T. This phase rule replaces the earlier same-bound-on-all-cancellations model; complete
initialization and outside-root survival still hold on every exit.

## Storage Optimization

These future rules refine v2.0.0 representation freedom. They do not change the released
allocation/region contract or the current checked standard-library declarations.

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
[Allocation and Failure](../library/memory.md#storage-optimization) and
[Vectors](../library/vectors.md#storage-optimization). Full interprocedural
certificates, compound-value, cleanup, and platform validation remain required before implementation.
