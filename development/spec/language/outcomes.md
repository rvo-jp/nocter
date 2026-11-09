# Optional Values and Recoverable Failure

## Scope and Constructors

`T?` is absence or presence of T. `T!` is success T or failure with the one built-in `error`
payload. A payload having type `error` never selects failure by itself. Error information can
be successful data, including the success payload of `error!`. The
[error library contract](../library/error.md) owns snapshot/storage behavior, independently of
these branch rules. No per-API failure type, implicit error conversion or exception mechanism
is introduced.

The shared failure payload is a deliberate composition boundary: callers should not need
per-library wrapper types, conversion chains or information loss merely to align signatures.
Failure classification and context remain library data rather than another static failure type.

Optional layers may repeat. At most one fallible layer may occur along an uninterrupted chain
of optional/fallible constructors. Preserve their order: `T??!` is a fallible result whose
successful value has two optional layers; `(T!)?` is optional fallible data. Ungrouped `T!?`
and `T!!`, or grouped repeated failure such as `(T!)!`, are not introduced. Check these limits
after aliases and generic substitution. A non-outcome constructor starts a separate component
chain; an array of fallible elements is not the same as repeated failure on one value.

`void` has its ordinary one value `()`, so `void?`, `void!` and supported mixed forms distinguish
absence, completion and failure. `never` does not become a storable success/present payload.
Optional Copy requires Copy for its payload, independently of its current tag. Every fallible
value is move-only because it may own an error, even if its successful payload is Copy.

These are adopted v2.0.0 source/model rules, not acceptance by the bounded private compiler.
Layout, native realization, general capabilities, evaluator outcome support and library migration
still require qualification. Published outcome rules are not a fallback.

## Contextual Construction and Exact Forwarding

At an ordinary value-producing expected-type boundary, apply one rule:

1. An existing value of exactly the expected type is forwarded with its tags unchanged, under
   ordinary value supply. Exact forwarding has priority over adding a layer.
2. At expected `U?`, the contextually typed literal `none` constructs outer absence. Otherwise
   supply a value to U recursively and construct one present layer.
3. At expected `U!`, supply a value to U recursively and construct one success layer. A plain
   `error` value follows this same success rule; it is not a failure instruction.
4. At a non-outcome expected type, require ordinary compatible data and a valid supply operation.
   Do not unwrap, flatten, reorder or implicitly erase a mismatching outcome.

The rule applies to value bindings, stored components, ordinary results and taking inputs.
Input operations remain independent. A direct edit place must already have compatible outcome
data and update authority; do not copy/inject a wrapper from that place to repair the call.
An already produced value may undergo ordinary contextual construction and lend read/update
access through its caller-owned evaluation temporary. A read input may also observe a wrapper
constructed from a Copy source. Implicit wrapper construction cannot consume a direct non-Copy
source merely to satisfy read/edit; construct the intended value explicitly when it requires a
transfer. The [temporary protocol](value-access.md#preparation-invocation-and-evaluation-temporaries)
does not grant ownership to an edit callee or extend an escaping root.
Do not change an input's operation to repair an injection failure.

`none` has no unconstrained payload type. An expected type or other generic constraints must
determine it. The ordinary prelude [some function](../library/prelude.md) deliberately adds
exactly one present layer; it is not syntax recognized by its name:

```text
let value: i32? = 10
let absent: i32?? = none
let present_absent: i32?? = some<i32?>(none)
let present_value: i32?? = some(some(42))
```

The final three values have different tags. A typed existing `i32?` injected into `i32??` is
present with that entire inner optional; its absence is not lifted to outer absence. Generic
construction records its selected layers before substitution: `some<T>` still adds one layer
when T is itself optional. Exact forwarding of an already complete result never adds another.

## Success, Failure and Propagation

`return expression` and a callable's body result use ordinary contextual construction or exact
forwarding. Bare return/normal unit completion supplies `()`, including success/presence wrappers
where the declared result admits that unit. It does not invent an absent optional value.

`throw expression` evaluates one error value, transfers it into the current callable's unique
declared failure layer and exits that callable with ordinary recoverable cleanup. Optional
layers outside that failure are constructed present. It never exits an enclosing closure,
searches for a handler or propagates through callers automatically. A non-fallible callable
cannot throw. A read/edit error alias cannot supply an owned error without an independently
valid value construction. There is no `throw ^error` alternative.

```text
func recorded(take reason: error): error! {
    return reason^ // success containing error information
}

func failed(take reason: error): error! {
    throw reason^ // failure containing the same kind of error information
}
```

Ordinary postfix `?` eliminates exactly one outer outcome layer. Presence/success delivers its
payload; absence/failure exits the current callable through its compatible declared layer.
Failure selects the unique fallible layer. Absence selects the outermost declared optional layer
along the result's outcome chain; other outer layers are constructed present/success. No
failure-to-absence or absence-to-failure conversion occurs. A result lacking the needed layer
is an error. To construct a different nested absence use an explicit ordinary value constructor,
not hidden flattening or a destination chosen from a later use.

For a value elimination, supply the complete carrier under value-supply before selecting its
tag. An existing non-Copy carrier requires its postfix transfer, as in `stored^?` or
`stored^ catch failure { ... }`; a marked Copy carrier also retires. A transferred source is
unavailable on every continuation. Snapshotting an unmarked Copy optional instead
leaves its source intact. Stage the successful result before destroying remaining temporaries;
on propagation stage the error/absence result before normal scope cleanup. Edits already
performed are not rolled back, and outward edit post-state/initialization promises still apply.

Postfix `!` eliminates one layer too, but absence/failure takes the ordinary non-recoverable safety
trap. That path does not unwind or run source cleanup; it does not print the error or promise a
process status. Neither `catch` nor `otherwise` catches it. A possible trap keeps its runtime
check unless proved unnecessary. There is no authored `notrap` restriction to restate that fact.

Only one elimination suffix appears at one expression level. Use grouping or a binding for the
next layer, as in `(operation()?)?`; `operation()??` is not expression syntax.

## Local Recovery

`expression catch failure { body }` handles one outer fallible layer. Success supplies its payload;
failure binds the error and evaluates the recovery body, whose normal result must supply the
same payload data. The body may instead return, throw, break, continue or terminate under those
constructs' ordinary rules. It is not a dynamic enclosing handler. A trailing error value does
not implicitly rethrow; use `throw failure^` with eligible ownership or `?` on another outcome.

For ordinary value recovery, the complete carrier is supplied first and the named error is a
fixed owning handler local. Protect the recovery result before cleaning handler locals. `catch _`
explicitly suppresses the failure, binds no name and destroys the received error before the
fallback body. Bare `catch { ... }` is invalid. A `void` payload can recover by unit completion;
another payload requires a value or terminating path. Recovery does not automatically flatten
another outcome value.

`expression otherwise { body }` similarly handles one outer optional layer, without an error
binding. Presence supplies its payload; absence evaluates a replacement/terminating body.
It is not optional propagation and cannot handle failure. Chained recoveries operate on each
established result in source order; they never commute or reorder constructor layers.

## Scoped Payload Projections

Outcome payloads also support existing-place access. A `read`/`edit` initializer, or a selected
direct read/edit call operand, can request a payload projection instead of ordinary value
elimination. The carrier must be an existing compatible place, with sufficient access; an
argument-evaluation frame may own a temporary for a call's bounded duration. A named scoped
alias still cannot retain an invisible temporary owner.

```text
read observed = maybe_file otherwise { return }
edit working = maybe_file otherwise { return }
read value = stored_result catch failure {
    report(failure)
    return
}
```

These illustrate separate access requests, each under its own enclosing function and sufficient
source authority; an already live read projection can conflict with the edit request.

On presence/success, preserve the carrier and alias its active payload. An edit projection needs
update authority and must keep that payload/carrier initialized. The live access prevents tag
replacement, incompatible mutation, transfer or destruction of its carrier. This is the same
existing-element protocol as [index projection](equality-and-indexing.md#index-projections),
without restoring ordinary `&T`/`^T` types. A value-producing use still extracts under value-supply;
context does not turn an arbitrary conditional value into a place.

In projection recovery, a named catch binding has the same read/edit operation into the existing
error payload; it is not a second owning error. `catch _` creates no alias and explicitly accepts
that failure for the recovery without destroying the still-retained carrier. A normal fallback
must itself produce a surviving compatible place; a fallback local cleaned at handler exit is
invalid. A terminating fallback needs no projected result. Stage the projection, then complete
handler-local cleanup before delivering it; do not postpone a guard's destruction to keep it valid.

A projected `?` may propagate absence without transferring a payload. A fallible projected `?`
cannot implicitly retire the carrier or extract its error on the failure branch. Reject that
use even for a locally owned carrier. Use explicit value elimination such as `carrier^?` when
ownership can be surrendered, or local recovery without surrendering caller storage. A read/edit
input cannot supply another owner's error by adding a caret. No implicit error copy repairs this.
Projected `!` instead traps on the non-value branch with the same non-unwinding rule.

Projection preserves payload dependencies and the active-variant relation. Copy of a projected
descriptor never duplicates exclusive access or removes outside roots. Taking a non-Copy value
through a read/edit projection remains invalid without a checked removal/replacement protocol.

## Failure Decisions and Resource Completion

Dropping/discarding an unhandled fallible value receives a default-enabled warning, even through
`let _`, an unused named local, explicit drop, replacement or an aggregate field. It is not a
memory-safety error: cleanup still runs and source validity is unchanged. Explicit `catch _`
expresses suppression; forwarding/propagation transfers the decision to its recipient. Ordinary
cleanup by itself does not express that failure was accepted. Do not warn while the value is
still retained for later handling or transfer. Read access alone does not transfer responsibility.
The warning concerns a failure that disposal could lose, not the presence of ordinary error data.
A producer proved to yield only success has no lost failure; unknown possible tags require a
conservative diagnostic rather than runtime handling machinery.

An explicit projected catch handles the encountered failure while retaining its carrier. Moving
that same locally handled instance should not produce duplicate warnings; a later operation can
produce a new failure decision. Library/generic forwarding and aggregate schemas must preserve
enough conditional handling evidence to diagnose actual loss without a new source qualifier or
runtime handling bit. A diagnostic cannot silently assume an opaque payload contains no failure.
The complete diagnostic analysis needs its own source-backed qualification before admission.

Cancellation and a produced outcome are different events. An unstarted future has no body result
to handle. Ready-output disposal follows its owning async cancellation contract; it must not
masquerade as successful resource completion. The [async contract](asynchronous-computations.md#outcomes-and-resource-completion)
distinguishes abandonment before output delivery from losing an already delivered outcome.
Task/library ownership and exact cleanup schemas must be closed before their admission; not
every future drop is plain source-outcome discard.

Resource release never proves that flush/finish/commit succeeded. Such operations expose their
own outcomes and partial-progress/cancellation contracts. Trap, immediate process termination,
recoverable failure and cancellation remain distinct lifecycle exits. No error-type reconciliation,
implicit clone, rollback or automatic resource-lifetime extension repairs a failed obligation.
