# Explicit User Conversions

## Declaration and Identity

An immediate type-owned conversion is declared with `coerce`, one receiver and its target data
type. It is not a construction function on the destination or an implicit compatibility rule.

```text
impl Buffer {
    coerce self as Summary { self.summarize() }
    coerce take self as Product! { self^.finish()? }
}

let summary = buffer as Summary
let product = buffer^ as Product!
```

These assume matching ordinary methods and independently declared data. The result of the
second expression is fallible; propagation remains explicit, for example
`let product = (buffer^ as Product!)?`. `coerce` has no separate colon result annotation,
parameter list or member name. The type after `as` is its exact ordinary result type, including
optional/fallible layers and their constructor order. No async or opaque-result conversion is
introduced. Read/edit/take receiver operations follow [type behavior](type-behavior.md).

The declaring nominal owns the entry. After alias normalization, one owner has at most one
conversion to each exact target. Receiver operation, origins, effects, requirements and body
contents cannot distinguish overloads. Source and target identity are established before access
checking; a missing permission does not select another conversion. Conditional fragments must
be non-overlapping under the generic contract. No global registry, destination-side lookup,
import-order ranking or chained conversion search is introduced.

The target may be unrelated data. This is the explicit alternative to type-owned `func`, whose
payload constructs its owner. It grants no authority to add behavior to a foreign source type.
Complete public/body headers pair under [modules](modules.md#complete-implementation-signatures).

## Expression and Value Supply

`expression as Type` selects the source owner's exact conversion result after ordinary expected
data/generic constraints are established. It evaluates the source once and invokes that one
entry with its declared receiver operation. Repeated explicit `as` expressions associate left;
each operation and its effects complete before the next. There is no automatic result unwrapping,
outcome injection, conversion through an intermediate target or retry after body failure.

Reading a place for a read receiver preserves its owner. An edit receiver needs direct update
authority, or lends from an already produced value through the ordinary caller temporary.
A take receiver uses ordinary value supply: an unmarked Copy source snapshots; an existing
non-Copy owner needs postfix retirement. `buffer^ as Product!` retires buffer even when its
eventual conversion fails after receipt. A produced Buffer needs no marker. A scoped alias
cannot surrender its referent merely because the conversion receives take.

An identity conversion to exactly the source data requires no user entry. It still evaluates
the operand and constructs an ordinary value under value supply; it is not alias formation,
an ownership grant or a way to bypass the non-Copy marker. A nominal owner cannot replace this
identity operation. Distinct equal-layout types have no identity conversion.

An annotation, parameter, assignment, return or branch join never invokes a user conversion
implicitly. Write the `as` expression at that boundary. Built-in numeric conversions, pointer
operations and callable adapters need their own selected contracts; this chapter does not
inherit a released conversion table or reinterpret them as nominal lookup.

## Origins, Effects and Const

The entry returns ordinary data. An optional/fallible target uses the ordinary result construction,
`throw`, propagation and cleanup rules. A target containing `!` is not a promise of numeric range
checking; the selected implementation owns validation. A conversion cannot suppress a failure
merely because another target or receiver operation would have succeeded.

A `from` clause after the target bounds result dependencies on self/static under the ordinary
[origin rules](memory-and-regions.md#value-access-origins). It cannot export direct storage of
a consumed receiver or evaluation temporary that ends. A receiver's edit-state clause uses the
existing parenthesized receiver form. Copying a descriptor retains carried dependencies.
Preparation, body execution, answer protection and destruction have the same order as an
ordinary one-receiver call; there is no conversion-specific lifetime extension or cleanup path.

`const coerce ...` exposes the same checked body to the existing evaluator domain. It does not
make user allocation/destruction, runtime calls or unsupported results evaluable. Generic
const obligations and final serialization must be proved under their owning contracts.
Optional runtime optimization remains independent of that source capability.

## Admission Obligations

This chapter adopts semantics and recognition, not current compiler support. Before admission,
qualify exact-target/alias selection, duplicate entries, all receiver operations, marked Copy and
non-Copy sources, unavailable/aliased sources, produced temporaries, chained explicit conversions,
outcome layer order, body failure, result origins, edit-state effects, cleanup and const rejection.
Ordinary receiverless calls or matching machine layouts cannot substitute for this evidence.
