# Strict Ordering Operator Architecture

Public behavior is specified in
[Strict Ordering Operators](../../spec/24-ordering-operators.md). This document owns compiler and
distributed-standard-library responsibility boundaries.

## Common Comparison Model

`ComparisonOperatorDecl` carries a `ComparisonOperatorKind` for fixed-shape equality or strict
ordering. Each declaration adapts to an ordinary static method under a compiler-private name, so
visibility, declaration patterns, overlap checking, body qualification, specialization,
reachability, and native call targets remain shared with methods. Presentation always recovers the
authored token from semantic identity.

`OperatorRequirementShape::Comparison` carries the same kind. `TypeEnvironment` keeps equality and
ordering evidence separate because neither operation implies the other. Generic call validation
proves concrete requirements through the same selector used by expressions.

## Selection and Immutable Facts

`comparison_semantics` normalizes each surface token to a declaration kind, semantic operand
orientation, and result inversion. The common selector uses the semantic left owner, exact
declarations before one-step readonly coercions, and one contextual adjustment for the other
operand.

`TypecheckComparisonPlan` stores source-order spans and types, comparison kind, selected callable,
both conversion plans, reversed-call orientation, implicit readonly adjustment, and result
inversion. Specialization substitutes generic types and reruns the same selector. Buildability,
IR, ownership, diagnostics, and editor analysis consume the plan and never search for a token,
method spelling, or standard type name.

## Evaluation and Lowering

The synthetic call used for semantic selection follows semantic operand orientation. The runtime
call view retains source operand order. Call lowering therefore evaluates and stabilizes source
left before source right, applies each recorded adjustment once, and only then swaps the completed
ABI arguments for `>` or `<=`. Boolean inversion occurs after the static call for `<=` and `>=`.

This boundary prevents declaration selection from controlling evaluation order. It also lets the
ordinary static borrow-call ABI, conversion lowering, cleanup, and owner-loan machinery remain
authoritative. Matching integer operations keep their existing primitive IR leaves.

## Standard-Library Boundary

The validated built-in surface registry is the only authority that lets `std/str` and `std/slice`
attach instance members to `str` and `[T]`. Both algorithms are ordinary Nocter source. `String`
and `Vec<T>` reach those declarations exclusively through their existing readonly coercions; the
selector and lowerer contain no recognition of either nominal name.

## Editor Boundary

AST-backed declaration hover formats the exact comparison kind. Use-site hover, definition,
references, rename, and semantic tokens follow the selected compiler-private callable identity
back to the authored `<` span. Completion independently offers missing equality and ordering
templates. Derived surface tokens remain use occurrences of the strict-order declaration.
