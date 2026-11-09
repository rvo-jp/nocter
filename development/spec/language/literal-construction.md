# Typed Literal Construction

## Scope and Selected Entry

This chapter owns typed sequence, keyed and non-interpolated string construction. These are
type-owned value-producing calls, not compiler-recognized collection names, field initialization,
implicit iteration or dynamic `[T]` values. [Type behavior](type-behavior.md) owns the declaring
nominal and [module pairing](modules.md#complete-implementation-signatures) owns public/body
agreement. This is an adopted source contract, not implemented compiler support.

Each owner has at most one entry for each shape: `literal []`, `literal [:]` and `literal ""`.
Result type, input operation, requirements and expected context cannot select between overloads.
Aliases resolve to the same owner and entry. Owner generic arguments follow the ordinary
[generic rules](generics-and-constants.md); an entry introduces no additional generic list.

```text
impl Batch<T> {
    literal [](... take items: T): Self!
}
impl Fields<K, V> {
    literal [:](... take entries: K: V): Self!
}
impl Label {
    literal ""(text: str): Self!
}

let batch = Batch<Job> [first^, second^]?
let fields = Fields<Key, Value> [key^: value^]?
let empty = Fields<Key, Value> [:]?
let expanded = Fields<Key, Value> [: ...pairs^]?
let label = Label "ready"?
```

These examples assume the independently declared owner, component types and ordinary checked
bodies. Their punctuation grants no allocator, collection, validation or string-layout privilege.
The result must construct the specialized owner, with the same permitted optional/fallible
wrappers as a type-owned construction function. It is ordinary result data; a fallible entry
does not insert `?` for its caller. Async and opaque-result entries are not introduced.

## Sequence and Keyed Inputs

A sequence entry has exactly one final ordinary pack, with one named component type. A keyed
entry has exactly one keyed pack, with its name and key/value types. Neither has fixed prefix
inputs, a receiver, defaults, hidden inputs or an implicit allocator parameter. Each can select
read, edit or take for its pack. The operation is authored once in that declaration; a take
entry does not grant permission to retire unmarked non-Copy source arguments.

```text
impl Snapshot {
    literal [](... values: i32): Self
}
impl Adjusted {
    literal [](... edit values: i32): Self
}
```

Read inputs observe existing storage; edit inputs require update authority for direct places.
Produced or explicitly transferred values may lend the receiving operation through caller-owned
temporaries. Take receives snapshots from unmarked Copy sources, explicitly marked owner
transfers, or produced values. No selected entry repairs a missing authority by cloning, changing
its operation, or trying another shape.

After an owner and separating gap, `Owner []` selects the ordinary shape and `Owner [:]` selects
the keyed shape even when empty. A nonempty keyed body contains `key: value` contributions;
an ordinary body contains ordinary values. Mixing the two kinds is invalid. A body consisting
only of spreads selects the ordinary shape. To select keyed expansion without a direct keyed
contribution, extend the empty keyed marker: `Owner [: ...pairs]`. After that leading colon,
only spread contributions are valid. Direct keyed contributions already select their shape
and cannot take a redundant leading colon. This gives each shape one spelling without inspecting
the owner or a spread's inferred type to reparse the body. Tuple values are ordinary entries,
not an alternate direct keyed spelling.

Contributions, including `...source` and `...source^`, follow the
[variadic call rules](variadic-inputs.md#calls-and-built-in-expansion) exactly. A keyed contribution
prepares key before value. All contributions preserve written order and exact component/root
identities. Duplicate keys are separate entries; the entry's checked body decides whether its
product permits them. There is no implicit hashing, comparison, deduplication or sorting.
Spreading uses the selected built-in or user expansion contract, never a method-name guess.

Generic inference uses only the selected public input/result contract and ordinary expected
data. An empty literal supplies no missing component-type guess. `Owner[index]` is instead a
joint index projection; whitespace is syntax, not an authority or type-dependent disambiguator.

## String Input

A string entry has exactly one named, unmarked, non-pattern input of type `str`. It receives read
access to the decoded string value. It has no pack, receiver, additional inputs or implicit
formatting call. The token's escape decoding follows the explicitly inherited
[string-token contract](../../../spec/language/lexical-grammar.md#string-character-and-byte-literals),
without executing an interpolation or a user conversion. The string's immutable UTF-8 contents
have program extent;
no source descriptor or evaluator-local address is a surviving root.

The entry may retain only dependencies that its ordinary result schema and origin contract
permit. Program-extent character storage is not permission to retain the temporary input slot,
mutate the text, assume a terminal zero byte or choose a platform ABI. Its length counts decoded
UTF-8 bytes, including embedded zero bytes. Layout, string capabilities and target realization
must be qualified before compiler admission.

`Owner "text"` requires a separating gap and selects that one entry. An interpolated string
is not silently formatted into this input: formatting evaluation, allocation and failure order
need their own contract. A plain string expression likewise does not implicitly call a nominal
owner's literal entry merely because the context expects that owner.

## Invocation, Origins and Cleanup

Select and validate the public entry before preparing inputs. From that point the construction
uses the same prepare/validate/commit/body/result/cleanup boundary as an ordinary call, including
the distinction between a direct marked source and an already committed nested computation.
An interrupted preparation releases reservations, destroys actually produced temporaries and
does not enter the body. Earlier nested effects/transfers are not rolled back. Invocation commits
the complete valid bundle once. Unreceived inputs cannot acquire callee cleanup responsibility.

Result origins use the named input or pack subtree under the ordinary
[origin rules](memory-and-regions.md#value-access-origins). Read/edit components can be retained
only with a proven surviving origin. A dependency on caller-owned preparation storage cannot
escape when that storage ends. A take pack owns received components and cleans its initialized
remainder on every applicable exit; component values moved into the result are not destroyed
again. Recoverable body failure uses the ordinary `throw`/propagation and cleanup rules.

No new allocation context is introduced. Allocator selection through `using` remains outside
this contract: the draft has no declared input/authority mapping for it. Use an explicitly
contracted construction function when ordinary fixed inputs such as an allocator are needed.
Neither the literal's spelling nor its owner's name permits hiding resource effects or changing
the [storage contract](memory-and-regions.md#storage-optimization).

## Const Staging and Qualification

An entry may be declared `const literal ...`. This gives its one checked implementation the same
constant-evaluation promise as `const func`, under the
[evaluator domain](constant-evaluation.md#const-calls-and-temporary-evaluation). It is still
callable at runtime. Pack preparation, explicit transfers, body execution and cleanup use the
same selected operations. Intrinsic structural field construction needs no user entry call.
A structural grant has no second `const literal { ... }` spelling: its ordinary constituent
operations determine evaluator eligibility.

Const does not make arbitrary container storage, string resources or user destruction evaluable
or serializable. All branches must check under the promised evaluator domain. Optional runtime
folding may prove further transformations but cannot manufacture a public const capability.
Conditional generic const evidence follows the
[shared eligibility contract](constant-evaluation.md#conditional-generic-const-contracts);
additional evaluator domains still need their own admission.

Qualification must cover empty/ordinary/keyed selection, unknown generic components, mixed
contributions, failed input authority, repeated marked sources, spread evaluation, interrupted
preparation, partial body consumption, body failure and dependency escape. A private-field
construction grant or parser-only acceptance is not evidence for these callable entries.
