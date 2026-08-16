# Expansion Operators

Expansion operators let a type define how its readonly, readwrite, and owned values become
iterators. Collection `for` and sequence spread use the same operator declarations. Expansion is
not an interface conformance and does not depend on a method named `iter` or `into_iter`.

## Declarations

Expansion operators belong in an `instance` declaration:

```nct
instance Buffer<T> {
    pub operator (...&self): BufferIter<T> {
        return BufferIter.from_view(self.view())
    }

    pub operator (...&+self): BufferIterMut<T> {
        return BufferIterMut.from_view(self.view_mut())
    }

    pub operator (...self): BufferIntoIter<T> {
        return BufferIntoIter.from_buffer(move self)
    }
}
```

The receiver capability is part of the operator identity:

- `...&self` expands a readonly borrow without transferring the source.
- `...&+self` expands an exclusive readwrite borrow without transferring the source.
- `...self` consumes the source.

An expansion operator takes no ordinary parameters and its return type must conform to `Iterator`.
Its body, visibility, generic declaration pattern, result provenance, and source-module rules are
the same as those of other `instance` members. A type may declare any subset of the three forms.

Expansion syntax is selected only by collection iteration and typed-sequence spread. `...value` is
not a general expression and cannot be called explicitly. An ordinary named method may expose the
same implementation when direct iterator construction is useful.

## Generic Requirements

A generic function states expansion and iterator behavior separately:

```nct
func visit<C, I>(source: &C): void where (...&C): I, I: Iterator {
    for item in &source {
        inspect(item)
    }
    return
}
```

The available requirement shapes are:

```nct
where (...&C): I
where (...&+C): I
where (...C): I
```

The result type is exact. The compiler may infer `I` by selecting the concrete source operator at a
call site. `I: Iterator` and associated-type equalities remain separate requirements because the
expansion predicate proves only the conversion result.

## Collection Iteration

Source syntax selects one capability:

```nct
for item in &values { inspect(item) }
for item in &+values { update(item) }
for item in move values { consume(move item) }
for item in move iterator { consume(move item) }
for item in make_iterator() { consume(move item) }
```

Readonly and readwrite forms select the corresponding expansion operator. For `move source`, a
source type that directly conforms to `Iterator` is used as that iterator; otherwise the form
selects the owned expansion operator. Direct conformance has fixed priority when a type provides
both. The final form above is a newly produced direct iterator and performs no expansion.

A bare direct-iterator expression follows ordinary ownership rules. A new temporary is already
owned by the loop, a copyable iterator place is copied, and an existing move-only iterator place
requires `move`. A bare collection is rejected rather than guessed as readonly or consuming.

`move` remains the ordinary place-only move expression in this grammar. A newly produced collection
cannot be written as `for item in move make_values()`. Bind it first and move that binding. A newly
produced direct iterator needs no prefix and remains valid as `for item in make_iterator()`.

The source expression is evaluated once. The resulting iterator is advanced through its selected
`Iterator.next` declaration. Absence ends the loop without initializing an item. Cleanup for
normal completion, `continue`, `break`, `return`, and propagation follows the ordinary ownership
and drop rules.

Readwrite expansion holds the exclusive source loan for the iterator lifetime. A typical iterator
has `Item = &+T`. Each loop body receives one element loan, and that loan must end before the next
step. The source cannot be accessed independently while the iterator remains live. A yielded
borrow may escape only when the ordinary provenance and region rules permit it.

## Sequence Spread

Typed sequences select the same operators:

```nct
let copied = Vec [0, ...source, 4]
let borrowed: Vec<&T> = Vec [...&source]
let owned = Vec [...move source]
```

- `...source` and `...&source` select readonly expansion.
- `...move source` accepts a direct owning iterator when the source type conforms to `Iterator`;
  otherwise it selects owned expansion. Direct iterator conformance has fixed priority when both
  are present.
- The operand of `...move` must be an eligible existing move-only place. A call, literal, or other
  newly produced temporary must first be stored in a binding.
- Every spread iterator must also conform to `ExactSizeIterator`.
- A directly selected iterator that lacks `ExactSizeIterator` is rejected; selection does not fall
  back to an owned expansion.
- Bare spread copies readonly yielded referents and therefore requires `copy` elements.
- `...&source` contributes the yielded readonly references themselves.
- `...&+source` is rejected.

Mutable spread is intentionally unsupported. A literal pack may retain every resulting element at
once, which requires a stronger disjointness proof than the one-at-a-time mutable loan used by a
collection loop.

## Tooling Contract

Diagnostics, hover, completion, definition, references, semantic tokens, formatting, and AST JSON
present the authored `operator (...receiver): IteratorType` form and its exact operator span.
Compiler-private callable identities are never source API and must not appear in editor output.
