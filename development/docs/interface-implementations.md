# Body-Bearing Interface Implementations

## Purpose

This document defines the compiler boundary for the body-bearing interface implementation model
adopted during the reopened v0.3.0 stabilization gate. Public source semantics belong in the
[language specification](../../spec/08-generics-interfaces-embedding-methods.md).

The migration removes structural matching between an empty conformance declaration and unrelated
public inherent methods. One source declaration must own each interface implementation method from
parsing through native lowering and editor presentation.

## Canonical Source Model

```nct
impl<T> Iterator<T> for Counter<T> {
    method &+self.next(): T? {
        // implementation
    }
}
```

- braces are mandatory
- required methods are declared in the conformance body and always have bodies
- default methods may be omitted or overridden in the conformance body
- implementation members do not carry `pub`; visibility comes from the interface contract
- extra methods, associated functions, literals, and `drop` members are rejected
- inherent `impl Type { ... }` members never satisfy or override an interface contract
- an empty body is valid only when every required method has an applicable default

## Compiler Ownership

The AST stores interface implementation methods on the `ImplDecl`. Resolution creates a stable
conformance identity and a stable identity for every member. The conformance record owns those
members; the target type's inherent member table does not.

Type checking resolves each member against exactly one method declaration from the stated
interface. It validates receiver mode, method generics and bounds, parameter and result types,
result provenance, and allocation effects. Parameter names do not participate. Missing, extra,
duplicate, or incompatible members are source errors.

Concrete method lookup may select an inherent member or one accessible conformance member. A name
that is supplied by both categories, or by multiple applicable conformances, is rejected until a
qualified-call surface is designed. Generic-bound lookup selects the named interface declaration
first and then specializes the corresponding conformance member or default declaration. Import or
declaration order never selects a candidate.

## Downstream Identity

Buildability and IR lowering receive the selected implementation declaration directly. They do not
repeat signature matching or recover a target by method name. Ownership, provenance, allocation
effects, cleanup, and native call targets use the same specialization key.

Hover, completion, definition, references, signature help, semantic tokens, and document symbols
use the implementation member's source span. Generic calls define to the interface contract while
specialization facts retain the concrete implementation target. Incomplete conformance blocks may
recover syntax, but must not invent conformance or member identities.

## Migration Boundary

The distributed standard library and test corpus move required and explicit override bodies from
inherent blocks into conformance blocks. Unrelated inherent helpers remain in `impl Type` blocks.
Brace-less conformances and structural inherent-method satisfaction are removed rather than
deprecated. No hidden desugaring recreates the former model.

The gate closes only after repository-home and packaged-home checks agree for direct calls,
generic-bound calls, conditional conformances, interface defaults, ownership-sensitive receivers,
provenance, allocation effects, malformed source, and all editor requests.
