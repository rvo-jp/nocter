# Borrow Coercion Compiler Boundary

This document owns the compiler design for v0.8.0 borrowed-view coercions. Public syntax and
behavior belong in the specification; milestone scope and completion evidence belong in the
[v0.8.0 milestone](../milestones/v0.8.0.md).

## Responsibility Model

Borrow coercion is a statically resolved call, not type equality and not an IR cast. Contextual
compatibility and explicit `as` share conversion selection; only the source of the target type
differs.

```text
coerce declaration
  -> resolver declaration identity and coherent key
  -> typechecked callable summary
  -> contextual or explicit conversion selection
  -> immutable ConversionPlan
  -> ownership, regions, analysis, and IR lowering
```

No consumer after typecheck searches declarations or recognizes source and target names.

## Declaration Identity

The coherent key consists of the source nominal declaration identity, receiver capability, and
canonical target type under the declaration's generic parameters. Visibility is attached to the
entry. Source and target spans remain available for diagnostics and semantic occurrences, but text
does not define identity.

Only the source type's defining module may add entries. This rule prevents import order and package
composition from changing the implicit conversion set.

## Conversion Selection and Plans

One selector accepts a mode, concrete source type, concrete target type, source expression, and
resolved type surface. Its result kind is exact compatibility, lossless integer conversion,
capability weakening, or a selected borrow coercion. Contextual checking considers coercion only
after exact compatibility fails. Explicit `as` accepts the existing lossless integer rule,
capability weakening, or one accessible exact coercion; it does not accept arbitrary redundant
casts.

Every non-trivial successful selection produces an immutable `ConversionPlan` containing:

- conversion kind
- concrete source and target types
- complete expression and source spans
- the explicit operator span when one exists

A borrow-coercion kind additionally records:

- coercion declaration and source-module identity
- concrete source, receiver, and target types
- generic arguments substituted from the source nominal type
- receiver capability and any built-in capability weakening
- result provenance instantiated from `self`
- source and expectation spans
- concrete callable target used by lowering

The plan is keyed by the semantic conversion boundary. Contextual leaves use their expression span;
explicit selection uses the complete type-conversion expression and separately retains its source
and `as` spans. Existing coercion consumers obtain the nested call plan from the conversion fact,
so there is no parallel lookup table or compatibility algorithm.

## Expected-Type Structure

Fact collection pushes concrete expectations through groups and value-producing `if`, `if is`, and
`match` branches before selecting a leaf conversion. Typed-sequence declarations provide their
instantiated capture element type. Enum constructors provide payload expectations through the same
resolved owner, variant, and generic substitutions used by type checking. Optional and fallible
projection retains the outer selected result boundary.

This ordering prevents both an outer compound plan and duplicate inner plans. It also ensures the
native control-flow lowering writes the selected result into one destination while each executed
branch invokes exactly one coercion.

## Ownership and Provenance

The caller supplies a borrowed source. Contextual source code writes that borrow at the producing
site; explicit `as` requires it in the source expression or source value type. Selection may
reborrow `&+Source` as readonly, but it never creates a borrow from an owned source. The returned
view carries the original source loan and uses the same non-lexical lifetime and region escape
machinery as an explicit method call.

Borrow-source collection follows every borrow-valued result expression, including explicit type
conversion, optional and fallible projection, `if`, `if is`, and `match`. A binding records every
possible source place from its executed result path. Later move, drop, assignment, and conflicting
borrow checks therefore use the binding's last use without treating coercion as a lifetime boundary.
Pattern branches are analyzed in their branch-specific type environments so payload-derived borrows
retain their actual source instead of becoming unknown.

A readwrite result requires a readwrite receiver. Returning mutable access to invariant-bearing
storage remains the defining type's responsibility; `String` therefore exposes only `&str`, while
`Vec<T>` may expose `&+[T]`.

## Lowering

IR lowering evaluates the source once, materializes the receiver according to the plan, and invokes
the concrete coercion body through the ordinary static-call ABI. Coercion does not create a target
value by reinterpretation and does not bypass callable provenance or cleanup.

Borrow-valued calls, optional/fallible projections, and value-producing control flow materialize
through the common borrow destination path. Coercion lowering supplies the selected call but does
not own special evaluation rules for those expression shapes.

## Editor Boundary

AST source ranges feed semantic occurrences for `coerce`, the source type, `self`, `as`, the target
type, and `from self`. Hover and completion render normalized notation from the declaration model.
Definition, references, and rename use resolver identities. LSP transport only converts positions
and presentation blocks.

An explicit conversion creates editor information from `ConversionPlan`. Hover focuses the exact
`as` span and renders concrete normalized types. Definition is present only for a borrow-coercion
kind and targets the entry's declaration identity; numeric and capability-only plans have no
invented declaration.
