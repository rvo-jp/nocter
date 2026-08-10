# Explicit Interface Conformances

## Purpose

This document defines the compiler boundary for explicit interface conformances. Public source semantics belong in the
[language specification](../../spec/08-generics-interfaces-embedding-methods.md).

One `ConformanceDecl` owns each conformance method from parsing through native lowering and editor
presentation. An `InstanceDecl` owns inherent methods and destruction separately.

## Canonical Source Model

```nct
conform Iterator for Counter<T> {
    type Item = T

    method &+self.next(): T? {
        // implementation
    }
}
```

- braces are mandatory
- required methods are declared in the conformance body and always have bodies
- default methods may be omitted or overridden in the conformance body
- conformance members do not carry `pub`; visibility comes from the interface contract
- extra methods, undeclared associated type bindings, associated functions, literals, and `drop`
  members are rejected; required associated bindings are validated separately
- `instance Type { ... }` members never satisfy or override an interface contract
- an empty body is valid only when every required method has an applicable default

## Compiler Ownership

The AST represents the two declaration forms separately. `InstanceMember` permits only `Method`
and `Drop`; `ConformanceMember` permits only `AssociatedType` and `Method`. Resolution creates a
stable conformance identity and a stable identity for every member. The conformance record owns
those members; the target type's inherent member table does not.

Type checking resolves each member against exactly one method declaration from the stated
interface. It validates receiver mode, method generics and bounds, parameter and result types,
result provenance. Compiler-owned result storage and execution allocation do not participate in
source signature compatibility. Parameter names do not participate. Missing, extra,
duplicate, or incompatible members are source errors.

Concrete method lookup may select an inherent member or one accessible conformance member. A name
that is supplied by both categories, or by multiple applicable conformances, is rejected until a
qualified-call surface is designed. Generic-bound lookup selects the named interface declaration
first and then specializes the corresponding conformance member or default declaration. Import or
declaration order never selects a candidate.

## Downstream Identity

Buildability and IR lowering receive the selected method owner directly. They do not
repeat signature matching or recover a target by method name. Ownership, provenance, allocation
effects, cleanup, and native call targets use the same specialization key.

Hover, completion, definition, references, signature help, semantic tokens, and document symbols
use the conformance member's source span. Generic calls define to the interface contract while
specialization facts retain the concrete conformance target. Incomplete conformance blocks may
recover syntax, but must not invent conformance or member identities.

## Migration Boundary

The distributed standard library and test corpus move required and explicit override bodies from
instance blocks into conformance blocks. Unrelated inherent helpers remain in `instance Type` blocks.
Brace-less conformances and structural inherent-method satisfaction are removed rather than
deprecated. No hidden desugaring recreates the former model.

Repository-home and packaged-home checks now agree for direct calls, generic-bound calls,
conditional conformances, interface defaults, ownership-sensitive receivers, provenance,
result-storage inference, malformed source, and editor requests. The migration passed the full compiler,
native runtime, packaged-home, Clippy, documentation, local distribution, `doctor`, and archive
qualification matrix on 2026-08-04. v0.11.0 Phase 4 replaced the overloaded `impl` declaration
with the structurally separate `instance` and `conform` declarations.
