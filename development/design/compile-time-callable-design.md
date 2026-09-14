# Compile-Time Callable Design

This document owns the compiler-internal boundary for callable evaluation during semantic
construction. Public constant-expression behavior belongs in `spec/`; this document defines how
that behavior may be implemented without creating a second type checker or making a later stage
reinterpret source.

## Problem

The current constant evaluator closes a deliberately small expression language before structural
type normalization. It receives resolved constant and type identities, produces a typed scalar or
fixed-array plan, and evaluates that plan without consulting declaration or checking internals.
That boundary is sound for literals, constant references, built-in operators, and lossless numeric
conversions.

A callable body is different. Name resolution, overload selection, generic substitution, operator
selection, ownership, control flow, and result construction belong to ordinary body checking. A
compile-time evaluator that reads callable source would have to repeat those decisions. Extending
the existing syntax planner to understand ordinary bodies would therefore create a second checker.

There is also a real dependency cycle in the current stage order:

```text
declaration type
  -> fixed-array length
  -> constant initializer
  -> compile-time call
  -> checked callable body
  -> declaration types
```

Hiding this cycle with an unevaluated placeholder, an optional constant value, or a second early
body checker would make correctness depend on which consumer happens to run first. Compile-time
callables must instead enter through an explicit dependency query whose cycles are diagnosed.

## Authorities

### Authored Capability

One callable declaration may explicitly promise compile-time callability. The promise is part of
its semantic callable contract and can be forgotten but never inferred from an implementation that
happens to use supported operations. It is independent of runtime allocation, synchronous waiting,
and deferred execution guarantees.

The first language surface is `const func` and `const method`. A compile-time callable remains an
ordinary runtime callable. Interface requirements and structural callable values use the same
capability only when the evaluator can consume their already-selected static witness; compile-time
evaluation never performs dynamic dispatch.

### Ordinary Checking

Ordinary body checking remains the sole authority for names, types, conversions, control flow,
generic substitution, overloads, operators, ownership, and dispatch. It produces the same checked
body graph for runtime and compile-time-capable callables.

Checking then projects an eligible checked body into a source-independent
`CompileTimeCallablePlan`. Projection is validation, not type checking: it accepts only checked
operations that have a defined compile-time meaning and retains their already-selected semantic
identities. Rejection points to the checked operation's source locator.

### Evaluation

`nocter-constant-evaluation` owns deterministic execution of closed compile-time plans. It receives
only values, typed operations, selected callable identities, and the compilation target where
numeric representation requires it. It cannot inspect syntax, perform lookup, select an overload,
or ask the target backend to execute code.

One `CompileTimeProgram` owns:

- evaluated constant values;
- recursively frozen static values;
- projected callable plans;
- the dependency graph among constants and callable specializations;
- memoized results for closed calls.

Recursive source call graphs are valid. Only an active evaluation cycle with no terminating value,
or a dependency cycle required to construct a declaration type, is rejected. Evaluation has an
explicit step and recursion budget so compiler resource exhaustion becomes a deterministic source
diagnostic rather than a host stack overflow.

## Dependency Queries

Semantic construction must request facts by stable identity instead of running whole stages in a
fixed order. The initial query set is:

```text
callable signature(CallableId, substitution)
checked body(BodyId, substitution)
compile-time callable plan(CallableId, substitution)
constant value(ConstantId)
static value(StaticId)
array length(ConstantExpressionId)
```

Each query records its exact dependencies and has one owner. Re-entering an active query produces
one dependency-cycle diagnostic containing the source-backed edge that closed the cycle. Completed
values are immutable and reused; no caller may bypass the query and recompute a value directly.

The query boundary is semantic and compiler-internal. The workspace computation engine may cache a
completed compilation product between editor revisions, but it does not become the authority for
dependencies inside one semantic construction.

## Initial Value Domain

The first evaluator admits values that have a target-independent semantic representation:

- `bool`, integer, floating-point, and `char` values;
- static readonly text;
- tuples and fixed arrays whose elements are recursively admitted;
- copy nominal values only after their field layout is represented as semantic aggregate values.

Owned `String`, `Vec`, `Map`, `Set`, allocator access, mutable global state, operating-system
services, asynchronous suspension, synchronous waiting, and destruction are outside the initial
domain. This is a value-domain boundary, not a list of forbidden source spellings.

## Projection Rules

The checked-body projector accepts an operation only when its complete checked meaning is
representable in the evaluator plan. In the initial implementation this includes constants,
parameters and immutable locals, primitive arithmetic and comparisons, tuples and fixed arrays,
blocks, conditional control flow, and direct calls to compile-time-capable callables.

The projector rejects allocation, mutation through aliases, borrowing of runtime storage, dynamic
or unresolved dispatch, primitives without an explicit compile-time implementation, asynchronous
construction or suspension, blocking operations, ambient storage, and destruction. Adding a new
checked-operation variant forces an exhaustive projection decision; an unclassified operation
cannot silently become compile-time safe.

## Cross-Stage Invariants

- Source syntax is interpreted once by parsing and declaration lowering.
- Names, types, overloads, operators, and dispatch are decided once by ordinary checking.
- Compile-time projection consumes checked decisions and never reconstructs them.
- Evaluation consumes only closed plans and never calls checking or target lowering.
- Constant values have one table authority; declarations and checked nodes refer to identities.
- Type construction requests constant values through the dependency query and cannot read a
  partially initialized table.
- Runtime lowering ignores compile-time plans and continues to consume the ordinary checked body.
- Editor presentation reads authored capability plus the same semantic identities used by
  compilation; it does not infer callability from source text or body contents.

## First Consumers

The first standard-library consumers are scalar helpers currently duplicated between runtime
functions and authored constant arithmetic: checked capacity arithmetic, duration-unit conversion,
ASCII classification, and platform numeric derivation. They exercise parameters, conditionals,
integer operations, direct calls, and target-width integers without requiring compile-time heap
allocation.

Compile-time table generation and copy aggregate construction follow only after the scalar path is
complete. Heap-backed collections are deliberately deferred until Nocter has a separate frozen
allocation representation rather than pretending runtime allocator state exists during
compilation.

