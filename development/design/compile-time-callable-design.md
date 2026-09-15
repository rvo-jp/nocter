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

Program finalization projects each eligible checked body exactly once into a
`CompileTimeCallableRecipe`. The recipe uses the canonical checked node graph, retains `TypeId`
only as a reference into that exact checked generation, and freezes every already-selected
operation and call edge. This first projection validates generic bodies even when no current call
site supplies a closed specialization.

Finalization then seeds every closed non-generic compile-time root and follows its recipe call
edges. One query specializes each reachable closed `CompileTimeCallTarget` into a
source-independent `CompileTimeCallablePlan`, then publishes the closed specialization set as the
`CompileTimePlanTable` inside the checked program. A specialization key stores evaluator-domain
type shapes, not generation-relative `TypeId` values. Recipe specialization transforms only type
and call-target edges through one exhaustive operation mapping; it never reads a checked body or
repeats an operation decision. Consumers can read the resulting table but cannot request a second
projection. Rejection points to the checked operation's source locator and is mapped to authored
diagnostic `E0421` only at the exact-current source boundary.

A generic recipe is specialized when a closed root or direct call supplies its complete generic
domain. Multiple callers share that completed specialization. Recursive source calls do not form a
plan-construction dependency cycle: the caller plan completes before its target edges are queued,
and evaluation later applies the independent source-call-depth budget.

### Evaluation

`nocter-constant-evaluation` owns deterministic execution of closed compile-time plans. It receives
only values, typed operations, selected callable identities, and the compilation target where
numeric representation requires it. It cannot inspect syntax, perform lookup, select an overload,
or ask the target backend to execute code.

One checked-program `CompileTimeProgram` owns:

- evaluated constant values;
- recursively frozen static values;
- projected callable plans;
- the dependency graph among constants and callable specializations;
- memoized results for closed calls.

Recursive source call graphs are valid. Only an active evaluation cycle with no terminating value,
or a dependency cycle required to construct a declaration type, is rejected. Evaluation uses
`CompileTimeEvaluationLimits`: one positive semantic-operation step count and one positive
source-call-depth count. The default limits are 1,000,000 plan operations and 256 nested source
calls. The counts are independent of host instructions and elapsed time, so compiler resource
exhaustion becomes a deterministic source diagnostic rather than a host stack overflow. Closed
plans carry parameter and result value shapes in addition to operation-node shapes. The executor
validates every call boundary itself and memoizes only fully completed typed results, so its
correctness does not depend on an initializer adapter supplying matching arguments. Both
expression evaluation and callable execution use the same scalar-operation authority for
arithmetic, comparisons, conversions, shifts, and target floating behavior.
The plan-table transition first validates every call edge against its target plan, including
receiver/argument arity and types and the produced result type. No partially linked call graph is
published.

## Dependency Queries

Semantic construction requests facts by stable identity within typed, ordered strata instead of
placing unrelated partially constructed facts in one heterogeneous cache. The initial query set is:

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

The query state machine has only absent, active, and completed states. A computation failure
removes its active branch before returning and cannot publish a value. Plans freeze their semantic
dependency edges when they are built; evaluation requests those edges through the query rather
than rescanning plans to create a separate topological order. Short-circuit evaluation may skip an
operation's value, but it does not erase the initializer's declared dependency edge or conceal a
dependency cycle.

One query owns each completed value behind a shared immutable handle. Resolving a completed key
reuses that handle instead of cloning the value, which keeps memoization effective for frozen
arrays and later aggregate values. The query authority itself cannot be cloned; a semantic
construction therefore has one active stack and one completed-value cache.

Literal payloads belong to the expression node that spells them. A checked reference to a declared
constant instead carries `ConstantId`; checking may read the declaration's type but cannot copy its
evaluated value into the body. Target reachability collects those identities, executable closure
freezes the required values once, and MIR can obtain a declared value only from that closed table.
This keeps declaration evaluation, runtime reachability, and lowering from becoming competing
value authorities.

`DeclarationValueTable` is the sole identity-indexed authority for values already completed during
declaration construction. `ConstantDeclaration` and `StaticDeclaration` contain only semantic
metadata. `DeclarationProgram` owns only the validated graph and header-type authority; value-table
shape and payload compatibility are validated separately. The public builder still publishes only
an `AcceptedDeclarationProgram` that pairs both complete products, so no caller can treat metadata
alone as accepted compilation input. `DeclarationProgramBuilder::prepare` consumes every mutable
declaration/type builder and produces a construction-only `PreparedDeclarationProgram`. That type
can complete reserved constant and static value slots, but exposes no checking admission; its
consuming finish transition validates the full value table and publishes the aggregate. Accepted
programs and all declaration/name/body recovery
branches carry the same immutable table beside their graph; editor presentation therefore does not
need a fallback source interpreter or a value copied into presentation metadata. This separation
is the construction seam for completing non-structural initializer values after ordinary checking
without introducing optional values into declaration metadata.

Header construction assigns every distinct fixed-array length expression a dense
`ConstantExpressionId`. One heterogeneous query then resolves constant values, array lengths, and
immutable static values. A static requests the identities required by its recursive frozen type;
an array-length plan requests its referenced constants. Root scheduling deliberately begins with
statics, so successful construction proves that dependency edges—not a constants/lengths/statics
pass order—determine evaluation.

The header-value stratum closes constants, statics, and array lengths before normalized
declaration types are published. Ordinary checking then closes body recipes, and a specialization
query closes every reachable callable target. Program finalization joins the exact shared value
table and specialization table into one immutable `CompileTimeProgram`. This is intentionally not
one re-entrant query: allowing a header computation to request a checked body would expose
unfinished declaration types to checking, while allowing checking to reopen a header would make
stage order a correctness precondition. A downstream consumer can see only the completed joined
authority.

The query boundaries are semantic and compiler-internal. The workspace computation engine may
cache a completed compilation product between editor revisions, but it does not become the
authority for dependencies inside one semantic construction.

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
