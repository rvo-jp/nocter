# Strict Ordering Operators

Nocter exposes one source-defined ordering primitive: strict less-than. A type declares the
operation in an `instance`:

```nct
instance Rank {
    pub operator (&self < other: &Self): bool {
        return self.value < other.value
    }
}
```

The declaration has a readonly `&self` receiver, one named readonly `&Self` operand, a `bool`
result, and a body. An owned or readwrite receiver, another right-operand type, another result type,
an unnamed operand, or a bodyless declaration is invalid. Ordinary visibility, declaration type
patterns, and declaration-wide `where` clauses apply.

The declaration defines a strict comparison operation. Its existence alone does not promise that
every pair is comparable or that the relation is a total order. A standard interface may impose
those stronger algebraic requirements on generic consumers that need them.

## Derived Comparisons

One selected `<` operation defines every ordering token:

```text
left <  right  =  less(left, right)
left >  right  =  less(right, left)
left <= right  =  less(left, right) || equal(left, right)
left >= right  =  less(right, left) || equal(left, right)
```

The operands are always evaluated exactly once from left to right as written. Reversing the
strict-order call for `>` or `>=` does not reverse source evaluation. `<=` and `>=` require both
applicable `<` and `==` operations and short-circuit equality when strict comparison succeeds.
This keeps unordered values such as NaN unordered instead of treating incomparability as less than
or equal.

The selected call borrows its operands and does not consume their owners. Existing borrow and
coercion syntax remains explicit at ordinary call boundaries. A caller may compare existing
readonly borrows directly; a nominal type with a direct declaration may also be compared through
the operator's implicit readonly operand adjustment.

## Generic Requirement

Generic code requires the same structural operation:

```nct
func earlier<T>(left: &T, right: &T): bool where (&T < &T): bool {
    return left < right
}
```

The requirement proves strict ordering for that exact operand pair and a `bool` result. It adds no
runtime witness or dispatch value. Concrete specialization must find primitive integer ordering,
an accessible source declaration, or a declaration reached through one readonly coercion.

## Total-Order Contract

The standard `TotalOrder` interface makes the stronger algebraic promise explicit:

```nct
pub interface TotalOrder where (&Self == &Self): bool, (&Self < &Self): bool {}
```

An implementation promises that `<` is irreflexive and transitive, equality is consistent with the
order, and every pair is comparable. Interface prerequisites provide the two operations to generic
code; they do not prove those laws. Implementations remain explicit and the compiler does not
create one merely because both operators exist.

Generic algorithms that require deterministic ordering use `where T impl TotalOrder`. The standard
in-place slice sort follows this rule. Integers, Unicode scalar values, borrowed text, and owned
strings implement the contract. Floating-point types do not because their ordinary NaN comparison
is partial.

The shared `Ordering` enum has `less`, `equal`, and `greater` variants. A named comparison API may
return it without changing the meaning of source operators.

## Selection

For the semantic left operand, selection uses this order:

1. matching integers use their primitive operation;
2. an accessible declaration on the exact owner is selected;
3. otherwise, one accessible readonly coercion may reach an owner with an applicable declaration.

The semantic left operand is the source left operand for `<` and `>=`; it is the source right
operand for `>` and `<=`. The other operand must match the selected `&Self` input directly or
through one readonly coercion. Coercions do not chain or receive implicit rankings. Multiple
distinct viable targets are ambiguous and require an explicit `as` conversion.

`>` , `<=`, and `>=` cannot be declared independently. This prevents four definitions for one
order from disagreeing.

## Standard Types

Matching integer types retain primitive ordering. `bool`, payloadless enums, and arbitrary structs
do not receive generated ordering or `TotalOrder` implementations.

Source-defined ordering for borrowed text and slices, including coercion from their owning
containers, belongs to the compiler-checked [`std/str`](../../development/std/str/index.nct) and
[`std/slice`](../../development/std/slice/index.nct) contracts and their assigned behavior guides.
Those declarations use the selector defined here. The `<` operator does not prescribe a sorting
algorithm.

## Tooling

Formatting preserves the authored `<` declaration and requirement forms. Hover presents the
selected declaration with its concrete owner. Definition, references, and rename from any of `<`,
`>`, `<=`, or `>=` use the identity of the selected `<` declaration. Semantic tokens classify the
authored operator token as a method declaration and both operand bindings as readonly parameters.
Compiler-private callable names are never public source or editor labels.

## Non-goals

Strict comparison does not define a total-order proof, three-way comparison values, comparator
callbacks, or hashing. Equality remains an independent operation, although inclusive comparisons
require it. Floating-point comparison is defined in [Floating-Point Values](floating-point.md).
The existence of a standard sorting consumer does not make sorting part of operator selection.
