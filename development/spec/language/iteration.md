# Iteration and General Expansion

## Scope and Operations

This chapter owns synchronous for loops and user-defined argument expansion. The
[traversal library contract](../library/iteration.md) owns exact interface declarations;
[value access](value-access.md) owns preparation, dependencies and cleanup;
[variadic inputs](variadic-inputs.md) own receiving pack operations. These adopted v2.0.0
contracts do not extend the bounded private compiler or claim a traversal implementation.
Async/fallible traversal, task protocols, typed literal entries, range-library APIs and target
realization require their own closure. Released syntax/iteration is not a fallback.

```text
for item in items { inspect(item) }
for edit item in items { item.update() }
for take item in items^ { submit(item^) }
```

These are separate schematic uses under independently declared data/helpers. Unmarked for binds
read access to each existing item; edit binds update access; take receives each delivered value.
One operation applies to the complete common name/tuple/underscore pattern. There is no explicit
read prefix, body-inferred mode, per-leaf mode or alternate colon-based keyed pattern. A keyed
pack or pair-valued source uses `(key, value)` under the common pattern rules.

The mode is selected before checking the source/body. It selects the read, edit or take expansion
operator family for ordinary source data, with the corresponding cursor protocol. ReadSource,
EditSource and TakeSource bundle those operator/result requirements for generic use. The declared
receiver operation is read, edit or take respectively. Missing authority/evidence is an error,
not a search for a weaker source or a Copy-dependent change of loop mode. Fixed arrays and packs use their selected structural
source evidence rather than a lookalike method or unrelated iterator.

## Source Acquisition

Evaluate the source expression once. An existing read source is observed in place. A direct
edit source needs initialized writable storage. A produced source value can instead lend
update access from its loop-owned evaluation temporary under ordinary call preparation;
it does not become a local-alias/assignment place or a hidden copy of an immutable source.
A take source uses ordinary value supply: an unmarked existing source requires Copy, otherwise
an eligible owner requires a postfix caret. A fresh source needs no caret. An explicitly marked
source can also lend read/edit access after acquisition into the loop evaluation owner.
The selected source operator is an ordinary immediate value-producing invocation, returning
one owned cursor. Store it in the loop's mutable control frame; advancing that frame does not
grant mutation of a source originally received for read.

Source construction, operator calls and their operand effects occur before the first step,
even for an empty traversal. A source operator can have ordinary trap/divergence effects; its
plain cursor result has no implicit failure or future layer. Calls made while acquiring it
keep their normal input commit points. A source carrier's own temporary stays in the enclosing
loop evaluation frame until cursor/body cleanup completes, without permitting an escaping
dependency on that temporary.

A fixed array's read/edit cursor accesses its original element places in ascending index order. A
take array loop acquires the complete array value before the first iteration, snapshotting a Copy
array from an unmarked source or transferring a marked owning array, even if Copy. Its cursor owns
the remaining initialized array and delivers elements in index order. An early break disposes unused
elements; it does not restore the original array. Empty arrays still perform acquisition and retain
their type's ordinary Copy proof. Zero-byte elements remain distinct logical items.

A pack read/edit cursor uses its fixed count and actual entry/component map under the available
input authority. A take loop acquires count and component values using the same operation as
[pack value capture](variadic-inputs.md#closures-and-qualification): ordinary entries, or key
then value, copy with evidence for an unmarked source or transfer every owned component for
a marked source before the first step. Its cursor owns the received
sequence. The original pack count remains; transferred components become unavailable. A
complete acquisition needs initialized contributed entries and sufficient ownership/Copy,
without granting ownership through a read/edit pack. No first-class dynamic array is invented.

Array/pack identities, projected ranges and owned remainder follow one checked traversal plan.
Intrinsic cursor layout is not source type identity, a second dispatch rule or permission to
infer initialization again in lowering. Inference must preserve source-context Copy choices.
On a successful intrinsic value step, stage the selected Item under the ordinary value rule
and retire that visited component from the cursor's remaining sequence. If delivery snapshots
a Copy component, clean/end the original visited slot after staging that independent output,
not the output itself. It is not another item to emit or destroy at final cursor disposal.
Physical retained bits do not create ownership.

## Step, Binding and Exit Schedule

For read/edit, call the selected cursor's advance operation once at the top of each step. False
ends the loop without a projection or body. True requests exactly one read/edit current-place
projection, then forms the pattern's scoped aliases. For take, call next once; absence ends,
presence supplies an Item value and forms the ordinary received-value pattern. It does not
silently lend a non-Copy value because the body only reads it.

Value bindings are locally writable as take inputs are; read/edit leaves retain their declared
alias operation. Names are fresh for each iteration. Pattern formation, including tuple
decomposition and ignored owned remainder cleanup before body entry, uses the
[one pattern protocol](patterns.md#binding-formation-and-cleanup). A named unused owner instead
receives ordinary end-of-iteration cleanup. `_` skips a binding, not source evaluation or
step/projection effects; for edit still needs update authority.

The body remains structurally checked against the known Item shape even when a traversal is
proven empty. Actual bindings, supply/access and cleanup exist only on produced-item paths;
do not fabricate an Item, Copy/witness evidence or runtime root for a false/absent step.

The body normally produces unit. On normal completion or continue, clean iteration locals and
remaining owning bindings before the next advance/next call. A read/edit alias ends according
to its last demand and dependent cleanup, not a forced whole-arm lifetime. An access retained
by a surviving value can nevertheless prevent advancement if it needs cursor storage, a
revoking guard or invalidated backing. Stable outside backing can survive only under the
checked origin/capability relation; do not insert copies, extend roots or delay a destructor.

Break cleans the iteration and cursor without an extra step. Return/throw stages its answer,
cleans iteration state, then the cursor and source evaluation frame, and hands off. Reject an
answer depending on that destroyed storage or released guard. A transferred answer keeps its
own dependencies/cleanup. Recoverable exits and cancellation dispose exactly the actual
remaining state. Terminal traps/divergence do not acquire unwind guarantees from this schedule.
Break/continue target the nearest enclosing loop, without new labels or values.

An async function may suspend inside an ordinary synchronous loop body. Preserve cursor,
current bindings, actual initialized remainder, guards and outside-state relations through
that suspension. The ordinary effect contract still checks every acquisition/step/projection/
cleanup during drive. This is not for-await or an implicit worker/executor.

Cursor cleanup precedes cleanup of the source temporary that backs it. Take-cursor disposal
releases unused received values exactly once under its checked lifecycle; intrinsic
arrays/packs use reverse remaining index/entry order, with keyed value before key. Cleanup
does not require exhausting a producer or calling next/advance for unvisited items.

## Current-Item Places

`cursor[]` is a joint, zero-input current-item projection. Its read/edit operators use the same
existing-place, origin, cleanup and authority contract as indexed projections, without a dummy
unit/index argument. `cursor[()]` remains a different one-input indexed operation; there is no
automatic alias between the forms. Ordinary methods continue returning values.

A current projection may designate cursor-owned storage or outside backing. Its checked schema
records which roots and invalidations apply. Advancing/cleaning the cursor cannot revoke a place
still needed by a body/result/cleanup. A protocol name or true advance result alone cannot
remove an implementation's validity checks, assume physical stability or manufacture a place.
This is one immediate projection invocation, without yield/coroutine return syntax or a
termination promise.

The pattern's source category is selected once. Grouping does not turn a computed item value
into an existing edit place, and a failed update projection cannot fall back to read. General
capability/root/write schemas and actual source/operator proofs remain admission obligations.

Finer root/advance facts used across a module boundary belong to the cursor's checked public
capability schema, verified once with its definition and exported under that declaration.
A broad from self alone does not promise stable outside storage. Callers must not inspect a
private body to narrow it. Generic traversal freezes its operations and symbolic root/write/
cleanup obligations under declared capabilities; actual schema/witness binding discharges those
public conditions without retyping or selecting another operation. Missing proof conservatively
rejects that use. It cannot infer a missing Copy/interface/ownership capability from a caller.

## General Argument Expansion

For `...source` in a final variadic tail, the receiving pack's operation selects the matching
source protocol before checking authority or Copy. Existing built-in pack/fixed-array expansion
keeps its direct component-preparation semantics under the variadic owner. Other ordinary data
needs an explicit matching expansion operator and cursor-protocol conformance; there is no
method-name duck typing or automatic container erasure. A custom source cannot redefine an intrinsic carrier's family by lookalike
members. Spread still does not fill fixed inputs or become a standalone expression.

At its written contribution position, evaluate/acquire the user source once and repeatedly
advance/project or next it. Each entry is prepared under the receiving pack contract before
requesting the next. Preserve keyed pair validation and key-before-value preparation. A value
returned by a taking iterator is already produced: it belongs to the prepared prefix, rather
than being a merely reserved caller place. A taking user source operator can already have
consumed its receiver before the outer call commits. Later failure does not undo that inner
invocation. Direct marked intrinsic whole-array/pack transfers instead remain reserved until the outer
call's commit, including Copy sources; unmarked take components snapshot with Copy evidence.
The variadic preparation contract records these producer categories, without alternate plans
selected from later use.

A fresh tuple Item for a keyed taking tail prepares computed component values, not aliases to
an invented caller tuple place. Preserve the tuple carrier's actual remaining temporary cleanup
and carried dependencies under the ordinary computed-aggregate rule. A stored evaluation slot
alone cannot change a produced value into a user-visible edit place or reserved caller owner.

Borrowed entries must coexist until outer invocation and any dependent result no longer needs
them. Check every later step and final exhaustion against earlier reservations/roots. A cursor
that overwrites one internal buffer may support sequential for but cannot spread simultaneous
read/edit arguments from that buffer. Do not silently snapshot Copy items to make it work.
Checked stable outside backing may permit accumulation; logical ordinals alone do not prove
disjoint actual storage.

After exhaustion, retain required source/cursor temporaries in the outer evaluation frame,
following ordinary prepared-operand cleanup and origin rules. A result/future cannot escape a
temporary guard/cursor root merely because its entries were spread. On recoverable preparation
exit or cancellation, clean prepared values in reverse preparation order and then their live
producer frames; release reservations without consuming uncommitted direct sources. Cursor
destruction may update outside state; compose that checked transition too.

Each prepared logical entry contributes to the checked target-usize count before the next
entry/step or later contribution. Overflow traps before outer invocation; required evaluated
CTFE reports an error. No streaming outer invocation, callback fusion or body execution occurs
before enumeration completes. A nonterminating producer can therefore prevent the outer call
from ever running. Capacity/count optimization must preserve step order, first failure, cleanup
and all user-visible resource/backing requests.

## Source Forms and Qualification

There is one for pattern family and an ordinary expression source. Range generation belongs
to an ordinary library source API; the earlier draft's special `low..<high` for-source and
`key: value` loop binder are not adopted. This chapter selects no range function name/export.
Use a declared pair-valued source with `(key, value)`, without implicit map-key mutation rights.
A map need not expose update traversal of its keys; value/entry APIs keep their own invariants.

Read/edit/take expansion operators are distinct fixed identities. Their complete public/body
headers repeat under the module rules, and supported interface requirements bind them by family
and associated cursor type. Source predicates preserve receiver operation and result identity.
No operator introduces a new opaque-result declaration or weaker input/effect contract.

For-await, fallible pulls, integer/range APIs and arbitrary capability erasure remain separate
closure tasks. [Typed literal construction](literal-construction.md) owns its callable entries.
Const traversal may use only the same checked
operations within the selected evaluator domain; source syntax supplies no missing const
capability. Parser/flow/root/capture/cancellation/evaluator/ABI/native qualification is still
required before compiler admission. No speedup follows from these contracts alone.

User expansion/current heads may advertise const under the
[shared staging contract](generics-and-constants.md#const-calls-and-temporary-evaluation).
Intrinsic array/pack traversal likewise needs checked evaluator evidence; a pure-looking user
traversal is not automatically admitted. Conditional generic const evidence remains explicit
closure work. Optional runtime folding needs its ordinary semantic proof and does not
manufacture a public const promise.
