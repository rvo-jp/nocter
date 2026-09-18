# Value Provenance Contract Boundary

## Purpose

This document owns the compiler boundary for source-level `from` contracts. Public syntax and
behavior belong to the language specification; crate-local storage belongs to the relevant crate
README.

`from` qualifies a value crossing a checked boundary. It is not part of nominal or structural data
type identity, does not declare a lifetime variable, and has no runtime or ABI representation.

## One Semantic Model

The declaration layer publishes normalized value contracts whose origins are semantic receiver or
parameter identities. Structural callable types publish the same relation using parameter
positions. Body-local annotations resolve to checked place roots. These are three identity domains
for one relation, not three independently interpreted features:

```text
provenance(target) ⊆ provenance(source 1) ∪ ... ∪ provenance(source n) ∪ independent
```

The result is a distinguished target supplied by invocation. A receiver, parameter, or local
binding is an input target. `static` contributes no caller-managed source place; an empty external
origin set therefore satisfies any `from` upper bound.

Checking owns containment. Declaration lowering resolves names but cannot prove value flow;
analysis and presentation consume the checked relation and cannot rebuild it from source syntax.

## Header Resolution

One callable header is resolved as a closed graph:

1. reserve the callable, receiver, and every ordinary or pack parameter identity;
2. resolve all parameter and receiver types;
3. resolve every `from` source against the complete reserved input set;
4. reject missing, duplicate, and directly tautological edges;
5. canonicalize the complete graph and interpret strongly connected components simultaneously;
   and
6. freeze input constraints and the result contract with the callable declaration.

No clause may observe whether another clause appeared earlier in source. A cycle is not rejected
merely because it is cyclic: mutually contained inputs form one equality component. The normalized
graph and its one all-branches implication operation, not traversal order or one successful path,
decide containment.

## Precision

A constrained input retains its own symbolic origin. The checker does not replace `value` with the
origins named by `value from owner`; doing so would make a returned `value` unnecessarily borrow
all of `owner`. Instead, the constraint graph proves that `value` is contained by `owner` only when
a destination contract requires that fact.

At a call, each actual input provenance is checked against the union of its named actual source
values. Result mapping then uses the actual argument selected by the result contract. This keeps
loans no longer than the exact value flow requires.

## Structural Callables

Input constraints participate in a structural callable contract. Conversion may forget a
constraint only when doing so cannot let the callee assume a relationship the caller did not
prove. Closure contextual checking, static callable witnesses, erased callable construction,
interface implementation matching, and generic callable predicates all use the same compatibility
operation.

The relation is normalized by parameter position. Parameter spellings never enter type identity.

## Local Values

An annotated local contract is checked when its initializer is committed. Its origins resolve
through ordinary lexical name evidence to checked place roots. Assignment to a constrained
mutable binding repeats the same containment check. Tuple-pattern annotations constrain the
complete aggregate value.

Local contracts do not escape into `TypeId`, and source projection cannot affect their proof.

## Ownership Boundary

`from` can constrain a new value but cannot retroactively add storage origins to an existing
owner. An operation that changes an owner's provenance consumes the old owner and returns a new
one:

```nct
method self.push(value: &str): Self from self | value
```

This form gives ownership, provenance, cleanup, and loans one explicit state transition. Direct
self-reference and address stability require separate storage mechanisms and are not inferred from
`from`.

## Downstream Boundary

`CheckedProgram` freezes resolved constraints, call-site proofs, result mapping, and loans.
Target, MIR, Machine, native code generation, and LSP projection receive closed facts only. They do
not inspect syntax, compare names, solve provenance again, or change layout.
