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
  relevant [standard-library contracts](../../development/std/README.md).
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

