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
Public enum exposure/exhaustiveness is not closed by these struct rules.

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
