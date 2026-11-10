# Constant Evaluation and Conditional Eligibility

## Scope

This chapter owns the adopted v2.0.0 contracts for const callables, inferred constant-evaluation
eligibility, constants, immutable statics and evaluator limits. [Generics](generics-and-constants.md)
own parameter schemas, substitution and constant arguments; [value access](value-access.md) owns
ordinary preparation, resource transfer and cleanup. These rules require compiler/evaluator
qualification; they do not extend the [implemented private fragment](../../architecture/pipeline/value-access-declarations.md).

## const Calls and Temporary Evaluation

`const func`/`const method` makes the same ordinary checked implementation available to constant
evaluation, subject to its inferred eligibility conditions below. The same prefix applies to
comparison, typed literal, coercion and index/current/expansion entries. It creates no second
operation identity or compile-time-only body. Public/body pairs agree on the authored capability.
Definitions reject statically forbidden source operations locally. Generic data/witness
obligations and guards for actual resource existence may remain conditional without changing the
ordinary runtime domain.

A const interface or structural-callable requirement guarantees the advertised const capability.
Its matching rules and the weaker per-witness facts of ordinary requirements are defined below.
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
unevaluated branches. External runtime input, I/O, clocks/randomness, external mutable state, async work,
concrete runtime-only calls, user destructors and source allocator/resource operations are outside this
domain. Internal evaluator memory is charged to compilation budgets, not a source allocation
effect or a runtime backing request. A pure checked helper name cannot supply a missing evaluator
contract. More general temporary heap resources, callable invocation and outcomes require their
own evaluator extensions before admission. Enum/pattern operations use the same checked
operations and cleanup as runtime source, not a separate constructor or pattern interpreter.

Evaluator-owned storage remains real semantic state. Read/edit inputs retain alias and update
behavior; const is not a purity or result-only memoization certificate:

```text
const func increment(edit value: i32): void { value += 1 }
const func two_updates(): i32 {
    var left = 0
    var right = 0
    increment(left)
    increment(right)
    left + right
}
const TWO: i32 = two_updates()
```

TWO is 2. Reusing a unit result for the second increment must not omit its write. A memoization
proof must preserve the complete relevant state transition, alias/root identity, ownership,
cleanup and checks, or establish that only the result can be observed. Failed evaluation cannot
publish partial state or a partially evaluated constant. This introduces no access to external
mutable storage or source runtime resources.

The same restrictions apply to user operation entries and implicit cleanup. A const place
projection must produce a valid place under the ordinary access/origin rules, referring to
eligible evaluator storage or explicitly supported immutable embedded data. It does not become
a value-returning function. A const expansion's cursor, selected advance/current operations and
cleanup must all have evaluator evidence. A protocol name or an intrinsic operation on another
type cannot supply a missing witness. Literal and conversion syntax grants no evaluator storage
or result-serialization exception.

A generic const body uses the same declared assumptions for ordinary typing and operation
selection. Constant eligibility is an additional checked product over those decisions. Missing
ordinary capability, Copy evidence or input authority is still a definition error; conditional
eligibility cannot repair it. The following contract closes symbolic const obligations without
adding source modifiers or requiring authors to repeat inferred conditions in a where clause.

Type checking and evaluation consume the same resolved targets, input operations and concrete
specializations. The [checked integer contract](integers.md), target widths, shifts, conversion and floating-point
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

## Conditional Generic Const Contracts

### Ordinary Validity and Required Evaluation

A generic const declaration has one runtime body, one authored signature and a compiler-derived
eligibility condition. Its ordinary generic requirements apply to every runtime call. The extra
condition applies when constant evaluation is required, or when binding a surface that explicitly
guarantees const invocation. It is not an additional runtime type constraint, overload candidate,
implicit branch or runtime check.

The condition states which selected operation witnesses and data/resource operations must be
constant-evaluable. It may be true, false or unresolved under the current substitution and known
value state. Unresolved is not proof of eligibility. It can remain symbolic inside a template,
but an actual required evaluation must discharge the relevant obligations before performing the
corresponding operation. A false condition does not invalidate an otherwise valid runtime call.

For example, the interface here promises a runtime operation, while one specific conformance
additionally has a checked const-capable implementation:

```text
interface Measure {
    method self.measure(): i32
}
copy struct Count {
    value: i32
}
impl Count {
    const method self.measure(): i32 { self.value }
    impl Measure
}
copy struct RuntimeCount {
    value: i32
}
impl RuntimeCount {
    method self.measure(): i32 { self.value }
    impl Measure
}
const func inspect<T>(value: T): i32
where T impl Measure {
    value.measure()
}
const FIXED: i32 = inspect(Count { value: 7 })
func run(): i32 { inspect(RuntimeCount { value: 7 }) }
```

Inspect's ordinary body is valid for both conformances. Its inferred const condition requires
const eligibility of the selected Measure.measure binding. FIXED satisfies that condition;
run uses the ordinary runtime body. Conversely, a required initializer
`const BAD: i32 = inspect(RuntimeCount { value: 7 })` fails at the call with the selected
non-const operation identified. RuntimeCount's simple private body does not manufacture an
authored const capability. An unmarked func inspect would remain unavailable to required
constant evaluation even for Count, although optional runtime optimization could fold it.

These are contract examples, not accepted private-source fixtures or evaluator evidence.

### Deriving and Composing Obligations

Check every const definition once with ordinary generic typing, access, initialization and
cleanup. Then derive eligibility over its frozen operations, witness positions and resource
schemas. A selected runtime-only concrete call, forbidden source primitive or unconditionally
required user destructor is a local definition error even in an uninvoked body or unevaluated
source branch. An operation whose eligibility genuinely depends on a generic data/witness
binding contributes a symbolic obligation at that operation. Implicit component cleanup also
retains its actual-resource guards, including in nongeneric bodies. A fixed forbidden source
operation cannot be disguised as a type parameter.

Use only facts that the selected operations expose: authored const capability, a callee's own
checked condition, selected view/adapter behavior, data operations and actual cleanup. A missing
method/operator requirement is not an inferred const condition. Nor may inference acquire Copy,
change read/edit/take, select another interface mapping, introduce a clone or retype a body after
substitution. A pure-looking unmarked function is not an eligible const call.

Compose a callee's condition through the already selected generic arguments, input positions and
witness/view substitution. Include preparation and cleanup introduced by the receiving contract;
for example, a consuming adapter's environment disposal is not erased by a const-capable body.
Forwarding preserves that condition rather than replacing it with the callee name or a favorable
current implementation. Unused generic parameters need no blanket const-data requirement.

Data operations are checked individually, not through a new const-safe pseudo-interface. For
example, `const func discard<T>(take value: T): void {}` retains an obligation for the actual
received value's implicit cleanup. A runtime caller can still transfer an error handle to it;
a constant-evaluation request cannot perform error-graph destruction in this evaluator domain.
Writing a nongeneric const discard with that known unsupported cleanup fails at definition.

Resource obligations retain actual initialization and selected variants. An inactive enum payload
or absent array element creates no cleanup obligation. For example, the declared evaluator domain
permits the following selected payload even though another variant can own an error:

```text
enum Token {
    plain(value: i32)
    failed(reason: error)
}
const TOKEN: Token = Token.plain(3)
const COMPLETE: void = discard(TOKEN)
```

The enum's own user destructor, if present, would still apply. A zero-count repetition still
evaluates and disposes its element expression under the array contract; final emptiness cannot
erase those operations. These resource-existence guards do not exempt forbidden user-written
operations in an ordinary if branch. Do not create another source-control interpreter or infer
availability by executing an unsupported operation speculatively.

Recursive const calls retain graph edges. Analyze local operation obligations and outgoing
conditions for a recursive component without demanding a finite unrolling of recursive calls.
A cycle supplies no missing capability, but recursion alone is not an ineligibility proof.
Evaluation still needs its independent work/depth budgets; declaration dependency cycles retain
their separate error. Growing substitutions or condition solving that exhausts a deterministic
budget report that resource instead of treating uncertainty as success or nontermination.

### Published Facts and Contract Views

A module exports a checked const-eligibility product for each authored const entry and applicable
static operation binding. It belongs to the exact matched declaration/body generation and retains
semantic type, input and witness identities plus causal operation locations. Its conditions and
references to evaluator plans derive from the checked program once. They are not another authored
signature, a copied body, or a set of where clauses maintained by the user.

`index.nct` remains the sole authored outward contract. A caller consumes the published product;
it does not open private source, repeat body checking or discover a stronger capability by looking
up the helper currently used by a conformance. Private helper identities may be carried by that
validated product without becoming source-visible names. Required evaluation still needs the
matched evaluator plans: an eligibility summary alone is not an executable implementation.
Missing or incompatible products produce an unavailable-evaluation diagnostic, not a guessed fact.

An ordinary interface/member or structural requirement does not guarantee const for all actual
implementations. Its exact selected static witness may nevertheless carry a separately published
availability fact, as Count does above. A const template can retain a condition on that fact.
This is additional evidence for that witness, not a silent strengthening of the interface or
ordinary callable type. No runtime search or private-body inference occurs at use.

A generic const declaration publishes conditional availability. A surface requiring guaranteed
const invocation, including a const interface member or const callable contract, is stronger.
Matching must prove the implementation's eligibility conditions from that surface's declared
premises and ordinary eligible evaluator inputs. It cannot silently attach a
stronger inferred precondition to the advertised surface. Matching that guarantee is ordinary
typing: an unproved const-view binding cannot be repaired by exporting its proof as an optional
body eligibility condition. In a generic body, the declared premises must justify the binding.
A runtime-only implementation fails that match. Ordinary invocation adapters retain their selected
invocation kind, input operations and cleanup; they cannot restore a const fact that a specified erasure discarded. General erased
callable execution remains outside the admitted evaluator domain.

Changing a body, type cleanup, selected conformance, adapter, target or evaluator domain invalidates
its dependent products and proofs. A source header can stay unchanged while an inferred condition
changes, just as an inferred execution effect can change. Reports must expose the changed
condition and its cause; consumers cannot reuse a condition with an unrelated body generation.
No special operator name or interface spelling registers evaluator privilege.

### Discharge, Results and Diagnostics

Resolve ordinary targets and substitutions first, then discharge their const obligations. Static
conditions can close before execution. If a resource condition needs an actual tag or initialized
state, the same evaluator checks it when that state is established, before the affected operation.
The evaluator consumes the frozen plan and its obligation references; it does not interpret a
second copy of the source or choose a different operation after failure. A tag/initialization
fact must be revalidated after an operation that can change that state; it is not a timeless
type fact. Interrupted/unproduced values acquire no fabricated payload, witness, invocation edge
or cleanup certificate.

Eligibility, successful execution and final serialization are separate. Supported checked
arithmetic can overflow for particular constant inputs; a const call can diverge or exhaust its
budget. Those failures are not missing const capabilities. A valid evaluator temporary may also
be ineligible as a final constant/static value. No successful condition proves termination,
trap freedom, bounded work, absence of runtime allocation, or final-value eligibility.

A failed required request reports its call/initializer, the instantiated condition, the selected
operation or implicit cleanup causing it, and the intervening forwarding/adapter path. Unknown
witness availability, a known runtime-only target, an unsupported evaluator operation, missing
artifact, arithmetic failure, budget exhaustion and final-value escape need distinct causes.
There is no runtime fallback or suggestion to add retired effect modifiers.

Conditional eligibility introduces no new evaluator value family. Callback invocation, arbitrary
resources, outcomes and additional representations still require their owning domain extensions
and end-to-end qualification before source admission.

## Constants, Statics and Final Values

A `const NAME: Type = expression` is storage-independent data, not an addressable owner. A
`static NAME: Type = expression` is one immutable addressable place with static extent. Their
public roots declare only name/type; one matched ordinary source supplies the initializer.
No runtime initializer, dynamic initialization order or mutable global is introduced.
An initializer is a value-expression boundary, not a source callable: `return` cannot target it
or escape it into an enclosing callable. A nested source callable retains its own ordinary
return target. Complex early-returning initialization belongs in a named const callable, whose
result can supply the initializer.

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

## Rationale

Compiler-derived conditions avoid repeating operation and cleanup requirements in source, while
exported products keep separate compilation independent of private-source lookup. Definition-time
domain checks make the promise independent of ordinary scalar branch values and optimization
settings; optional runtime folding remains free to prove more. Resource-existence guards instead
avoid inventing operations on absent components. D's
[function attribute inference](https://dlang.org/spec/function.html#function-attribute-inference)
illustrates deriving template facts from checked bodies to reduce annotations. The C++ draft's
[constexpr contract](https://eel.is/c++draft/dcl.constexpr) separately distinguishes a callable's
staging designation from whether a particular invocation can be a constant expression. Nocter's
conditions, definition-time checks and evaluation domain are defined above; these precedents are
evidence for the separation, not inherited source or evaluator rules.
