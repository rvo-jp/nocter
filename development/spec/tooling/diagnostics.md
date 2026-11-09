# Diagnostics

## Core Syntax Diagnostics

The future [value/alias grammar](../language/syntactic-grammar.md#access-syntax)
replaces the pre-S12 prefix-based checkpoint. Ordinary parser failures use `E0120` and identify
the expected production. There is no surviving direct-marker assertion grammar or unary `&&`
subdivision. Retired prefix spelling is not an automatic repair opportunity: replacing a prefix
with a binding/input operation requires knowing its intended behavior.

Retain non-place alias initializers/assignment targets in syntax for semantic diagnostics. A
contextual `read`/`edit`/`take` spelling is recognized from grammar position, never type lookup.
The [earlier checkpoint snapshot](../../history/reviews/v2.0.0-access-checkpoint-contracts.md)
preserves retired diagnostic requirements. The
[bounded implementation record](../../history/reviews/v2.0.0-value-binding-front-end.md)
identifies the private source/checking scope. These rules do not change released code assignments
or diagnostic behavior.

## Private Header Diagnostics

Resolve all headers before bodies. Duplicate function names use `E0241`, duplicate/shadowing
input names use `E0341`, and reserved function names use `E0240`, retaining the existing related
first-declaration locations. Private record-name and field-name collisions use `E0241` with the
first conflicting declaration related. Unknown header data names use `E0290`; a known type outside
the closed fragment, non-finite by-value containment or an invalid `copy struct` request uses
`E0291`. Focus the authored type or field type; relate the referenced record when explaining a
containment cycle. These rules do not permit overload selection by input operation,
body or declaration order. Header/body mismatch reports both declarations and differing modes,
types and origin/state relations. An impossible source/product pairing remains an integrity
failure, never a fabricated source name error.

## Core Body Typing Diagnostics

Use source operations in messages, rather than old I/U/X-qualified local types. Distinguish:

| Existing code family | Future failed obligation |
| --- | --- |
| `E0340`, `E0341` | Name resolution or conflicting binding |
| `E0370` | Data/type/result mismatch, incompatible callable witness/annotation, unknown field/body data type, invalid/missing/duplicate record initializer, or insufficient local/result type information |
| `E0371`, `E0377` | Value construction has neither Copy evidence nor an eligible owning source |
| `E0372`, `E0373` | Non-unit intermediate expression or missing normal result |
| `E0375` | Integer outside the selected data type's range |
| `E0382` | Loop exit outside a loop |
| `E0383` | Explicit drop does not name an owning value binding |
| `E0384`, `E0386` | Non-writable assignment target or invalid numeric compound target |
| `E0387` | Alias creation/update input lacks the required initialized place/authority |
| `E0390` | Invalid selected call target or argument count |
| `E0407` | Invalid typed discard |

Exact allocation within an existing family is fixed before implementing its compiler migration;
this table does not duplicate numeric registration. A generic read input cannot deliver T without
Copy merely because one future instantiation is an integer. Explain the missing capability or
owning source, not a missing `^` prefix. An uncontextualized closure's unmarked input is read;
body extraction cannot silently change it to take. If an expected callable supplies a different
operation, relate that selected contract when explaining its mismatch. Do not describe a data-type
annotation as overriding that input operation. A fixed value supplied to an edit input needs an intentional
mutable binding; do not silently rewrite it. A view descriptor Copy still retains its dependencies.

A field of a computed record value delivers a value, not existing mutable storage. Reject alias
creation, assignment or edit-input supply at that attempted operation; explain the need to bind
an intentional owner before taking scoped access. Grouping a read/edit alias does not convert
its referent into an owning result. Preserve known result types on interrupted evaluation, so
unknown fields and data mismatches still report their structural cause without inventing a
runtime access on that path.

A callable annotation mismatch identifies the incompatible invocation view, input count,
input operation/data (with position), or result data. Distinct concrete witnesses are not an
ordinary assignment/join match merely because their signatures agree. Explain that identity
constraint instead of selecting another target or silently erasing dispatch. A scoped alias cannot
construct an invocation adapter; explain that the adapted value needs its own binding. Invalid
callee data or call arity belongs to `E0390`; a non-writable `mut` environment belongs to `E0387`.
A formal callable contract supplies no Copy evidence: value extraction or taking forwarding from
read access uses `E0371`, regardless of the callbacks currently supplied by callers. Consuming or
dropping an owning formal participates in ordinary availability (`E0378`) and retained-access
checks. Equal signatures cannot repair a formal-witness identity mismatch. Callback header leaves
use the same unknown/unsupported data diagnostics as other private headers, including callable
result annotations.

Callable-result closure uses `E0370` when reachable returned values establish distinct code
witnesses, an unanchored recursive relation establishes none, or a dependent application lacks
an established witness for a required input in the bounded implementation. Explain that specific
failed obligation; never guess a target from a matching signature. A named value in a dead branch or overwritten before return
cannot establish a producer, including when supplied through another factory's input. Distinct
applications of the same factory cannot conflate different actual witnesses. For a deferred
join/replacement mismatch, identify the established code or input witnesses at the failed
operation. Retain known structural mismatches even when input evaluation interrupts the call. The exact closed producer can establish Copy for its result; this does not
change the lack of Copy evidence for a generic formal contract.

## Named-Slot Initialization Diagnostics

`E0378` identifies a reached read/update/extraction/drop/compound operation that lacks its required
initialization. Value use and transfer require a complete selected value; field replacement
requires present containing records, and `drop name` requires a present owning container rather
than restoring moved fields. A missing sibling does not invalidate a disjoint selected field. Relate the declaration and contributing transfers/drops/backedges. A Copy boundary
is not a transfer of its source; an explicit drop is destruction even for Copy. A mutable owning
slot can be restored by a complete replacement; a fixed binding cannot be assigned after transfer.
Check RHS-first compound targets after all right-hand effects. Unreachable code keeps structural
typing requirements without acquiring fictitious runtime initialization obligations.

Initialization does not establish dependency survival or absence of conflicts. Conditional cleanup
is not permission to read missing parts or skip required user destruction.

## Referent and Access Diagnostics

Retain the existing `E0395`–`E0398` access/lifetime families for failing origin, conflicting access,
reserved-place and surviving-root obligations. Explain the attempted source operation, affected
place, originating alias/capability, and later use or cleanup requiring it. In particular:

- An ordinary value return into destroyed local/guard storage identifies that cleanup boundary.
- A `void` call that retains a dependency identifies the edited destination/environment and the
  selected post-state clause/default, rather than claiming it returned a reference.
- An omitted public multi-origin bound identifies eligible source inputs without reading a body.
- A callback retaining a fresh invocation input identifies the escaping state and required extent.
- A fixed capability holder that needs ordinary update distinguishes holder mutability from
  whether the pointed-to element type is Copy.

Keep these causes separate from missing generic Copy evidence and data-type mismatch. The
compiler can end access after the last required use/cleanup; do not recommend broader scopes,
clones, resource-lifetime extension or symbols on every call as automatic repairs. Passing these
checks still requires resource/cleanup and native realization validation.
