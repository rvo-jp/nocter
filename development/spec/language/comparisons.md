# Equality and Comparison Operations

This chapter owns v2.0.0 comparison identities, selection, derived expressions and required
evidence. It is an adopted contract, not a compiler-support claim. The
[private handoff](../../architecture/pipeline/value-access-declarations.md) owns implemented scope;
[place projections](place-projections.md) have a different, existing-storage result protocol.

## Core Primitive Comparisons

Initial primitive `==`/`!=` compare matching `bool`, intrinsic integer, or `void` payloads and construct a
Boolean value. Reading an operand does not transfer its owner. Unit equality is true and inequality
false after evaluating both operands in source order. Do not remove their effects/access validation.
Opaque `error` has no primitive equality. Nominal entries below do not add structural equality
or expand this primitive domain. Other primitive families need their own adopted contracts;
the published domain is not inherited.

Matching integer operands additionally have intrinsic strict comparison by their signed
or unsigned integer values. Their contextual integer literals follow the
[integer contract](integers.md#pointer-size-integers-and-literal-context). Bool and unit have no
intrinsic ordering. The derived rules below apply to all intrinsic integer types;
their known total integer relations may justify an equivalent lowering only while preserving
operand evaluation, access and effects. A primitive witness does not change a generic body's
already checked input/preparation contract.

## Scalar Enum Equality

A scalar enum whose original complete declaration selects the payloadless policy supports
intrinsic ==/!=: matching nominal types compare their variant identities, without consuming either
operand. Aliases retain that identity. There is no implicit numeric conversion or declaration-order comparison, and no
comparison between distinct nominal enums with equal tags/names. A `copy enum` payload family
does not thereby gain structural equality; an opaque enum's Copy grant exposes no hidden scalar
schema or equality operation. Those types need an explicit comparison entry when equality is
required. Scalar enum equality does not grant ordering.
Seeing a private completion of an opaque family does not select another intrinsic equality policy.
This selected extension is not implemented by the bounded primitive-equality fragment above.

## Type-Owned Entries

A nominal owner may declare one equality entry and one strict-comparison entry. Both read their
receiver and one named operand of the same normalized Self data type, and return ordinary bool:

```text
copy struct Rank {
    value: i32
}
impl Rank {
    const operator (self == other: Self): bool {
        self.value == other.value
    }
    const operator (self < other: Self): bool {
        self.value < other.value
    }
}
```

The fixed head spells operand data as Self and result as bool. The input is unmarked read,
not a value-taking input or a type qualifier. Copy does not change either entry's passing operation. Edit/take receivers or operands, another operand data
type, generic parameters on the entry, variadic inputs, an async head and a non-bool result are
invalid. These entries do not produce places or expose a result-origin clause. `!=`, `<=`, `>`
and `>=` have no separately declared entries. Arithmetic and arbitrary token overloading are not
introduced by this contract.

The ordinary type-ownership and applicability rules apply. Aliases introduce no entry or
new owner. Entry identity is the owner plus fixed comparison family, not the operand name,
requirements, effects, const capability or body contents. Conditional fragments cannot overlap
or provide ranked alternatives. A root publishes the bodyless contract; its reciprocally seen
implementation repeats the complete signature and supplies the body. A required interface entry
is bodyless; an ordinary private entry requires its body. Comparison defaults and named helper
mappings are not introduced. The [grammar](syntactic-grammar.md#comparison-operations) owns syntax.

Intrinsic comparison policy is selected by the data contract before nominal lookup. It cannot be
replaced by an entry on an alias, a private completion or a similarly named method. A builtin's
additional behavior still needs its separately selected ownership authority. This chapter does
not expand the primitive numeric, text or container domain by importing released behavior.

## Selection and Requirements

After ordinary type inference and alias normalization, a comparison requires one matching operand
data type. A type-owned entry reads that same type on both sides. There is no implicit user
conversion, receiver conversion, heterogeneous overload, interface-name lookup or fallback after
an access failure. Use an explicit `as` expression when a conversion is intended. Equality and
strict comparison select independently; knowing one does not supply the other.

Generic code may require the fixed operations without inventing a nominal pseudo-interface:

```text
func no_later<T>(left: T, right: T): bool
where (T < T): bool, (T == T): bool {
    left <= right
}
```

Each structural requirement names exactly one read/read operation on one normalized data type
with a bool result. A different operand pair or result does not match this contract. Requirements
validate an already inferred substitution; they do not search comparable types to infer it.
Specialization supplies a concrete primitive or type-owned witness, preserving the checked
operation and input contract. A private body, later Copy evidence or a different available
operation cannot repair missing evidence.

An interface may require these same fixed entries directly or through structural prerequisites.
They are not independently named methods: conformance must bind the owner's unique matching
entry. Multiple prerequisite paths may prove that same fixed operation without creating an
overload or allowing proof traversal order to select a body. Keep their nominal interface/member
evidence identities distinct; agreement on a type-owned target does not merge conformances.
An incompatible signature or competing owner entry rejects at declaration/conformance, rather
than asking an expression to qualify its operator with an interface name. Named operation
collisions retain the separate [interface rules](interfaces.md#calls-and-name-collisions).

## Derived Expressions and Evaluation

The expression grammar admits one ungrouped equality operator and one ungrouped ordering operator
at their respective levels. Parentheses are required for further composition; there is no
comparison-chain shorthand. Resolve every required operation before execution, even when a
runtime short circuit can skip it.

```text
left == right  = equal(left, right)
left != right  = !equal(left, right)
left < right   = less(left, right)
left > right   = less(right, left)
left <= right  = less(left, right) || equal(left, right)
left >= right  = less(right, left) || equal(left, right)
```

Here equal/less denote the frozen operation identities, not user-visible function names. Evaluate
the written left operand before the right operand, once each. Reversing a strict call changes its
receiver, never source evaluation order. Inclusive comparisons execute strict comparison first
and invoke equality only if it returned false. Both equality steps keep the original left operand
as receiver. No expression, projection, conversion or value construction is repeated for equality.

Operand preparation follows [ordinary access and invocation](value-access.md#preparation-invocation-and-evaluation-temporaries).
Primitive scalar operations preserve prepared scalar snapshots. A type-owned comparison instead
receives read inputs: an existing place lends access, and Copy does not silently snapshot it.
Produced or explicitly transferred operands retain caller-owned backing for the comparison.
An existing non-Copy place needs no transfer merely to be read; an authored postfix transfer
still retires its source under the usual commit rule. A direct source reserved for this comparison
is not consumed if later preparation exits before commit. Previously committed nested effects
are not rolled back.

Retain the prepared pair, carried dependencies and required read authority across both selected
calls. Validate simultaneous input access before the first call. An interrupted operand cannot
invent a call/result. Each entered operator body runs its own normal cleanup before the next step;
caller-backed temporaries remain in their existing enclosing evaluation frame. Stage the bool
result before that frame's once-only cleanup in reverse actual construction order. Skipping
equality neither skips operand evaluation nor duplicates or postpones required destruction. Trap/divergence has no normal-completion cleanup
guarantee. There is no comparison-specific hidden clone, allocation or lifetime extension.

## Effects, Const and Semantic Laws

A comparison's signature does not assert purity, determinism, absence of allocation/waiting or
freedom from traps. [Execution inference](execution-effects.md) includes both selected entries,
operand preparation and required cleanup. An inclusive comparison cannot advertise an effect
bound that forgets its equality path merely because one sample strict comparison returns true.
Removing work or changing call order requires proof preserving all observable behavior.

`const operator ...` makes the same selected body available under the ordinary
[const contract](constant-evaluation.md#const-calls-and-temporary-evaluation). It is not a second
entry or overload. Const interface requirements and per-witness availability follow the
[shared conditional contract](constant-evaluation.md#conditional-generic-const-contracts).
A runtime-only requirement alone is not a const guarantee. Both selected entries of an inclusive
expression must have the evidence required by its checking context; comparison normalization
cannot manufacture that evidence.

An equality operation need not prove reflexivity, symmetry or transitivity. A strict comparison's
existence does not prove irreflexivity, transitivity or comparability. Therefore `<=` is not
defined as `!(right < left)`: with an incomparable pair, both strict directions and equality can
be false. The derived rules preserve that outcome. Similarly, `x == x` is not an unconditional true
constant, and `x < x` is not an unconditional false constant for user entries.

A separately authored interface may document stronger laws for algorithms that need them:

```text
interface TotallyOrdered
where (Self == Self): bool, (Self < Self): bool {}
```

Its author must state the laws; the empty body alone proves none. Conformance exposes the required
operations and records the law commitment, without a runtime dictionary or an automatic proof.
The compiler cannot create conformance merely because both entries exist, infer it from Copy or
layout, or use an unproved law as memory-safety evidence. Analysis may prove useful properties from the
checked implementation and actual operands without requiring an extra user assertion.
Named three-way comparison APIs and
standard-library ordering families need their own library contracts; this example does not
register a privileged interface name.

## Qualification Obligations

Before full source admission, qualify owner/root agreement, duplicate and foreign entries,
exact-type and alias selection, absent strict/equality evidence, repeated proof paths and const
visibility. Exercise read-only non-Copy operands, explicit retirement, later-operand effects,
interrupted preparation, partial temporary cleanup and nested dependent inputs through the actual
checking/instance/MIR/native consumers. Primitive acceptance does not qualify nominal entries.

Derived-expression cases must distinguish original/reversed receivers and source order, a true
strict result that skips an effectful equality, false strict results with either equality result,
incomparable pairs and non-reflexive user equality. Inspect the frozen operation edges and native
effects; a method spelling, a parser shape or an assumed total-order model is insufficient.
