# Typed Literal Core

This document owns the compiler design for v0.3.0 Phase 1 typed literal definitions, expressions,
literal element packs, and per-literal allocation-context selection. Public semantics belong to
[Future Literal Definitions and Spread](../../spec/17-future-literal-definitions-spread.md), and the
active completion gate belongs to the [v0.3.0 Development Contract](v0.3.0.md).

## Separate Concepts

| Concept | Meaning |
|---|---|
| literal shape | compiler-defined source delimiter category such as sequence `[]` or string `""` |
| literal definition | ordinary source body attached to one nominal declaration and one shape |
| literal expression | construction syntax that resolves to exactly one visible definition |
| element pack | compiler-owned, non-escaping sequence of evaluated literal elements |
| allocation-context override | optional established aborting context selected before element evaluation |

The compiler must not infer literal behavior from nominal names such as `Vec` or `String`, method
names, private representation, or delimiter-adjacent text rewriting.

## Phase 1 Surface

Phase 1 implements only sequence and string shapes:

```nct
pub literal Vec<T> [](...items: T): Self {
    let result = Self.with_capacity(items.len())
    for item in items {
        result.push(move item)
    }
    return move result
}

pub literal String ""(text: &str): Self {
    return Self.copy(text)
}
```

The empty delimiters in a definition are shape markers, not values. A definition is keyed by
nominal declaration identity and shape. There is at most one definition for a key; parameter types,
generic arguments, and visibility never form an overload set. Different shapes on one nominal type
are distinct keys.

Typed literal expressions require whitespace between the target type and its delimiter. Thus
`Vec [1]` is a sequence literal while `values[1]` is an index expression. The distinction is
lexical and independent of capitalization or later name resolution. The formatter preserves this
single canonical spelling.

## Element Pack

`...items: T` introduces an owned ephemeral element pack. It is not `[T]`, a slice, an allocated
collection, a normal ABI parameter, or a general variadic parameter.

The pack supports only the operations needed by the Phase 1 literal body:

- `items.len()` reads the number of elements
- `for item in items` consumes elements once from left to right
- each loop binding is an ordinary owned `T`
- an unconsumed element retains its drop obligation
- the pack itself cannot be returned, assigned outside the body, stored in an aggregate, borrowed
  beyond the body, or passed to an ordinary callable

Lowering owns a `LiteralElementPack` fact rather than pretending the pack has a public Nocter type.
Diagnostics may display `literal pack of T`, but resolver and typechecker identify it by binding
identity.

## Evaluation and Cleanup

Construction order is fixed:

1. resolve and evaluate an optional `using` place
2. install the selected allocation context for the construction
3. evaluate literal elements once from left to right
4. activate a recursive drop obligation after each element completes
5. enter the literal body with the completed pack
6. transfer each consumed element obligation to its loop binding
7. publish the completed result
8. drop every unconsumed element in reverse acquisition order
9. restore the surrounding allocation context

Failure during element evaluation drops the completed prefix in reverse order. `return`, `?`, and
other body exits drop the current loop item and unconsumed elements exactly once. The pack is
compiler-owned stack/runtime state and never causes an implicit heap allocation.

## Resolution and Typechecking

- The target must resolve to a nominal type declared in the same module as the definition.
- Visibility belongs to the definition; target visibility does not implicitly publish it.
- An expression resolves by target declaration identity and source shape.
- Generic arguments are inferred from the expected result and every element, using the ordinary
  specialization engine.
- An empty sequence requires explicit target arguments or sufficient expected type information.
- A string expression passes its decoded static `&str` value to the single declared parameter.
- Sequence capture is final and unique. Phase 1 does not implement required leading parameters.
- Result provenance and allocation effect use the ordinary callable-summary model.

## Allocation Context

Without `using`, the literal body receives the current statically propagated aborting context.
`using place` selects a different established aborting allocator/context before any element is
evaluated. It does not alter the literal result type and never accepts `TryAllocator`.

Context override is a typed literal fact consumed by lowering. It is not a synthetic region, a
mutable global, a hidden source parameter, or a search for allocator names. Storage allocated by the
literal body carries the selected context's Phase 0 provenance.

## Compiler Ownership

| Responsibility | Owner |
|---|---|
| definition/expression syntax and recovery | `parser/literals` and dedicated AST nodes |
| declaration and shape identity | `resolve/literals` |
| specialization, pack rules, context validation | `typecheck/literals` |
| element ownership and escape checks | ownership consuming typed literal facts |
| pack and context operations | `ir/lower/literals` |
| editor-facing literal facts | `analysis/literals`; protocol conversion in `driver/lsp` |

AST consumers must handle literal nodes explicitly. They must not desugar them into calls before
resolution, because doing so loses source shape, pack ownership, and context-selection spans.

## Phase 1 Boundary

Sequence spread expressions, normal variadic callables, mapping/tuple/numeric/byte shapes, general
collection iteration, iterator protocols, embedding, interpolation, and recoverable literals are
not Phase 1 work. Later phases may reuse element-pack infrastructure only after defining their own
ownership and escape rules.
