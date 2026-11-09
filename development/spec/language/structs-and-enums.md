# Structs and Enums

## Nominal Value Payloads

This section adopts the value-binding direction; it does not change released struct behavior.
Input operations, local bindings and value construction follow [Value Access](value-access.md).

A struct field contains a value of its declared data type. Read/edit access to that field is an
operation on the containing place, not a different field data type. The declaration supplies one
ordered field schema. Distinct fields remain distinct semantic places even when their data types
and names in other structs are identical. Field names are unique within their struct. A field
name does not reserve the same spelling in another struct or in the type namespace.

This chapter's complete schemas and raw field construction apply to a private definition or an
explicitly closed public structural schema. A root's visible fields alone are not a complete
representation. [Module contracts](modules.md#nominal-representation-and-visible-fields) select
the single complete schema owner; an outside caller needs the
[structural grant](type-behavior.md#structural-literal-grants) or a construction function.
Enums use the separate schema and exposure rules below; the struct-field rule does not hide
variants of a purportedly exhaustive enum.

Ordinary structs are move-only. `copy struct` explicitly permits structural Copy only when every
field has Copy evidence and the struct has no user destructor. An ordinary empty struct and an
ordinary struct containing only Copy fields remain move-only. An empty `copy struct` satisfies
the field condition. Copy evidence for a nested ordinary struct cannot be inferred from its
private fields or bypassed by an enclosing `copy struct`.

A generic `copy struct` derives a conditional Copy obligation from its field schema. A field
depending on a generic parameter contributes that parameter's required evidence; an
unconditionally non-Copy field invalidates the declaration. Each concrete specialization is
Copy exactly when every substituted field is Copy. A specialization lacking that evidence is
move-only with ordinary structural field cleanup; the whole family still cannot have a user
destructor. Generic code may use this condition only from its declared assumptions, not a
concrete caller discovered later. An ordinary generic struct does not gain this opt-in from
its fields. These schema rules require the owning generic proof before compiler admission.

```nct
copy struct Point {
    x: i32
    y: i32
}

struct Failure {
    cause: error
}
```

`Point` has structural Copy evidence. `Failure` does not. Changing `Failure` to `copy struct`
is invalid because `error` is move-only. Copy eligibility does not erase a payload's carried
accesses, dependencies or cleanup obligations.

Direct by-value record containment must have a finite schema. Reject self-containment, mutual
containment and a containing record whose schema necessarily expands into such a cycle. Report
the field whose declared type leads into the non-finite schema. A separately specified owner or
container capability can terminate direct containment; mentioning an element type behind that
capability does not itself imply direct containment. This rule does not forbid recursive data
structures through those independently checked contracts or prescribe physical storage placement.

```nct
struct Invalid {
    next: Invalid // Invalid: direct by-value containment has no finite payload schema.
}
```

For the bounded proof case consisting only of records and `bool`, `i32`, `void` and `error`
leaves, with no user destructors, external payload dependencies are empty by the exact leaf
contracts and component composition. This is not a general inference from receiving a value,
nominal identity, move-only status or Copy evidence. Adding views, capabilities, callables,
generic arguments, outcomes or user destructors requires their own payload contracts before
those shapes can be admitted.

A structural value is written `Type { field: expression, ... }`, with every declared field exactly
once and no unknown fields. Initializer order is free and evaluation is left to right in the order
written. Each component uses ordinary Copy-or-move value construction, not an implicit read input
or an alias. Empty records use `Type {}`. Commas separate fields and one trailing comma is allowed.
There is no field shorthand, default, update spread or positional construction.

Successful component values remain owned until the complete value is assembled. If a later
initializer returns, breaks, continues or otherwise exits the construction, clean only the
successfully produced components in reverse actual production order; do not run a whole-record
cleanup for a value that was never completed. Earlier moves remain performed. Once complete,
ordinary record cleanup follows reverse declaration order as specified by Value Access, independently of
the initializer order. A compiler may retain the components in separate temporary places until
completion; the rule does not require allocating incomplete record storage.

At the outer level of an `if` or `while` condition, the first `{` starts the control body.
Parentheses or another nested delimiter are required around a record literal in that condition;
parsing never asks whether the preceding name denotes a type.

Field schemas retain declaration order and nested types rather than flattening a record into a
scalar or a list of destruction effects. Partial initialization, selected-place conflicts,
replacement and surviving-field cleanup must be checked under
[Value Access](value-access.md#return-replacement-and-cleanup) before record operations are accepted.

## Enum Schemas and Construction

An enum is a nominal choice of one active variant and that variant's ordered payload. Variants
and payload positions have declaration identities, not byte offsets or names borrowed from
another enum. Variant names are unique within an enum; payload field names are unique within
their own variant. Different variants can reuse a payload name. A complete enum has at least
one variant. An empty variant set is not another way to declare never.

```text
enum Message {
    empty
    number(value: i32)
    report(reason: error)
    pair(values: (i32, i32))
}

let message = Message.number(42)
let empty = Message.empty
```

Payloadless variants are values written `Type.variant`, without an empty parameter list.
Payload variants declare one or more named data positions and are constructed with positional
arguments of exact arity. The owner is always qualified; an alias or namespace path retains
the same nominal/variant identity. Generic owner arguments are complete or uniquely inferred
under the generic contract. The compiler does not guess an owner from a variant name.
Arguments belong to the enum owner, not a separate callable-generic list on the variant name.
Every payload type must be valid storable data; a variant does not authorize never as a stored
field. Unit payloads remain real logical fields even when their physical size is zero.

A variant constructor is an enum construction entry, not a first-class function value, receiver
method or special function name. Variants and named functions/methods share the owner member
namespace and cannot overload each other. A user can declare an ordinary helper/closure when
a callable construction value is needed. No default payloads, named call arguments, field
shorthand, variant spread or payloadless `variant()` alternative is introduced.

Evaluate supplied payload arguments once, left to right. Each component uses ordinary value
construction. Complete the variant only after all components are ready. Interrupted construction
cleans completed payloads in reverse production order, without whole-enum destruction or
restoring previous transfers. Normal structural cleanup visits only the active payload in
reverse declaration order. Inactive variants have no initialized resources to destroy.

Read/edit payload access is obtained through the [pattern protocol](patterns.md), not a field
selection valid on every variant. A tag test does not grant ownership or make an invalid payload
initialized. Known tags can narrow dependency/cleanup alternatives, but do not make a non-Copy
enum Copy. Carried accesses, outside storage and reachable writes follow the selected variant's
checked schema; unknown alternatives are not simultaneously owned duplicate capabilities.

This selected source scope requires finite by-value payload schemas, including through records
and tuples. A terminating alternative alone does not make an unbounded direct by-value cycle
finite. Recursive data through an independently checked owner/container is not forbidden;
implicit recursive indirection, its backing/failure semantics and general recursive schemas need
separate closure before admission. This is not permission to copy a published representation limit.

### Copy Contracts

A complete declaration with only payloadless variants is a scalar enum with Copy evidence.
A payload-bearing enum is move-only unless it opts in with `copy enum`. That opt-in uses the
same conditional structural rule as copy structs, requiring Copy for every declared payload
position in every variant, not just the current tag. A permanently non-Copy payload invalidates
the request. Generic conditions remain part of the checked type contract and cannot be inferred
anew by callers/lowering from hidden bodies; changing an advertised condition is an API change.

```text
copy enum Reading {
    missing
    value(number: i32)
}
```

Copy types cannot have a user destructor, and an opted-in Copy family cannot have one even when
one generic specialization lacks Copy. Payloadless scalar families likewise cannot acquire a
destructor. A move-only enum may define its unique complete-self destructor under type behavior;
its active payload is cleaned once after normal destructor completion. Patterns cannot bypass
that invariant. Copy never implies structural equality, ordering or absence of outside roots.

### Public and Opaque Schemas

A braced enum in index.nct publishes the complete variant/payload schema. It owns that schema
without a second private representation declaration. Each variant is an explicit public
construction choice, unlike a struct field list that needs a structural grant. All variants
have their enum's audience; per-variant/payload visibility and hidden additional variants are
not introduced. Use an opaque enum when clients should not construct or exhaustively inspect
the alternatives. A hidden payload type cannot be exposed through a supposedly accessible variant.

A bare enum contract, such as `enum Token`, hides every variant and requires one complete
definition in a reciprocally seen ordinary source. The completion repeats its nominal/generic/
Copy header and supplies one schema, not another type. Only sources with direct visibility of
that complete definition can use its private variants. Seeing its root or an unrelated helper
does not inherit private variant access. Outside construction uses contracted named factories;
pattern coverage cannot depend on the current hidden alternatives.

An opaque enum without a Copy grant is a move-only family even if its private completion has
only payloadless variants. The completion does not change the root's chosen policy or give
private bodies another whole-type Copy rule. `copy enum Token` explicitly exposes the Copy
contract and its checked conditions while hiding variants. That grant alone advertises neither
equality nor any variant. A private complete enum declaration is its own schema owner; the
payloadless scalar rule applies there normally. Nominal aliases cannot widen schema visibility.

The [module chapter](modules.md#nominal-representation-and-visible-fields) owns source pairing
and audiences; [patterns](patterns.md#variant-identity-and-coverage) own use-site exhaustiveness.
Adding a private completion to a closed root, missing an opaque completion, changing its Copy
policy or repeating a variant is a declaration error, without first-match selection.

### Representation and Evaluation

There is no source-level eight-bit tag or 256-variant ceiling. Semantic alternatives are finite
but physical tag width, niches, unboxed/scalarized payloads and layout belong to the selected
target/representation proof. Source does not expose a variant ordinal or infer numeric
conversion/ordering from declaration order. Resource exhaustion is not an invented language
variant limit. Foreign/unchecked data needs its own validity/layout contract before it can
participate as a valid enum; a match cannot sanitize an unvalidated representation by assumption.

Only the selected constructor/arm's required effects execute. Proven tags may remove branches,
inactive cleanup or physical tag storage while preserving source operations, ordering, roots,
outcomes and destruction. Such transformations grant no alias ownership or hidden Copy evidence.

Checked const evaluation may construct finite eligible variants and inspect them with the same
pattern operations. Final-value eligibility and target serialization belong to
[compile-time values](generics-and-constants.md#constants-statics-and-final-values), not a
second enum evaluator. Only active payload values are present; an inactive resource-bearing
variant does not create a resource in an otherwise eligible constant. An enum-owned destructor or
actual retained resource still prevents final constant/static admission. These selected rules
do not claim implemented enum parsing/checking, const execution or native layout.
