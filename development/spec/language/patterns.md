# Patterns and Enum Branches

## Scope

This chapter owns common tuple binding and enum branch operations. [Nominal payloads](structs-and-enums.md#enum-schemas-and-construction)
own enum declarations, construction and Copy. [Value access](value-access.md) owns supply,
initialization, dependencies and cleanup. The [grammar](syntactic-grammar.md#patterns-and-enum-branches)
owns recognition. These adopted v2.0.0 contracts do not extend the bounded private compiler.
Generic schemas, iteration protocols, target layout and native realization still need qualification.

## One Operation over a Pattern

A binding pattern is a name, `_`, or a nested tuple pattern of at least two elements. It is
irrefutable: its tuple shape must agree with the known data type. A tuple pattern does not
convert an array, record or equal-layout type into a tuple. There are no per-leaf operation
prefixes, record patterns, rest/splat patterns, single-element tuples or whole-value `@` binders.
Enum patterns occur only in enum branch heads, not ordinary local/parameter bindings.

The enclosing operation applies uniformly to named leaves:

| Position | Leaf operation |
| --- | --- |
| let / var tuple binding | Fixed / mutable independent values |
| read / edit tuple binding | Scoped read / update access to existing component places |
| Unmarked / edit / take input pattern | Read / update / received values under the input contract |
| Unmarked / edit / take enum branch | Read / update / received values under the branch input contract |

```text
let (left, right) = pair
var (left, right) = pair
read (left, right) = pair
edit (left, right) = pair

func combine((left, right): (i32, i32)): i32 {
    left + right
}
```

The first four lines are alternative declarations. Let/var acquire one aggregate value by ordinary
Copy-or-move. Read/edit identify one existing aggregate and project its components without
constructing owners. Optional annotations constrain data, never the operation. Named take-input
and take-branch leaves are locally writable, like other take parameters; a read leaf does not
become writable because its datum is Copy. Scoped aliases cannot escape as ordinary T values.

Each parameter pattern still denotes one argument position. Tuple nesting does not flatten
source arity or create runtime parameters. Names are unique across the complete binding/header;
`_` binds no name and cannot be referenced or named as an origin. Ordinary lookup is not a
destructuring search. A failed pattern shape or unavailable authority cannot select a different
operation. The future for-binding shape may reuse this pattern, but this chapter does not pick
an iteration protocol or add read/edit iteration spellings.

### Header Origins and Correspondence

Each named leaf in a parameter pattern identifies its input position plus a semantic tuple path.
For example, `from left` in a header with `(left, right): (Buffer, Buffer)` names that projection,
not another argument. A whole name such as `pair: (Buffer, Buffer)` instead names its complete
eligible source subtree. Resolve names after collecting the whole header, including later inputs.
When every eligible origin slot belongs to one input source, elision can bound that whole input
subtree even if its header has no whole-input name. Destructuring alone therefore does not add
a from requirement. This conservative bound does not equate different slots or identify one
actual referent; an explicit leaf name can promise a narrower source when useful. Multiple
eligible input sources still need the stable explicit public bound under the origin rules.

An origin clause after an edit tuple input's type bounds the complete edited aggregate, including
ignored leaves and reachable writes, rather than only leaves used in the body. It cannot exempt
an unchanged second component from a promised dependency-free post-state. Use the owning origin
rules for retained-state and deferred cancellation relations.

Contract/body pairing repeats the complete pattern and names under the module rule. Structural
callable identity instead uses input positions, data/operations and normalized dependency paths,
not leaf spellings or an alternative destructuring spelling. Pattern names in a callable type
name those paths; they do not impose another callee implementation or caller-side discard.
Interface/helper compatibility likewise maps names through positions/paths rather than matching
text. Neither matching mechanism may invent extra arguments.

## Enum Branch Input

Match and enum if-is tests use the input vocabulary: unmarked reads, edit updates, take receives
a value by Copy-or-move. Do not add an equivalent explicit-read mode. The mode selects access
before tag inspection or body checking, independently of Copy, later use and which arm matches.
It is a new pattern-binding operation, not a repeated assertion on a declared function call.

```text
match message {
    Message.number(value) { inspect(value) }
    Message.report(reason) { report(reason) }
    Message.empty { () }
}

match edit message {
    Message.number(value) { value += 1 }
    Message.report(_) { () }
    Message.empty { () }
}

if take message is Message.report(reason) {
    consume(reason)
}
```

These are separate schematic uses with compatible enclosing result types and independently
declared helpers. The unmarked arms alias existing payloads; they do not move a resource merely
because a pattern names it. Edit needs initialized writable storage. Take named leaves receive
values; taking a Copy scrutinee snapshots it and otherwise transfers an eligible owner. A read/
edit alias cannot supply a non-Copy take. No field rights are inferred from subsequent mutations.

Evaluate the scrutinee once. A read test can observe a fresh computed value owned by its evaluation
frame, but an edit test requires an existing writable place. Grouping preserves that distinction.
Fresh owners survive through branch evaluation and its cleanup, including suspension where valid;
a result depending on a temporary cleaned at handoff cannot escape it.
An explicitly computed conditional/call still performs its ordinary value construction before
the pattern observes it. A read branch mode does not undo transfers performed by its operand.

Take acquires the complete carrier before the test, even on a false if-is path. A non-Copy source
is unavailable on both paths; Copy leaves its source intact. On a false path or whole `_` arm,
clean that received carrier before entering the fallback. There is no delayed move depending
on a successful pattern, rollback or permission-based fallback to read mode.
Bindings/tag facts describe the received snapshot for a Copy take, not an alias of the original
source. Existing index/outcome projection rights still govern the operand: take does not make
a projected container element an owning source for non-Copy removal.

## Variant Identity and Coverage

An enum pattern is a qualified owner/variant, with one common binding pattern per declared
payload position. The owner may be a namespace path, transparent alias, Self or an explicit/
inferred generic enum owner under the generic rules. Resolve one exact nominal variant identity;
equal names/layouts in other enums cannot match it. This is not runtime type testing.

Payloadless variants omit parentheses. Payload variants require parentheses and exact arity;
`_` ignores one payload position. Nested tuples use the same irrefutable pattern above. There
are no payloadless `variant()` patterns, implicit payload omission, nested enum patterns, arm
guards or optional/fallible Some/None/success/failure patterns.

A match must have at least one arm and cover every variant accessible in its complete schema,
or use a final `_` for the remainder. More precisely, exhaustiveness concerns the actual schema
promised at that use: an opaque public enum requires `_` because its variants are not exposed;
a source with direct visibility of its private completion may cover those exact variants.
Reject inaccessible/foreign variants, duplicate arms, a non-final catch-all, and a redundant
catch-all after every variant is already covered. Do not infer exhaustiveness from an opaque
type's currently private representation. Coverage is checked structurally even if a particular
scrutinee is known to select one arm; that knowledge can optimize execution, not repair missing
source coverage. Only the selected arm executes.

If-is tests one variant and need not be exhaustive. Names exist only on its true branch. A
normal Boolean if requires bool, without truthiness/implicit outcome extraction or an enum
input mode; without else, a normally completing true branch must
produce unit. A value-producing if needs compatible normally producing alternatives. Match/if
results construct ordinary values using the common branch rules, never implicit aliases.
Returning a non-Copy read leaf is invalid; a Copy leaf can supply a snapshot. The expected result
cannot retroactively make a read pattern take its payload.

## Binding Formation and Cleanup

For an acquired value, process named leaves in written tuple/payload order using the ordinary
value-supply rule on each selected component. Copy evidence can snapshot a component; otherwise
an eligible component transfers. Record those choices in the source checking context, not after
concrete specialization. Stage all named bindings before running ignored/remainder cleanup.
Names enter body scope only after formation completes. No partial binding is an argument or
result from an interrupted scrutinee evaluation.

`_` creates no component alias/value binding. In a read/edit pattern it leaves its component in
the original carrier. In value decomposition it leaves that component for the received carrier's
cleanup. After forming named values, dispose the remaining carrier before the branch/function
body or local declaration continuation. Active payload/tuple components follow their ordinary
reverse declaration order, skipping transferred fields. Named owning bindings then receive
ordinary reverse formation-order cleanup on their enclosing scope exit. An explicit whole
discard local remains `let _ = expression`; all-discard local tuple bindings and var/read/edit
complete-discard aliases are not alternate spellings.

A user-destructible enum cannot be made incomplete by pattern extraction. Copy leaves may be
snapshotted while keeping the received carrier complete; ignoring other fields leaves them for
whole-carrier destruction. Run that one destructor and component cleanup before the body. A
non-Copy leaf needing extraction from such a carrier is invalid, without silently making it a
read alias or delaying its destructor. Read/edit patterns can instead expose existing payload
access while preserving complete initialization on every recoverable/cancellation exit.

At an immediate function/closure entry, received value patterns are decomposed then. For an async
producer, value receipt occurs at construction but body-entry pattern formation/discard occurs
when first driven. Cancelling before first drive instead cleans the intact received values under
the async schema. Do not advance a discard's destructor effects into lazy construction because
the parameter name is unused. A structural callable annotation alone runs no pattern cleanup.

The tag refinement and payload accesses last only as long as needed by uses/dependent cleanup.
Live payload access prevents invalidating its variant/container. After those demands end, an
already writable parent may replace the variant; invalidate stale tag facts rather than holding
an unused alias to the end of the arm. This does not grant write permission to a read parameter.

Break/continue target an enclosing loop, not the match. Return/throw and ordinary branch completion
stage their result before branch-local cleanup. Suspension preserves the active payload/root
relation and parent initialization. Check destructor dependencies, ignored resources and all
exit paths under their owning rules. A discarded owner needed by a surviving view is invalid;
do not retain it invisibly, retarget a view or introduce a clone to make decomposition succeed.
