# Instance Coercions and Generic Evidence

This document owns the compiler architecture for v0.13.0 Phase 6. Public syntax and behavior
belong in `spec/22-borrow-coercions.md`; milestone scope and completion evidence belong in
`development/milestones/v0.13.0.md`.

## One Instance Owner

Borrow coercions are instance capabilities. `InstanceDecl` owns three semantically distinct member
collections: named methods, syntax operators, and coercion entries. Each member retains its native
AST shape and source anchors, while common callable-body consumers iterate adapters over all three.

There is no top-level coercion item and no synthetic instance declaration. The defining module and
nominal owner validation runs directly against the enclosing instance. Coherence remains global to
the type surface, so splitting one type across instance blocks or module source files cannot create
duplicate receiver/target pairs.

## Structural Requirement Identity

`where Source as Target` is its own predicate kind. Its stable identity contains canonical source
and target types; its authored representation retains the source span, exact `as` span, and target
span. It is neither type equality nor an operator requirement because the target already states the
result and conversion selection has different capability and visibility rules.

Validation restricts the source to a readonly or readwrite borrow of one visible generic parameter
and the target to a borrowed type or view. Duplicate predicates use canonical types, not source
spelling. Qualification traverses both sides with the same declaring-source context as every other
where predicate.

## Delayed Evidence

The generic type environment stores coercion requirements separately from nominal, copy,
equality, index, and expansion evidence. The ordinary conversion selector first accepts exact
compatibility and capability weakening, then an accessible concrete declaration, then matching
generic evidence. A requirement match produces a selected coercion whose authority is the authored
requirement span and whose source/target/capability facts are otherwise identical to a declared
selection.

Conversion facts preserve that authority through generic body analysis. After call-site
substitution, specialization resolves the concrete source type in its owning source context and
runs the same declaration selector for the exact target. The resulting plan contains a real
declaration span and callable target before call specialization or lowering. Failure to select a
concrete declaration is diagnosed at the generic call's evidence span.

## Consumer Boundary

Contextual conversion, explicit `as`, method-receiver fallback, comparison receiver adjustment,
and indexing receiver adjustment all call the same selector with a type environment. None inspect
where clauses directly. Ownership and provenance consume the selected plan and continue to treat
the result as borrowed from the source expression. Analysis and LSP presentation use the authored
requirement while generic, then the selected declaration for concrete navigation.

No downstream consumer searches instance blocks, compares formatted type strings, recognizes
standard-library names, or reconstructs requirement evidence from diagnostics.
