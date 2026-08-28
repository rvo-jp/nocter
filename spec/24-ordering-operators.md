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

The implementation promises a strict total order. It must be irreflexive, transitive, and
consistent for every pair of values. These algebraic properties are an API contract; the compiler
does not execute additional comparisons to verify them.

## Derived Comparisons

One selected `<` operation defines every ordering token:

```text
left <  right  =  less(left, right)
left >  right  =  less(right, left)
left <= right  =  !less(right, left)
left >= right  =  !less(left, right)
```

The operands are always evaluated exactly once from left to right as written. Reversing the
strict-order call for `>` or `<=` does not reverse source evaluation. `<=` and `>=` negate one
strict-order result; they do not call equality or execute ordering twice.

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
do not receive generated ordering.

The standard `str` instance compares UTF-8 encoding bytes lexicographically. `String` uses its
readonly coercion to `str`; no second string comparison algorithm exists.

The standard `[T]` instance compares elements lexicographically under
`where (&T < &T): bool`. If one slice is a prefix of the other, the shorter slice is less. `Vec<T>`
uses its readonly slice coercion and does not own a duplicate ordering declaration.

The same requirement powers the standard readwrite-slice `sort` method. That collection API is
specified separately in [Practical Standard Library](21-practical-standard-library.md); the `<`
operator selects the order but does not prescribe the sorting algorithm.

## Tooling

Formatting preserves the authored `<` declaration and requirement forms. Hover presents the
selected declaration with its concrete owner. Definition, references, and rename from any of `<`,
`>`, `<=`, or `>=` use the identity of the selected `<` declaration. Semantic tokens classify the
authored operator token as a method declaration and both operand bindings as readonly parameters.
Compiler-private callable names are never public source or editor labels.

## Non-goals

Strict ordering does not define partial ordering, three-way comparison values, floating-point
behavior, comparator callbacks, hashing, or equality. Equality remains an independent operation.
The existence of a standard sorting consumer does not make sorting part of operator selection.
