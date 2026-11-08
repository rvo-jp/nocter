# Equality, Indexing, and Core Operators

This chapter defines equality and indexing declarations, built-in logical and unary operators, and
their relationship to the shared expression grammar. Strict ordering has its own
[owning chapter](ordering.md).

## Operators, Comparison, and Precedence

Nocter has a closed operator grammar. An `instance` may define equality, strict ordering,
readonly/readwrite indexing, and readonly/readwrite/owned expansion with fixed declaration shapes.
Equality uses:

```nct
instance Text {
    pub operator (&self == other: &Self): bool {
        return self.bytes == other.bytes
    }
}
```

The declaration shape is fixed: both operands are readonly borrows of the same `Self` type and the
result is `bool`. It may be private or use an ordinary `pub` boundary. `!=` cannot be declared; it
negates the selected `==` result. Nocter does not derive structural equality.

Readonly and readwrite indexing use separate declarations:

```nct
instance Buffer<T> {
    pub operator (&self[index: usize]): &T {
        return &self.values[index]
    }

    pub operator (&+self[index: usize]): &+T {
        return &+self.values[index]
    }
}
```

An index declaration always returns an element borrow. It therefore defines a place, not a
value-producing or partial lookup operation. Use an ordinary method returning `&T?` for partial
lookup. The declaration body owns bounds and failure policy; the compiler does not add a second
bounds check around a source-defined operation.

Generic equality and indexing requirements may constrain immediate execution with `noalloc` and
`notrap`, for example `where noalloc notrap (&T == &T): bool` or
`where noalloc (&C[usize]): &V`. Primitive equality carries both guarantees. Built-in indexing is
allocation-free but may trap on an invalid index, so it cannot satisfy a `notrap` requirement.
Source-defined operations and implicit operand or receiver coercions are checked independently
against every requested guarantee.

Arrays, slices, and `str` are directly indexable. For a nominal receiver, selection first checks an
accessible declaration on the original type. If none applies, selection may use one accessible
borrow coercion whose target is directly indexable or owns an applicable index declaration.
Coercions do not chain, and multiple viable coercion targets are ambiguous. Readonly access uses a
readonly operation; assignment and `&+values[index]` require a readwrite operation and writable
receiver storage.

Comparison rules:

- Primitive equality is available for `bool`, matching integer types, and matching payloadless enum
  types.
- Nominal and view equality selects an accessible equality declaration from the left type.
- Equality may apply one readonly borrow coercion to each operand. An exact left declaration wins
  before coerced candidates; multiple remaining coercion candidates are ambiguous.
- Owned operands are implicitly borrowed for the selected readonly equality call and remain usable.
- Standard text and collection equality declarations and their coercion behavior belong to the
  relevant [standard-library contracts](../../std/README.md).
- Struct equality is not automatically generated.
- Payload-carrying enum equality is not supported. Use `match` or `if expr is Pattern`.
- `<`, `<=`, `>`, and `>=` are ordering comparisons.
- Matching integer operands have primitive ordering.
- Other types may own strict ordering through
  `operator (&self < other: &Self): bool`; the complete declaration, generic-requirement,
  derivation, coercion, and evaluation rules are specified in
  [Strict Ordering Operators](ordering.md).

Logical rules:

- `&&` requires `bool` operands and returns `bool`.
- `||` requires `bool` operands and returns `bool`.
- `&&` short-circuits: the right operand is evaluated only when the left operand is `true`.
- `||` short-circuits: the right operand is evaluated only when the left operand is `false`.
- `!expr` requires `expr: bool` and returns `bool`.

Unary numeric rules:

- `-expr` requires a signed numeric operand.
- Unary `+expr` is not part of the language.

The complete precedence, associativity, move-place, outcome-suffix, recovery, and primary-expression
grammar is centralized under [Expression Precedence](syntactic-grammar.md#expression-precedence).
In particular, unary borrowing binds before `as`, one ungrouped expression layer accepts one
outcome suffix, and `move place?` moves the complete place before applying that suffix.

Assignment remains a statement rather than an expression. `..<` remains confined to a range
`for` header. `if` and `match` are primary control expressions. `condition ? then : else` and
`enum_expr ?{ ... }` have no productions. `&&`, `||`, `otherwise`, `if`, and `match` evaluate only
the required operand, fallback, branch, or arm.

Example:

```nct
if count > 0 && state == ScanState.inside_word {
    ...
}
```

## Future Direction: v2.0.0 Core Primitive Equality

Initial primitive `==`/`!=` compare matching `bool`, `i32`, or `void` payloads and construct a
Boolean value. Reading an operand does not transfer its owner. Unit equality is true and inequality
false after evaluating both operands in source order. Do not remove their effects/access validation.
Opaque `error` has no primitive equality. User-defined, compound and interface equality use their
own full-language contracts; this bounded foundation does not derive structural equality for them
or change the released domain above. The earlier prefix-based private fragment is superseded.

## Future Direction: v2.0.0 Index Projections

An index operator exposes an existing element place, rather than returning an ordinary value.
Its declaration names the element data type; the receiver form selects read or edit projection:

```text
impl Items<T> {
    operator (self[index: usize]): T from self
    operator (edit self[index: usize]): T from self
}
```

These are two fixed index-operation identities, not permission-ranked overloads. Context selects
the required read/edit projection before checking authority and then selects that one declared
operation. A missing edit authority/operator is an error; it does not fall back to another body.
A read use requests the read projection; an edit alias/input, replacement or compound target
requests the edit projection. A value-producing use requests a read projection followed by the
ordinary Copy-or-move supply check. The projected place itself carries access authority, not
ownership of the container element; non-Copy extraction needs a checked exchange/removal API.
There is no automatic copy, hole in the container or silent conversion to an owning element.

```text
let snapshot = numbers[i]   // Copy numeric element into a value
read observed = items[i]   // retain read access to the existing element
edit working = items[i]    // retain update access to the existing element
items[i] = replacement     // RHS first, then edit projection and replacement
```

Each operator body supplies an initialized place with `return place` (or its final place result),
checked against the projection kind and origin bound. This is the defined result protocol of an
index operator, not an alternative result permission on `func` or `method`. A computed fresh value
is not a projected place. Nested ordinary value construction still follows Copy-or-move.

The index operator evaluates receiver/index inputs once in their selected order. It identifies
and stages the place/access, runs normal operator-local cleanup, and hands off the checked
projection. It does not suspend or later resume, and introduces no `yield` or implicit coroutine.
The place and any guard/structure needed for its validity must survive that cleanup. Reject a
projection into a dead local, consumed index input or revoked local guard; do not postpone cleanup
to make it valid. Child access remains rooted in the surviving receiver/input, with conflicts and
last use handled by the same access graph as `read`/`edit`.

A projected element may itself contain dependencies. Copying its descriptor retains them; an
exclusive capability cannot become Copy through indexing. Root and range identities must be
precise enough for disjointness, and every potentially invalidating resize/relocation conflicts
while needed access remains. Unknown invalidation is conservative, not hidden address repair.

Primitive tuple/field/array projections retain their owning/subplace rules and are not invented
operator calls. Exact array extraction/partial-initialization rules and the library index ABI must
be closed before native integration; this source decision does not prove their implementation.
