# Static Opaque Results

## Declared Identity and Implementation Witness

`some Interface` in a callable result exposes a static interface contract while keeping the
concrete payload type private. Optional/fallible wrappers, associated bindings and result origins
remain explicit parts of that contract. An interface contract does not create a runtime object,
vtable, dictionary, allocation or automatic erasure.

```text
// index.nct
see ./entries.nct
func entries(): some Source { .Item = i32 }
```

The matched implementation repeats the complete result contract and supplies its body. The pair
has one callable identity and one opaque result identity. The root need not contain an executable
body or public concrete witness. Neither a caller nor a later compiler stage infers a replacement
opaque identity from an equal rendered signature.

One finite concrete witness must be established for each admitted callable specialization, after
normalizing ordinary aliases, generic arguments and associated projections. Every reachable
success-producing return/body result must agree with that witness. Branches cannot select
heterogeneous runtime payloads through a shared interface name. Failure-only, absence-only and
diverging implementations do not establish a witness. Recursive witness equations need an
anchored finite producer; recursion alone cannot invent a layout. Type checking of unreachable
expressions still applies independently of witness production.

The witness must explicitly satisfy the advertised interface and every associated binding.
Result origins and cleanup dependencies are verified under their own contracts, not guessed from
the interface name. A witness cannot refer to destroyed local/consumed storage. Changing its
private representation preserves the public opaque identity only when the whole advertised
contract and observable behavior remain satisfied. This grants no public ABI-stability promise.

## Available Capabilities and Positions

Callers see only the advertised interface, associated selections and origin/state requirements.
Hidden fields, inherent methods, constructor names and private Copy evidence are unavailable.
An opaque value is move-only at the boundary unless a separately adopted public capability says
otherwise; the compiler cannot change source availability after inspecting its witness. Two
declaring callables remain different opaque types even if their interface and concrete witness
happen to match. Generic substitution preserves the declaring identity and ordered arguments.

The source form is allowed as an ordinary named function/method result, including a construction
function whose witness satisfies its owner-construction rule, and a body-bearing interface
default result. A default declared bodylessly in a public root still has one matched body and
witness. For async functions it describes the selected deferred output, subject to the complete
async contract. It is not a general data type for fields, aliases, parameters or structural
callable annotations.

Bodyless required interface operations cannot introduce an implementation-specific opaque result;
they use an associated type when implementations need to choose a result type. Primitives,
typed literals, coercions and destructors cannot introduce one either. A mapping cannot silently
replace an associated-type requirement with a new opaque identity. General existential or
dynamic interface values are not introduced by this source form.

### Default Overrides

For an interface default operation, instantiate its opaque identity by the operation, selected
conformance and generic arguments. Selecting the ordinary default body establishes its witness
for that instantiation. An explicit override mapping instead establishes the witness from the
selected helper's delivered data, which must satisfy the same advertised interface, associated
bindings and origin contract. There is still one selected witness, not a union of default and
override payloads.

This is part of checked conformance binding, not ordinary type equality or a runtime adapter
search. A helper may return concrete data or its own separately established opaque result; its
identity does not become equal to the interface operation's opaque identity. Direct inherent
calls retain the helper's declared result, while interface calls use the instantiated opaque
contract. Hidden Copy evidence is not exposed through that opaque interface result. A same-name inherent
method cannot trigger this witness change without the explicit default override mapping.

## Closure and Tooling Boundary

Resolve contract/body pairing once under [modules](modules.md#complete-implementation-signatures).
Establish the witness in the checked implementation, retaining the opaque contract separately
from concrete layout/cleanup evidence. Freeze both for specialization and executable realization.
A missing body, duplicate body, disagreeing witness, unresolved cycle, interface mismatch and
invalid origin are separate failures; none permits signature-based body search.

Public hover/navigation shows the declared interface/associated bindings and contract location;
implementation navigation can lead to the private body. Completion does not leak hidden members
or synthesize Copy from a witness. These are adopted design obligations. Full generic witness
solving, target-specific body selection, executable representation and native admission remain
implementation/qualification work; a bodyless signature alone is not evidence of their support.
