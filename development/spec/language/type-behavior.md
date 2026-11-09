# Type-Owned Behavior and Construction

## Scope and Ownership

`impl Type` groups a type's construction functions, receiver methods, literal entries, supported
operators, coercions, explicit interface conformances and destructor. It does not define stored
fields, inherit behavior or create a runtime type object. `instance`, `construct` and top-level
`drop Type(...)` are retired forms, not alternative spellings.

The target is a nominal owned by the declaring module. An alias does not own another behavior
surface. Adding behavior to a foreign type or built-in requires an independently selected
authority; neither imports nor visibility grants that authority. `Self` denotes the current
specialized owner, independently of the spelling used to reach it.

Multiple source fragments may contribute distinct members to that owner. This is not an overload
set: each member identity has one contract and at most one definition under
[module pairing](modules.md#complete-implementation-signatures). Members cannot differ only in
result type, parameter operation, origin bound or generic requirement. Method and construction
names share the owner member namespace and cannot reuse a name to offer two call styles. Operator
identities are their fixed family/shape; read and edit index projections are explicitly separate
identities. Literal identity is its supported shape; one structural grant is permitted.

Source discovery order, access failure and a more specialized-looking signature never select a
member. Conditional/generic fragment applicability must be non-overlapping and established by
the owning generic contract before admission; this chapter does not provide overlap ranking or
complete generic solving. Full literal/coercion/expansion and platform contracts remain required
before accepting their bodies. The [source grammar](syntactic-grammar.md#files-and-declarations)
describes their container boundary, not those missing semantics.

## Functions and Methods

```text
impl Buffer {
    func create(capacity: usize): Self
    method self.len(): usize
    method edit self.append(value: u8): void
    method take self.finish(): Product!
}
```

A type-owned `func` constructs the owner. Its delivered payload must be `Self`, allowing the
selected optional/fallible wrappers. For `async func`, apply this test to the deferred output,
not the outer future. An alias that normalizes to that owner can satisfy the test, but an array
of owners, an arbitrary wrapper, `void` or an unrelated type cannot. Receiverless utilities remain
module functions. A nominal variant or literal entry is not an automatically generated named
factory. There is no implicit `new` entry.

A `method` has an explicit receiver: `self` reads it, `edit self` updates it and `take self`
receives a value under ordinary Copy-or-move supply. The receiver and ordinary inputs follow the
[value access rules](value-access.md); they do not depend on whether the result constructs
another value. The receiver has no alternate expanded `self: T` signature.

Call construction as `Buffer.create(64)` and methods as `buffer.len()`. Type-qualified method
calls and value-qualified construction calls are invalid; no UFCS conversion is performed.
Calls have no caller-side access markers. Result data is ordinary data, without a separate
ownership marker before the callable name or result type.

The public contract and implementation repeat complete signatures. Public bodyless construction
entries are valid contracts; their result does not require a body to be present in `index.nct`.
Generic owner arguments and a member's own generic arguments are distinct positions; their
complete inference and CTFE rules still require their owning generic contracts.

## Structural Literal Grants

Public fields expose access to fields, not the right to bypass construction. A root explicitly
grants structural construction with one bodyless member:

```text
// index.nct
copy struct Point {
    x: i32
    y: i32
}
impl Point {
    literal { x, y }
}
```

An outside caller can then write `Point { x: 1, y: 2 }`. The grant's names resolve to the root's
visible field identities; types are not repeated in the grant. It explicitly closes that braced
field contract as the complete semantic schema, so Point needs no private representation copy.
List every field exactly once, in declaration order. Unknown, duplicate or omitted fields
invalidate the grant. Each field must be accessible to the grant's whole audience. The empty
grant `literal {}` requires an explicitly empty braced field contract.

The grant owns a synthesized initialization entry, not another user body. Do not repeat it as a
private implementation or add a recursive `literal { ... }` body. Validate it once against the
root-owned complete schema. A later private representation/field addition is a contract violation
at the type definition, not a reason to withdraw construction from callers silently. A named
factory is the mechanism for hidden state, validation or additional initialization.

Actual field initializers may be written in any order. Require every granted field exactly once
with no extras, shorthand, defaults or update spread. Evaluate in written order with the ordinary
Copy-or-move component rule. Interrupted construction cleans successfully produced components in
reverse production order and never drops an uncompleted record. Complete record cleanup follows
reverse representation declaration order. [Nominal payloads](structs-and-enums.md) own these value
and partial-initialization rules; a grant does not weaken them.

Within a source that can see a private complete representation, direct complete field construction
remains available under the nominal payload rules. A root grant governs outside construction;
it is not a requirement to publish every internal initialization operation. Restricting a grant's
audience does not restrict a separately published factory or expose private fields.

## Destruction

A move-only nominal family may define one `drop { ... }` member in an ordinary implementation
source. There is no callable public destructor declaration, visibility directive, explicit
receiver/result head, async form or separate contract body. The member has implicit `edit self`
access to complete initialized `Self` and unit completion. Returning unit early proceeds to the
same automatic component cleanup as falling through.

```text
impl Handle {
    drop {
        self.release_resource()
    }
}
```

`release_resource` here denotes an independently checked immediate, non-failing and non-waiting
operation; its name supplies no effect guarantee. Infer and verify destructor effects through
its actual selected calls. A destructor cannot throw, suspend or wait. This rule does not invent
an authored `notrap` guarantee or guarantee cleanup after a trap/divergent operation. Full effect
inference is required before admitting user destructors to executable products.

After normal user-body completion, clean the initialized stored fields in reverse representation
declaration order exactly once. The body cannot move/drop fields into an incomplete state or
recursively drop `self`; an ordinary checked replacement may preserve complete state. A type with
a user destructor cannot leave scope or enter destruction partly initialized. A consuming method
cannot bypass that invariant to move out a non-Copy field; it needs a checked complete replacement
or another independently valid extraction protocol. Complete-self entry and remaining cleanup
follow [value access](value-access.md#return-replacement-and-cleanup).

The destructor covers the entire unrefined nominal generic family. A conditional fragment cannot
introduce another destructor or make destruction depend on a specialization. A `copy struct`
family cannot define one. A private destructor is still observable lifecycle behavior: inference,
origins and optimization must preserve its required accesses/effects, even though its body is
omitted from the public contract. Cleanup is resource release, not proof of successful protocol
completion such as finishing a writer. General resource-protocol and cancellation contracts
remain separate obligations.
