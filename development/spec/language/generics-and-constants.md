# Generic Arguments, Requirements and Compile-Time Values

## Scope

This chapter owns the adopted v2.0.0 source contracts for generic arguments, requirements,
constant-required computation, constants and immutable statics. [Type foundations](values-and-types.md)
own nominal/structural identity and generic checking; [interfaces](interfaces.md) own operation
binding; [module contracts](modules.md) own publication and definition pairing. The
[grammar](syntactic-grammar.md#generic-arguments-and-requirements) owns recognition.

This is a design contract, not an implementation claim. Full generic proof, constant evaluation,
serialization and platform arithmetic must be qualified together before compiler admission.
In particular this chapter supplies no foreign primitive authority, dynamic interface object,
runtime type metadata, evaluator I/O, arbitrary compile-time allocation API or symbolic arithmetic
type solver.

## Parameter Schemas and Arguments

An ordered generic schema contains type parameters (`T`) and value parameters (`const N: usize`).
Names are unique across both kinds and cannot shadow an enclosing generic name. No defaults,
named arguments, partial lists or `_` argument placeholders are introduced. Constant parameters
other than `usize` require a separate extension. `void` is ordinary storable data and is a valid
type argument; `never` is not an arbitrary storable payload substitution.

```text
struct Fixed<T, const N: usize> {
    items: [T; N]
}

func decode<T>(bytes: Bytes): T!
let config = decode<Config>(bytes)?
```

Callable arguments belong after the callable name, including a member name. Owner arguments
belong after the owner. In `Table<Key>.from_entries<Entry>(entries)`, the two lists specialize different
schemas; each explicit list is complete for its own schema. The caller may independently omit
either list if its entire substitution can be inferred uniquely. Type-owned construction must
still deliver its owner; this example does not authorize a receiverless utility returning an
unrelated Entry.

Infer omitted arguments bidirectionally from selected receiver/input data, contextual closure
contracts and expected result data. Normalize established associated bindings to propagate those
constraints. Requirements validate that solution; they do not search implementations to guess
an unknown type. Names, later use, private body contents and allocation context supply no type
guess. An unknown or non-unique solution requires an explicit argument list or local expected
type. Do not choose a different target or passing operation to repair inference.

Generic argument syntax preserves a single source tree before its parameter kind is known. A
bare path, grouped path or array-shaped argument can have both a type and a constant-expression
reading. The selected declaration's parameter slot determines elaboration, not whether a name
happens to resolve as a type. A literal expression cannot satisfy a type slot; a type-only shape
cannot satisfy a constant slot. Reject a kind mismatch without reparsing or looking for another
generic declaration. Explicit call delimiters follow the syntactic jointness rules below.

A generic body is checked under its declared assumptions before executable specialization.
Substitution preserves selected operations and dependencies. Concrete Copy evidence at a caller
can create a snapshot into a take input; it cannot turn an already checked generic transfer into
a non-invalidating copy. Specialization creates no hidden runtime generic parameter or dictionary.

## Requirements and Associated Projections

Use `T impl Interface` for nominal capability, `copy T` for intrinsic Copy, `F: func(T): R` for
a structural callable contract and `T = Type` for type equality/refinement. `impl` does not accept
Copy, callable or execution-effect pseudo-interfaces. Associated bindings belong to their
immediately preceding interface application:

```text
where T impl Source { .Item = Item }, copy Item
```

Resolve subjects to generic parameters, `Self` where permitted by an interface prerequisite, or
their established associated projections. Reject duplicate requirements/bindings, unknown
associated declarations, contradictory equalities, non-finite normalization and requirements
that do not follow from the declared environment. Equality is semantic type equality, not a
request for layout compatibility or permission conversion. Binder refinement cannot rank
overlapping implementations. Every applicable operation/conformance must be unique over its
whole admitted domain; the compiler may reject an overlap it cannot disprove, with the competing
declarations identified. It cannot silently choose one specialization.

`T.Item` requires one owning associated declaration under one normalized interface application
in the selected capability environment. Different arguments of the same generic interface can
therefore require selection too, such as `T.(Source<Key>.Item)`.
When distinct interfaces expose Item, use `T.(Source.Item)` to select the declaration. This is
the type-position counterpart of the existing interface operation selector; it does not add a
second spelling for unique lookup. Qualification is required for a collision and rejected when
redundant. Equal normalized types do not merge distinct associated identities. A prerequisite
diamond reaching the same declaration through the same normalized application is not a collision.
The selected interface application
must already be justified; the selector cannot invent a new requirement or conformance.

Operator/coercion/expansion requirements retain their syntactic family but need each family's
closed operation contract. They carry input operations separately from ordinary data types.
Authored `noalloc`, `notrap` and `blocking` requirements are invalid. Inferred cleanup/effect
evidence is independent of Copy and must survive generic forwarding and specialization.
Declared semantic laws are not additional type equality or memory-safety evidence.

## Constant Parameters and Type Identity

A fixed-array length and a constant argument require a target `usize` value from the same
compile-time evaluator as a value initializer. A closed supported const call is not excluded
merely because the result determines a type:

```text
const func width(): usize {
    var result: usize = 1
    var index: usize = 0
    while index < 6 {
        result *= 2
        index += 1
    }
    result
}

// Ordinary implementation source; a public root would declare only the constant header.
const WIDTH: usize = width()
type Block = [u8; WIDTH]
```

Closed arguments compare by evaluated typed value. `[u8; 2 + 2]` and `[u8; 4]` have the same
array type. A visible constant parameter used alone retains its own symbolic identity until
substitution supplies the value. Arbitrary arithmetic still containing such parameters,
including `N + 1` or a const call requiring symbolic N, is not admitted in type identity by this
extension. Supporting ordinary closed evaluation is not adoption of a symbolic theorem prover.

Within a generic body N is also a readonly storage-independent value, available to ordinary
computation after substitution. It has no source storage to alias, mutate or transfer. `let n = N`
constructs an ordinary value; `read n = N` does not acquire a place. Capturing N as a constant
uses the substituted value, not a fictitious runtime generic environment field.

## const Calls and Temporary Evaluation

`const func`/`const method` promises that the same ordinary checked implementation is available
to constant evaluation. The same prefix applies to adopted typed literal, coercion and
index/current/expansion operator entries. It does not create a new operation identity or
compile-time-only body. Public/body pairs agree on the capability; an interface requirement
advertises const explicitly when its callers may depend on it. A runtime-only implementation
cannot satisfy that requirement. A const implementation may satisfy a runtime-only requirement,
but callers through that requirement do not recover the stronger promise by private lookup.
Runtime optimization may evaluate other computations when proved valid, but cannot manufacture
the public const capability. `const async` and a source-bodyless runtime primitive have no
implicit evaluator implementation and are invalid combinations.

The supported evaluator admits primitive deterministic arithmetic/conversion, unit, tuples,
fixed arrays including repetition, eligible finite record/enum construction and projection, local
`let`/`var` values, scoped read/edit access to evaluator-owned storage, replacement, blocks,
branches including enum tests/matches, tuple patterns, loops and direct calls to compatible
checked const callables. Read/edit/take input
contracts and partial initialization/cleanup remain ordinary language rules. A temporary may
be move-only; this is not permission to duplicate it or retain access after its storage ends.

Check every const body for its promised operation domain, including uninvoked declarations and
unevaluated branches. Runtime input, I/O, clocks/randomness, external mutable state, async work,
runtime-only calls, user destructors and source allocator/resource operations are outside this
domain. Internal evaluator memory is charged to compilation budgets, not a source allocation
effect or a runtime backing request. A pure checked helper name cannot supply a missing evaluator
contract. More general temporary heap resources, callable invocation and outcomes require their
own evaluator extensions before admission. Enum/pattern operations use the same checked
operations and cleanup as runtime source, not a separate constructor or pattern interpreter.

The same restrictions apply to user operation entries and implicit cleanup. A const place
projection must produce a valid place under the ordinary access/origin rules, referring to
eligible evaluator storage or explicitly supported immutable embedded data. It does not become
a value-returning function. A const expansion's cursor, selected advance/current operations and
cleanup must all have evaluator evidence. A protocol name or an intrinsic operation on another
type cannot supply a missing witness. Literal and conversion syntax grants no evaluator storage
or result-serialization exception.

A generic const body must establish const eligibility from its declared assumptions, including
interface member capabilities and selected const callable constraints. A non-const requirement
does not become const merely because one concrete implementation happens to be eligible.
Automatically exported conditional const obligations are not introduced here; their public
signature and separate-compilation contract remain unresolved. Report the unavailable selected
capability locally rather than retyping the body under a more favorable specialization.

Type checking and evaluation consume the same resolved targets, input operations and concrete
specializations. Target integer widths, overflow, division, shifts, conversion and floating-point
semantics apply; host arithmetic or host layout cannot substitute. Evaluate supported short-circuit
and conditional paths normally, while still checking the other paths structurally. Arithmetic
failure on an evaluated required-constant path is a compilation error, never runtime initialization.

[Fixed aggregate construction](tuples-and-arrays.md#repetition-and-copy) owns repetition's
once-only element evaluation, zero-count disposal and source-context Copy requirement. The
evaluator uses those same operations, including checked indexing and partial initialization;
it does not invent a pure zero-count shortcut. Final array count must be a closed valid usize.
Evaluator/resource/target implementation and production conformance remain required.

[Variadic input](variadic-inputs.md) count/index/forward operations consume the same checked
component preparation, access and remaining-entry cleanup in the evaluator. Every input/effect
must satisfy the ordinary const domain; variadic syntax cannot admit an allocator, runtime
callback/capture execution or forbidden destructor that an ordinary const call would reject.

[Synchronous traversal](iteration.md) uses the same checked source, cursor and projection/value
operations. Required CTFE needs const-capable selected implementations and ordinary access/
cleanup validity; neither a for statement nor a protocol name makes a runtime-only operation
evaluable. Frozen intrinsic array/pack plans and user witnesses share the same evaluator boundary.

## Constants, Statics and Final Values

A `const NAME: Type = expression` is storage-independent data, not an addressable owner. A
`static NAME: Type = expression` is one immutable addressable place with static extent. Their
public roots declare only name/type; one matched ordinary source supplies the initializer.
No runtime initializer, dynamic initialization order or mutable global is introduced.

Final values must be reproducible, immutable serializable data: supported scalar/unit leaves,
tuples, fixed arrays and finite nominal records/enum payloads composed of eligible leaves, without user
destructors or retained runtime resources. Any admitted static-text/reference leaf needs its
explicit immutable embedded-storage contract. Raw pointers, callable environments, exclusive
capabilities, allocator/region resources and evaluator-local addresses are not implicitly
serializable. For an enum value only its selected variant's payload exists; other variants do
not contribute fictitious resources or initialized fields. An enum-owned user destructor still
applies regardless of its variant and invalidates final admission. General outcome/view
serialization remains a separate extension. A valid
temporary operation does not make its result an eligible final value.

A constant reference constructs a fresh instance of that eligible data, like a named literal;
there is no living constant owner to move from. This permits a storage-independent record/enum
constant without asserting that its ordinary type is Copy. Each reference constructs its own
value; an ordinary binding of that value follows value-supply thereafter. This is not a general
clone operation: no final resource, destructor, outside mutable identity or evaluator address may
be replicated. Generic code acquires no Copy evidence from the existence of such a constant.
Aliasing, assigning or explicitly dropping the constant name is invalid because it is not a place.

A static reference denotes its immutable place and may be read/aliased. Extracting an independent
value still requires ordinary Copy evidence. Moving, editing or explicitly dropping static
storage is invalid; static data has no runtime user destruction. Distinct static declarations
retain distinct semantic place identities. Merging physical backing is permitted only if all
observable identity/access behavior is preserved. Constants do not acquire a unique program
address merely because their serializer shares backing.

Serialize eligible final data with the selected target layout after its semantic value/type is
closed. Endianness, alignment, aggregate encoding and permitted relocations belong to the
platform contract; an unclosed serializer cannot guess them. Qualification must inspect emitted
data and absence of runtime construction, not merely successful initializer evaluation.

## Dependencies, Limits and Diagnostics

One dependency protocol coordinates value initialization, checked const bodies, type lengths,
constant generic arguments and required layout. A query cannot use its own unfinished result.
Direct or transitive constant/type-normalization cycles are errors, including authored constant
references in unevaluated branches. A recursive callable SCC alone is not a declaration-value
cycle; a terminating const recursion may execute within evaluator limits. Calling width while
checking a length does not authorize reparsing, a second type authority or partially fabricated
header data.

Use deterministic work, memory and call-depth budgets. Required evaluation that exhausts one
reports the exhausted resource and dependency/call location; it cannot fall back to runtime
initialization or a guessed value. Budget exhaustion does not prove semantic nontermination.
Unknown runtime inputs, unsupported symbolic terms, forbidden effects, final-value escape,
arithmetic failure and dependency cycles need distinct explanations at the local cause. Normal
optimization may abandon optional folding without changing source validity; required CTFE cannot.

The evaluator and the typed-value canonicalization authority are each unique. Later layout, ABI,
native code and editor consumers use established values/identities without evaluating source
again. Effects and resource lifecycles remain proof obligations independently of staging.
