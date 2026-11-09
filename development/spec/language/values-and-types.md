# Values and Types

## Type-System Foundation

This section owns the v2.0.0 foundation for type identity, compatibility and inference. Binding,
input and value-supply operations remain owned by the [Value Access Model](value-access.md);
callable invocation and dependency contracts remain owned by [Callable Values](callables.md)
and [origin rules](memory-and-regions.md#value-access-origins). These are joint checking
obligations, not interchangeable meanings of a type annotation.

### Data, Operations and Evidence

A data type describes a value's shape and supported contracts. A place additionally has an
initialization state, binding authority and identity. An expression has a known result shape
when one is established, independently of whether evaluation can complete normally. Access to
that place or expression is an operation; ordinary T is not silently qualified as read, edit or
owned T.

```text
var count: i32 = 1
let snapshot: i32 = count
read observe: i32 = count
edit update: i32 = count
```

All four declarations name i32 data. The declarations select different operations; adding or
removing their optional annotations cannot change those operations. Copy evidence determines
whether a value-supply boundary snapshots an existing place. It neither grants update authority
nor removes dependencies carried by the value. Knowing T, having a usable T value and having
permission to perform an operation on its storage are separate facts.

### Identity and Compatibility

Nominal data identity follows the selected declaration and its ordered generic arguments, where
those arguments are supported. Equal field shapes, equal layouts and equal unqualified names do
not identify distinct declarations. A transparent type alias preserves its target's identity;
it does not construct a new nominal type. Structural constructors preserve the identity and
order of their components. Storage placement and machine calling conventions do not decide
source type identity.

[Tuples and fixed arrays](tuples-and-arrays.md) own their structural shapes, required length
identity, component selection and empty-array Copy proof. Those operations do not assign
read/edit/owned qualifiers to the component data types or define the dynamic `[T]` family.

Compatibility is checked for the selected boundary. Exact data identity alone does not establish
availability, Copy, input authority, result origins or retained-state validity. Any permitted
injection, callable adapter or future erasure must be justified by its owning constructor or
boundary contract and recorded as an operation. It cannot be treated as type equality or recovered
later from an implementation body. There is no general permission-based overload search,
implicit clone, common-supertype guess or automatic runtime erasure.

In particular, a structural callable annotation constrains invocation, ordered input operations
and data, result data and dependency/state contracts. It does not identify distinct concrete
function values or erase their code/environment identity. The [callable rules](callables.md)
own permitted invocation adapters and the current static-witness joining boundary. A
heterogeneous branch cannot acquire a runtime function-pointer representation just because its
branches have compatible signatures.

### Inference and Generic Checking

Inference may fill missing data information from an initializer, an already selected expected
contract and compatible expression components. It must not revise an explicit type, a binding
operation, a named input's default operation or an earlier selected target to make later uses
succeed. A mismatch is reported at the constraint that conflicts. A missing unique data solution
requires a local annotation or an explicit type argument; it does not select an operation from
the spelling, Copy status or body use of the value. Full-language inference across remaining
generic/interface forms requires their own closed contracts before compiler admission.

A generic body is checked against its declared assumptions. Substitution preserves constructor
order, input operations, invocation contracts and carried dependencies. A caller's concrete Copy
evidence may justify its own snapshot into a taking input; it cannot change a move selected
inside a body that had no such evidence. Concrete implementation selection cannot repair a body
that was invalid under its declared assumptions.

### Known Shape Without a Produced Value

Interrupted evaluation retains a known structural result contract. It establishes neither a
runtime payload nor a code/environment witness. For example, in the bounded callable fragment:

```text
func forward(take action: func(i32): i32): func(i32): i32 { action }
func run(): i32 {
    forward(if true { return 1 } else { return 2 })(1)
}
```

The inner call has the declared callable result shape. Its argument returns from run, so neither
call commits and no callable result is produced. The outer call must still check its arity and
input/result data against the known shape. It must not demand an actual callback witness, allocate
an environment, acquire Copy evidence or schedule cleanup for that absent result.

The same distinction applies to later unreachable bindings, forwarding and compatible structural
constraints. Unknown result shape is not a wildcard data type. A known shape cannot justify a
reachable value construction or invocation without its required witness. Every actually produced
callable in the static-witness fragment retains that witness; recursive result equations still
need a reachable producer anchor.

### Scope of Closure

These foundational rules constrain later extensions; they do not admit the complete language.
The [bounded grammar](syntactic-grammar.md#bounded-value-binding-fragment) owns the implemented
source subset. The [generic/constant](generics-and-constants.md) and
[opaque-result](opaque-results.md) chapters own their selected source contracts. Full
generic/interface requirement solving, associated-type normalization, erased representations
and their joining rules, general capability schemas, effects, outcomes and recursive data
representation still require their corresponding proof and qualification before compiler
admission. No rule
for those open subjects is inherited implicitly from a released specification.

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

An optional unit can distinguish absence from presence of completion. Its construction and
branch rules belong to the [outcome contract](outcomes.md); ordinary generic substitution does not invent
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

The [grammar's origin attachment](syntactic-grammar.md#access-syntax) likewise delimits a type
before a clause on its enclosing input/result. If a callable remains open at the type's right
edge, group that complete type; an ungrouped from belongs to the inner callable's result.
Origin-name lookup cannot choose another owner. Already closed array/generic delimiters do not
require another group merely because their contents include a callable.
