# Generic Coercion Evidence

This document owns the compiler architecture for structural `where Source as Target` evidence.
Concrete declaration selection, conversion plans, ownership, and lowering remain owned by
[Borrow Coercions](borrow-coercions.md). Public syntax and behavior belong in
`spec/22-borrow-coercions.md`.

## Predicate Identity

The requirement has a dedicated AST node containing canonical source and target types plus the
authored source, exact `as`, and target spans. It is not encoded as type equality or an operator:
the right side already states the result, and coercion capability and visibility rules are
independent of operator selection.

Validation accepts `&T` or `&+T` for one visible generic parameter and a borrowed target. A
readonly source cannot promise a readwrite target. Qualification traverses both sides in the
declaring source context, duplicate detection uses canonical type identity, and formatter, JSON,
semantic tokens, hover, and type occurrences consume the node directly.

## Delayed Authority

`TypeEnvironment` stores coercion evidence separately from nominal, copy, equality, index, and
expansion evidence. Contextual conversion, explicit `as`, receiver-method fallback, comparisons,
and indexing all call the ordinary coercion selector with that environment. A requirement match
produces the same selected conversion shape as a concrete declaration but records the authored
requirement span as temporary authority.

After generic substitutions are known, call specialization resolves the concrete source type
across its owning source contexts and reruns the ordinary declaration selector for the exact
target. The resulting immutable plan replaces requirement authority with the real declaration
span, callable target, receiver capability, and owner substitutions before lowering. A missing or
inaccessible concrete declaration is diagnosed at the generic call.

No runtime witness, requirement table, nominal-name rule, source-text comparison, or second
coercion algorithm exists. Once specialized, ownership, provenance, regions, editor navigation,
and IR consume the ordinary concrete plan.
