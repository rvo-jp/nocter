# Tuples

**v0.33.0 language contract.**

Tuples are anonymous, structural products. They group a fixed number of values when names would not
improve the meaning of the data. A tuple is not a shortened struct declaration and does not acquire
a nominal identity.

```nct
let result: (String, usize) = (text, text.len())
let name = result.0
let length = result.1
```

Use a struct when fields carry domain meaning, the value needs a stable public name, or behavior
belongs to the aggregate. `MapEntry<K, V>`, `ProcessOutput`, and other documented records therefore
remain structs even after tuples become available.

## Type and Value Syntax

A tuple type contains at least two comma-separated element types:

```nct
(String, usize)
(&str, bool, i32)
```

A tuple expression contains at least two comma-separated expressions:

```nct
(name, count)
(left(), middle(), right())
```

The comma, not the parentheses alone, identifies a tuple. `(T)` remains a grouped type and `(value)`
remains a grouped expression. Nocter does not have zero-element or one-element tuples: `()` and
`(value,)` are invalid. `void` remains the type and value for the absence of a result.

A tuple with two or more elements may retain a trailing comma. The formatter writes short tuples on
one line with one space after each comma. It writes a multiline tuple with one element per line and
a trailing comma.

## Structural Identity

Tuple identity is determined only by arity and the ordered element types. Two tuple types are the
same type exactly when every element at the same position has the same type.

```nct
(String, usize) // different from (usize, String)
```

Element names, declaration sites, and source modules do not participate in identity. There is no
implicit conversion between a tuple and a struct with the same element types.

## Evaluation and Inference

Tuple elements are evaluated from left to right, exactly once. The resulting value stores elements
in the same order.

Without an expected type, each element is inferred independently. An expected tuple type supplies
an expected type to the corresponding expression and must have the same arity.

```nct
let pair: (String, usize) = (String "name", 4)
```

## Element Access

A decimal projection selects a tuple element by its zero-based position:

```nct
let first = pair.0
let second = pair.1
```

The index is part of the source syntax and uses only ASCII decimal digits. It must not contain digit
separators, a radix prefix, a sign, or leading zeroes, and must be in range for the known tuple type.
It is not a runtime indexing operation. A tuple projection is a place, so ordinary read, move,
borrow, mutable-borrow, and assignment rules apply to it.

```nct
let view = &pair.0

var counters = (1, 2)
counters.1 = counters.1 + 1
```

Different tuple positions are disjoint places. Borrowing one position therefore does not borrow a
different position.

## Binding Destructuring

Local bindings may destructure a tuple:

```nct
let (name, length) = move result
let (head, (_, tail)) = move nested
let (_, status) = move response
```

The right-hand expression is evaluated once. The binding pattern must have the same structure and
arity as its tuple value. `_` discards one element while preserving its ordinary destruction
obligation. `var (left, right)` creates mutable local bindings; `let` creates immutable bindings.
An annotation, when present, describes the complete value:

```nct
let (name, length): (String, usize) = make_result()
```

Moving or copying from an existing place follows the same explicit ownership rules as a
non-destructuring binding. Parameter, closure-parameter, `for`, and `match` patterns remain outside
the v0.33.0 tuple contract; code may bind the tuple first and destructure it in a local statement.

## Ownership, Storage, and Destruction

Tuple ownership is structural:

- a tuple is copyable exactly when every element is copyable;
- moving the whole tuple moves every element;
- a moved element is unavailable while other unmoved positions remain independently usable;
- destruction visits all still-initialized elements in reverse element order;
- borrowing and storage provenance are derived position by position.

A tuple introduces no storage origin of its own. A result containing borrowed elements carries the
origins of those elements under the ordinary `from` rules. Editor presentation may summarize this
as an aggregate result, but that summary does not become source syntax.

## Interfaces and Behavior

Tuples have no declaration on which to place `construct`, `instance`, or `destruction` blocks.
v0.33.0 does not synthesize equality, ordering, hashing, formatting, iteration, coercion, or
interface conformance merely because the elements provide those operations. Programs that need a
public behavioral abstraction should define a named type.

Structural copy and destruction are compiler-owned value semantics, not synthesized interface
implementations.

## Layout and Calling Convention

Tuple elements use source order. Target layout computes alignment, padding, size, and element
offsets once from the ordered element types, using the same aggregate ABI rules used for ordered
struct fields. Calls and returns consume that frozen target layout; later lowering must not
recompute it from tuple syntax.

Nocter does not promise that tuple layout is interchangeable with a source struct or with a foreign
language tuple. Foreign interfaces must use an explicitly specified ABI surface.

## Tooling

Hover, completion detail, inlay hints, and diagnostics render tuple types canonically as
`(A, B, C)` from semantic type identity. Tooling must not reconstruct tuple types from source text.
Completion after a tuple-valued place's `.` lists its valid decimal positions and resolved element
types. Hovering the decimal token in `value.0` displays the canonical receiver and result type.
That token is highlighted as a property, but go-to-definition has no target because a structural
position is not a declaration. References inside tuple element types retain their ordinary targets.

The formatter preserves the distinction among grouped syntax, closure syntax, callable types, and
tuples. Numeric projections are highlighted and resolved as tuple elements, not struct fields.

## Non-goals for v0.33.0

- zero-element or one-element tuples;
- named tuple elements;
- runtime or dynamic tuple indexing;
- implicit tuple-to-struct or struct-to-tuple conversion;
- expansion of `...` into or out of tuples;
- tuple patterns in parameters, closures, `for`, or `match`;
- synthesized operators, coercions, or interface implementations;
- replacing public semantic records with positional tuples;
- a stable foreign ABI for tuple values.
