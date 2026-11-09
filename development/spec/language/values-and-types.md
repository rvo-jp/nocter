# Values and Types

## Storable void

For v2.0.0, `void` has exactly one information-free value, written `()`. It is a Copy value,
usable in bindings, parameters, fields, arrays, generic arguments, associated types, and outcome
payloads wherever the surrounding type constructor admits ordinary data. Aliases preserve that
identity; a separate `Unit` or zero-element tuple type is not introduced.

```text
let complete: void = ()
```

Normal completion and bare `return` still supply unit without writing the literal. Evaluating a
`void` expression still performs its effects. A zero-byte payload does not make the producer pure,
remove initialization requirements, or change the distinct places used by access checking.

An optional unit can distinguish absence from presence of completion. Its exact construction and
branch rules belong to the future outcome contract; ordinary generic substitution does not invent
a second unit value. `never` remains distinct: it describes a path producing no normal result.
The adopted storable-unit direction does not broaden `never` to arbitrary data-bearing positions.

`()` is value syntax; `void` is its type name. Do not add `void()` or `unit()` constructor aliases.
Unit/closure recognition is defined by the
[future grammar](syntactic-grammar.md#access-syntax).

## Type Constructor Scope

The adopted future [Value Access Model](value-access.md) retires access qualifiers on ordinary
data types. `&T`/`^T` are not alternative forms of T; binding/input operations are separate.

Capability types describe their own operations and dependencies as ordinary types. An optional
capability and a capability accessing an optional slot are different constructor trees, such as
illustrative `ReadView<File>?` and `ReadView<File?>`. These names are library API examples,
not newly declared built-ins. Neither permits extracting a non-Copy value through read access.
An update capability is non-Copy independently of its element type.

`File!` delivers a successful File value or the one error failure. `File??` preserves distinct
optional layers. Ordinary optional injection remains possible, including `let a: i32? = 10`;
`some(...)` explicitly constructs a presence layer when that distinction matters. Preserve
constructor order through aliases, generic substitution, matching, display and formatting.
Ungrouped `T!?` and `T!!` are not introduced; broader fallible nesting remains separate work.

Callable results consume their complete type: `func(i32): File!` has a fallible call result,
while `(func(i32): File)!` is a fallible value containing a callable. The same distinction applies
to `future T`, pointer operands and grouped type constructors. Type recognition does not prove
lifetime validity, permit moving a payload from read storage, or erase descriptor dependencies.
