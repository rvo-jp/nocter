# Diagnostics

## Additional Diagnostic Registration

This section owns newly registered v2.0.0 codes. Codes already assigned by the
[published catalog](../../../spec/tooling/diagnostics.md) keep their numeric registration there;
their adopted v2.0.0 meanings are specified by the sections below. Do not copy released entries
into this registration list or publish these additional entries before specification promotion.

- `E0236`: a bodyless primitive callable is not selected for its exact module/declaration by the
  toolchain, a selected role is authored as an ordinary definition, or its value-model signature
  has not been qualified. Identify that declaration; matching source spelling alone grants no
  primitive authority.

## Core Syntax Diagnostics

The future [value/alias grammar](../language/syntactic-grammar.md#access-syntax)
defines adopted recognition. Ordinary parser failures use `E0120` and identify
the expected production. There is no surviving direct-marker assertion grammar or unary `&&`
subdivision. Retired prefix spelling is not an automatic repair opportunity: replacing a prefix
with a binding/input operation requires knowing its intended behavior.

Retain non-place alias initializers/assignment targets in syntax for semantic diagnostics. A
contextual `read`/`edit`/`take` spelling is recognized from grammar position, never type lookup.
Current private implementation scope belongs to the
[access handoff](../../architecture/pipeline/value-access-declarations.md). These rules do not
change released code assignments or diagnostic behavior.

## Explicit Transfer Diagnostics

An unmarked existing non-Copy value identifies the supplying place and missing postfix caret.
Offer `place^` only when it owns a complete initialized value and no access/destructor obligation
prevents transfer. Distinguish this from a read/edit alias, projection or static source, where a
caret cannot create ownership, and from a fresh value/repeated caret, where no existing source
can be retired. A marked Copy value becomes unavailable too; diagnose later demands at that source
without suggesting that specialization or last-use analysis can remove retirement.

Relate direct call reservations to final commit and any simultaneous conflicting input. A later
preparation failure alone does not consume the reserved source. Distinguish earlier nested
construction, capture formation and outcome acquisition, which can already have committed. A
body caret cannot authorize an implicit outside non-Copy capture; point to its formation-time
capture segment. Pack diagnostics distinguish component retirement from retained count metadata.
These adopted obligations do not qualify the private prototype or allocate new numeric codes.

## Private Header Diagnostics

Reserve declaration names before bodies, then normalize requested headers through their type/value
dependencies. Complete Unit acceptance still checks every header, including unused declarations.
Duplicate function names use `E0241`, duplicate/shadowing
input names use `E0341`, and reserved function names use `E0240`, retaining the existing related
first-declaration locations. Private record-name and field-name collisions use `E0241` with the
first conflicting declaration related. Unknown header data names use `E0290`; a known type outside
the closed fragment, non-finite by-value containment or an invalid `copy struct` request uses
`E0291`. Focus the authored type or field type; relate the referenced record when explaining a
containment cycle. These rules do not permit overload selection by input operation,
body or declaration order. Header/body mismatch reports both declarations and differing modes,
types and origin/state relations. An impossible source/product pairing remains an integrity
failure, never a fabricated source name error.
An authorized primitive's input count, input data/operation or result data mismatch also uses
`E0291`. Identify the actual name/input/result subject and state the expected contract; a failed
signature must not leave a checked binding in a later construction snapshot or retry.

## Core Body Typing Diagnostics

Use source operations in messages, rather than old I/U/X-qualified local types. Distinguish:

| Existing code family | Future failed obligation |
| --- | --- |
| `E0340`, `E0341` | Name resolution or conflicting binding |
| `E0370` | Data/type/result mismatch, incompatible callable witness/annotation, unknown field/body data type, invalid/missing/duplicate record initializer, or insufficient local/result type information |
| `E0371`, `E0377` | Value construction lacks Copy, an already produced value or a marked eligible owning source |
| `E0372`, `E0373` | Non-unit intermediate expression or missing normal result |
| `E0375` | Integer outside the selected data type's range |
| `E0382` | Loop exit outside a loop |
| `E0383` | Explicit drop does not name an owning value binding |
| `E0384`, `E0386` | Non-writable assignment target or invalid numeric compound target |
| `E0387` | Alias creation/direct update input lacks the required initialized place/authority |
| `E0390` | Invalid selected call target or argument count |
| `E0407` | Invalid typed discard |

Exact allocation within an existing family is fixed before implementing its compiler migration;
this table does not duplicate numeric registration. A generic read input cannot deliver T without
Copy merely because one future instantiation is an integer. Explain the missing capability or
owning source, not a missing `^` prefix. An uncontextualized closure's unmarked input is read;
body extraction cannot silently change it to take. If an expected callable supplies a different
operation, relate that selected contract when explaining its mismatch. Do not describe a data-type
annotation as overriding that input operation. A fixed value supplied to an edit input needs an intentional
mutable binding; do not silently rewrite it. A view descriptor Copy still retains its dependencies.

A field of a computed record value delivers a value, not existing mutable storage. Reject local
alias creation or assignment at that attempted operation. The adopted future call protocol can
lend update access through its owned temporary. Actual implementation coverage belongs to the
[private handoff](../../architecture/pipeline/value-access-declarations.md); a private scope limit
is not a general language-invalidity rule.
An immutable direct source still cannot gain edit through an invented snapshot. Grouping a read/edit alias does not convert
its referent into an owning result. Preserve known result types on interrupted evaluation, so
unknown fields and data mismatches still report their structural cause without inventing a
runtime access on that path.

A callable annotation mismatch identifies the incompatible invocation view, input count,
input operation/data (with position), or result data. Distinct concrete witnesses are not an
ordinary assignment/join match merely because their signatures agree. Explain that identity
constraint instead of selecting another target or silently erasing dispatch. A scoped alias cannot
construct an invocation adapter; explain that the adapted value needs its own binding. Invalid
callee data or call arity belongs to `E0390`; a non-writable `mut` environment belongs to `E0387`.
A formal callable contract supplies no Copy evidence: value extraction or taking forwarding from
read access uses `E0371`, regardless of the callbacks currently supplied by callers. Consuming or
dropping an owning formal participates in ordinary availability (`E0378`) and retained-access
checks. Equal signatures cannot repair a formal-witness identity mismatch. Callback header leaves
use the same unknown/unsupported data diagnostics as other private headers, including callable
result annotations.

Callable-result closure uses `E0370` when reachable returned values establish distinct code
witnesses, an unanchored recursive relation establishes none, or a dependent application lacks
an established witness for a required input in the bounded implementation. Explain that specific
failed obligation; never guess a target from a matching signature. A named value in a dead branch or overwritten before return
cannot establish a producer, including when supplied through another factory's input. Distinct
applications of the same factory cannot conflate different actual witnesses. For a deferred
join/replacement mismatch, identify the established code or input witnesses at the failed
operation. Retain known structural mismatches even when input evaluation interrupts the call. The exact closed producer can establish Copy for its result; this does not
change the lack of Copy evidence for a generic formal contract.

## Named-Slot Initialization Diagnostics

`E0378` identifies a reached read/update/extraction/drop/compound operation that lacks its required
initialization. Value use and transfer require a complete selected value; field replacement requires
present containing records, and `drop name` requires a present owning container rather than
restoring moved fields. A missing sibling does not invalidate a disjoint selected field. Relate the
declaration and contributing transfers/drops/backedges. An unmarked Copy snapshot is not a transfer
of its source; a marked Copy source is retired; an explicit drop is destruction even for Copy. A
mutable owning slot can be restored by a complete replacement; a fixed binding cannot be assigned
after transfer. Check RHS-first compound targets after all right-hand effects. Unreachable code
keeps structural typing requirements without acquiring fictitious runtime initialization
obligations.

Initialization does not establish dependency survival or absence of conflicts. Conditional cleanup
is not permission to read missing parts or skip required user destruction.

## Referent and Access Diagnostics

Retain the existing `E0395`–`E0398` access/lifetime families for failing origin, conflicting access,
reserved-place and surviving-root obligations. Explain the attempted source operation, affected
place, originating alias/capability, and later use or cleanup requiring it. In particular:

- An ordinary value return into destroyed local/guard storage identifies that cleanup boundary.
- A `void` call that retains a dependency identifies the edited destination/environment and the
  selected post-state clause/default, rather than claiming it returned a reference.
- An omitted public multi-origin bound identifies eligible source inputs without reading a body.
- A callback retaining a fresh invocation input identifies the escaping state and required extent.
- A fixed capability holder that needs ordinary update distinguishes holder mutability from
  whether the pointed-to element type is Copy.

Keep these causes separate from missing generic Copy evidence and data-type mismatch. The
compiler can end access after the last required use/cleanup; do not recommend broader scopes,
clones, resource-lifetime extension or symbols on every call as automatic repairs. Passing these
checks still requires resource/cleanup and native realization validation.

## Module and Operation Contract Diagnostics

For the adopted [module](../language/modules.md), [type behavior](../language/type-behavior.md)
and [interface](../language/interfaces.md) scope, distinguish these obligations:

- A root body/initializer/private helper identifies the forbidden file role; do not silently
  publish it or move it to another source.
- A missing direct `see` names the source dependency needed at that use/pairing, without
  propagating visibility transitively. A cross-module `see` explains the module boundary and
  the need for a module contract rather than suggesting a longer filesystem path.
- A header mismatch relates both declarations and the differing component. A duplicate body
  relates both definitions; neither failure triggers signature-based replacement selection.
- Invalid visibility focuses the path/subject. A widening re-export identifies both audiences
  and its target; inaccessible signature data identifies the exposed declaration and its bound.
- A structural grant missing/duplicating a field identifies the grant and field schema. A
  forbidden private completion of its closed schema explains that the grant already owns it.
  An opaque struct lacking its definition is a different failure.
- A conformance failure identifies its exact interface member or associated declaration and
  selected helper/binding. Missing, duplicate, incompatible and redundant mappings are distinct.
  Adding an inherent method does not satisfy an explicit default override.
- A name collision lists the distinct operation identities, not merely two equal signatures.
  Offer required `value.(Interface.member)(...)` selection only for a genuine collision, not
  as an alternate style or a way to obtain missing authority.
- A destructor-effect or partial-state failure relates the offending operation and the complete,
  immediate cleanup obligation. A helper's name cannot establish that it is safe to call.

Fix exact diagnostic code registration before compiler admission. These requirements do not
invent numeric codes or claim that the bounded private front end handles these subjects.

## Generic, Constant and Opaque Diagnostics

Identify the selected generic parameter slot when an argument has the wrong kind or cannot be
inferred uniquely. Do not recommend a passing-mode change, implementation search or `_` placeholder
as a repair. For an associated collision list the distinct owning interface applications and
required `T.(Interface.Item)` selector; a redundant selector is a separate source error.

Required constant failures distinguish unknown runtime input, unsupported symbolic arithmetic,
forbidden temporary effects, ineligible final resource/address, target arithmetic, dependency
cycle and deterministic evaluation limit. Show the authored cause and relevant dependency/call
locations, not a fabricated runtime allocation failure or proof of nontermination. Explain that
a constant constructs data without owning a place, while a static is an existing immutable place.
Neither an unsupported final value nor budget exhaustion may silently enable runtime initialization.

Opaque-result failures distinguish missing/duplicate matched bodies, disagreement between reached
witnesses, unanchored recursive witness relations, failed interface/associated requirements and
invalid origins. Public messages use the advertised contract; relate private witness locations
only as appropriate implementation evidence. Do not suggest runtime boxing or a public Copy
capability inferred from the hidden type. Exact code registration precedes compiler admission.

## Outcome and Payload Access Diagnostics

Distinguish malformed nesting, unknown optional payload, incompatible exact forwarding and
unavailable Copy/transfer. Successful error data never produces a diagnostic recommending a
failure conversion merely because its type is error. A throw needs an owned error and a declared
failure layer; absence and failure propagation need their matching result layer.

For a projected payload, identify the retained carrier/tag and requested read/edit operation. A
fallible projected `?` cannot implicitly extract its error; explain that explicit value elimination
needs a marked owner, while a read/edit referent cannot surrender caller storage. A projected catch
binding is an alias, not a fabricated owned error; do not recommend throw unless ownership is
actually supplied. A fallback place that dies at handler cleanup identifies that local and cleanup
boundary. Neither returning a capability nor grouping converts dead storage into a valid alias.

The [failure-loss warning](../language/outcomes.md#failure-decisions-and-resource-completion)
is enabled by default and is not a language-validity error. Relate its producer and loss operation,
including replacement, discarded aggregate fields or unused named owners. Do not treat `let _`
as suppression or emit repeated warnings merely as the same local instance moves. Distinguish
explicit catch handling, recipient transfer, pending retention and actual loss. Trap paths do
not receive invented unwind cleanup or an implicit handler. Register exact codes and qualify
conditional/aggregate/async analysis before implementation admission.

## Async and Inferred-Effect Diagnostics

Distinguish producer construction, drive, cancellation and ready-output disposal. A waiting child
constructor executed during its parent's drive is a parent drive failure even if the child body
is safe. Relate the originating call, callback witness, destructor/backend or external-progress
cycle; do not suggest `blocking`, an effect type parameter or a hidden worker as a source repair.
An unknown fact is not a known wait, and non-waiting is not a CPU/time-bound guarantee.

For temporary-input escape explain that read does not snapshot Copy data and that the producer
outlives the caller's evaluation frame. An immediate await can retain that evaluation frame
through suspension; a separately stored future cannot retain a cleaned temporary. Ordinary
borrowed input roots are not extended by promoting them into hidden future owners.

A failed cancellation post-state identifies the affected outside owner, its entry dependency
state, completion bound, suspension/cleanup path and remaining invalid root. Keep this separate
from an uninitialized edit target. Unstarted cancellation does not establish a completion
`from static` promise. Ready output cleanup may have its own outside-write transition; do not
report an empty footprint because the output is owned or zero-sized. Await versus cancellation
must use the corresponding established state evidence without guessing from a source name.

Exact diagnostic registration and production source/target evidence precede admission. None of
these requirements claims async support in the current private fragment.

## Enum and Pattern Diagnostics

Separate declaration, coverage, input authority and extraction failures. A zero-variant schema,
duplicate variant/payload name, invalid Copy request, infinite by-value payload or second schema
owner identifies the conflicting declaration/component. An opaque root's hidden scalar shape
cannot supply missing Copy/equality to a client or silently change the family policy.

A pattern identifies its exact variant and payload/tuple path. Report foreign/inaccessible
variants, missing/duplicate coverage, non-final/redundant catch-alls and shape/arity mismatch
without searching by layout or reparsing a contextual header. A typed non-Copy read leaf cannot
be returned as a value; explain the selected read operation and the need for intentional value
receipt. An edit needs writable initialized storage even on a nonmatching path. Take invalidates
an eligible non-Copy source before testing, including a false if-is path; relate that acquisition
when reporting later use.

For user-destructible carriers, identify the ancestor requiring complete-self entry rather than
silently borrowing an extracted resource. For a discarded component needed by a surviving view,
show the ignored root, binding and later use/cleanup. Ignoring a read/edit leaf does not destroy
its owner; an ignored owned fallible leaf still follows the failure-loss warning. A tuple
parameter keeps one argument index with projected names; no arity repair may flatten its leaves.
Exact code registration and production parser/flow/resource tests precede admission.

## Structural Aggregate Diagnostics

For [tuples and fixed arrays](../language/tuples-and-arrays.md), distinguish invalid tuple arity,
unknown empty-array element type, component mismatch, expected-length mismatch and a count that
is not valid required compile-time usize data. Relate the selected expected type or failing
constant dependency rather than guessing a container, coercion or literal entry.

A repetition failure identifies the element's missing source-context Copy evidence and count;
a zero count still has producer/cleanup effects. Do not ask for T Copy to snapshot an exact
`[T; 0]`, or infer Copy for symbolic N from one known caller. Positive-count zero-sized elements
remain resources, and independent invalid element declarations remain invalid at count zero.

For an unavailable selected element, relate the containing owner, frozen index/position,
contributing move and later demand. Distinguish a provably missing element from inability to
prove a dynamic selection disjoint/initialized. A computed aggregate has no existing place for
a scoped local alias/assignment. A call can lend update to the produced value in its own
temporary; it does not gain ownership of the original field or container. A user index projection/read/edit input has no owning extraction right; an
owning fixed-array subplace can have one. Explain that category instead of recommending a
transfer prefix, blanket constant-index restriction or implicit removal API for every array.

Required const indexing failure reports its evaluated out-of-range operation. Runtime indexing
retains its checked trap contract; the checker must not fabricate an optional or fallible layer.
Exact diagnostic registration and source/resource/evaluator/target qualification precede admission.

## Variadic Input Diagnostics

For [variadic inputs](../language/variadic-inputs.md), separate malformed/final-pack placement,
ordinary/keyed shape mismatch, unsupported tuple/discard binders, fixed-prefix arity and missing
generic/element data. A keyed callable requires its binding name; a one-colon named ordinary
pack is never reparsed using type lookup. A colon in the fixed prefix is not a named argument.

Explain pack names as argument sequences at a failed bare-name data/alias/drop operation. Show
the selected element/key/value operation, frozen source entry and actual root for unavailable
initialization, missing ownership or overlap. Distinct entry ordinals do not prove distinct
actual places. A keyed index is ordinal, not a map lookup, and duplicate keys are not declaration
errors. Receiver-selected pair versus separate key/value preparation cannot change after failure.

A spread outside the final variadic tail, unsupported carrier or incomplete contributed range
reports that contract failure without a guessed iterator, dynamic array type or alternate call
target. For capture/async escape relate the actual roots and cleanup phase. Distinguish capture
formation moves, invocation commit and pending cancellation; no transfer prefix, container
allocation, snapshot of a declared read or delayed transfer is an automatic repair.

Qualify exact diagnostic codes and actual source/capture/resource/evaluator/target cases before
compiler admission. These obligations do not extend current private diagnostic support.

For a from on a trailing callable type, identify its syntactically selected result owner and
origin scope. An unknown inner name cannot trigger outer-state reinterpretation. Relate the
required complete-type grouping when explaining an intended enclosing relation; a second
ungrouped from does not provide an alternative spelling.

## Traversal Diagnostics

For [synchronous traversal](../language/iteration.md), report the selected read/edit/take family,
missing operator/cursor evidence, associated Item/Cursor mismatch and unavailable source rights.
Do not infer take from body use, rank by Copy, call a same-spelled next method or recommend
unit indexing as a current-projection alias. Current-place data containing ! is not an operator
failure envelope. A retired colon-key/range-only loop head is not an alternate valid style.

For a failed step/expansion, show the current/prepared entry, actual backing or guard root, and
advance/cleanup operation that invalidates it. Separate safe sequential for use from simultaneous
variadic retention. A missing stable-root proof cannot silently trigger Copy, hidden cursor-owner
retention, a delayed destructor or an inferred authority requirement. Generic schema conditions
are checked at their definition/actual binding without private-body lookup or retyping.

Distinguish array whole-value loop acquisition, pack component acquisition, direct argument
reservations and already executed user-source/pull transfers. Count overflow follows the prepared
logical entry. A break does not pull another item; a cancellation does not drain the producer.
Qualify exact diagnostic codes and real parser/source/root/effect/cleanup/evaluator/target cases
before admission; documentation cases are not implemented diagnostic behavior.
