# Tooling and Editor Integration

## Value Access Presentation

Present the selected [value-access contract](../language/value-access.md) without redoing selection.
Hover/signature help exposes input operations, callable invocation, delivered value type, result
origins and edited-state bounds. Distinguish default-read inputs from value supply,
and fixed `let` holders from mutable `var` holders. Optional hints can show transfers, snapshots
and retained roots; those hints are not mandatory source tokens or a second type system.

An explanation of a conflict links the attempted operation, access/capability formation and later
use/cleanup. Explain generic evidence at its source boundary, not by inspecting specialization.
Contract navigation connects a complete implementation header to its `index.nct` declaration;
readers do not need an editor to recover an omitted signature. Semantic tokens/references follow
resolved identities, not contextual words or source coordinates as semantic keys. The existing
private-unit frontend does not provide the adopted editor behavior.

## Contract and Interface Navigation

Present each public contract/body pair as one declaration with separate contract and implementation
locations. Definition navigation selects the root contract; implementation navigation selects its
matched body. Public API listings include authored root `use` aliases and effective audiences,
without publishing `see`-visible helpers or compiler-managed fallback names.

For a selected interface operation, retain links to its owning interface, concrete frozen binding
and selected default/helper. Rename of a helper must not rename a mapped interface operation.
Associated bindings link to their owning associated declarations, not an equal spelled type in
another interface. Completion uses source-visible identities and the contracted operation set;
whole-program access to private bodies is not a source visibility grant.

Show a structural grant as construction availability and a destructor as automatic lifecycle
behavior, without suggesting a callable `drop` API. Signature help exposes full input/result and
origin contracts; it cannot depend on reading an implementation body to reconstruct the public
surface. These contracts follow [module rules](../language/modules.md) and
[interface binding](../language/interfaces.md), without claiming implemented v2.0.0 editor support.

## Generic and Compile-Time Presentation

Present generic arguments and associated projections using their resolved declaration/parameter
identities. Hover may show a canonical evaluated constant or length, but retains source navigation
to the authored expression and definition. Symbolic generic constants stay identified by their
parameter, not one caller's specialization. Type and operation selectors link to the selected
interface application/member; unrelated equal spelled members are not references to it.

Distinguish a constant's storage-independent constructed value from an immutable static place.
Do not dump large emitted table initializers into hover. Opaque-result completion shows only its
advertised interface, associated selections and origins; implementation navigation may expose
the matched private body. Editor consumers use established values/witness relations without
running an independent evaluator, deriving hidden Copy evidence or searching for another body.

## Outcome Payload Access

Present value extraction separately from scoped payload projection. Hover on a projected catch
binding shows its read/edit operation and error data; an ordinary catch binding instead owns
its error. Retain the carrier/tag/root links needed to explain invalidation and failed propagation.
Optional hints may show a Copy snapshot, transfer or projection but are not source assertions.
The ordinary prelude some function follows normal declaration navigation; its spelling grants
no intrinsic semantics to a shadowing function. Failure-loss reports use established producer,
handling and loss evidence, not another editor-side control-flow analysis.

## Deferred Phases and Inferred Effects

Present an async declaration's output separately from its call type future R, and an immediate
future-returning callable separately from a producer. Show inferred construction/drive/cleanup
facts as reports, not synthesized source modifiers. A report distinguishes an uncertain witness
or external-progress cycle from a proven wait and links the selected cause.

Explain completion state and cancellation state separately. An edit `from static` can establish
independence after await while unstarted cancellation leaves entry dependencies. Ready-output
disposal includes its own checked state effects. Optional hints may show these dependencies and
their later use; no new cancellation annotation or editor-derived effect solver is introduced.

## Enum and Pattern Presentation

Present an enum's advertised schema/Copy policy, without revealing private opaque alternatives
or deriving whole-type Copy from a known current tag. Variant construction and pattern references
navigate to the same declaration identity. Complete public schemas navigate to the root; private
completion variants participate only in source-visible lookup. Payload names are declarations
inside their own variant, not fields available on every enum value.

Show branch input operation and each leaf's projected data/authority. A take leaf may own a
value while an equal-data read leaf aliases storage. Parameter names link to their one input
position and semantic projection paths; callable hover does not invent flattened parameters.
Explain the specific live payload/tag access preventing mutation and the cleanup responsible
for an ignored owner. Consume the compiler's selected plan and coverage evidence, without an
editor-side pattern resolver or another cleanup analysis.

## Structural Aggregate Presentation

Present tuple positions and fixed-array element/length identity from canonical checked data.
Hover can show the evaluated count while retaining navigation to its authored expression.
Distinguish an exact empty-array Copy proof from Copy of its element type; zero-byte positive
counts must not be displayed as resource-free by byte size alone.

Show whether selection denotes an owning structural subplace, read/edit projection or computed
component value. Optional transfer hints and initialization explanations use the frozen position/
index relation, source-context supply choice and cleanup evidence. They must not rerun index
expressions, infer a separate owner ledger, flatten an array into fields in a public signature,
or offer an alternate parse after a failed proof. These presentation requirements are not
implemented v2.0.0 editor support.

## Variadic Presentation

Show one final formal pack with its ordinary/keyed kind, component data, declared operation
and origin/state bounds. Signature help maps contributions to its entry/component paths after
the fixed prefix. Do not flatten its public signature into a particular invocation's count or
display a first-class array type for its binder. len and indexing are checked pack operations;
ordinary members with those spellings do not share their authority.

Transfer/capture explanations use the frozen prepared sources, actual roots, Copy evidence and
remaining initialization, including separate key/value supply. Show a capture's formation-time
ownership and inferred environment invocation separately. A result's from-pack bound preserves
all relevant actual origins without presenting new authored lifetime names for each entry.
Consume established mappings, overlap/state and cleanup evidence rather than reconstructing
an argument list, evaluating indices or reselecting an expansion in the editor.

## Traversal Presentation

Show the selected expansion family, exact cursor/interface evidence, Item type and read/edit/
received binding authority. Current-place references navigate to their own zero-input operator;
do not merge them with unit indexing or expose an alias-returning ordinary method. Source
interfaces and direct operator requirements project the same frozen operation relation.

Explain cursor-backed versus outside backing and the particular step/cleanup conflicting with
a retained entry. Present conditional public root/write/cleanup schema requirements alongside
authored bounds; the editor cannot infer new Copy/authority, inspect private bodies or solve a
second traversal. Show source acquisition and take-item cleanup distinctly when explaining
early exit, capture/future escape or cancellation. No v2.0.0 editor implementation follows from
these requirements alone.

For produced edit operands, show the caller-owned temporary loan separately from a direct
place update. The temporary owns its cleanup; the callee receives update access. A computed
value does not become a local-alias/assignment place, and an immutable direct source is not
copied merely to satisfy edit. Explain an escaping result/future against that actual temporary
extent rather than suggesting a longer lifetime annotation. The
[private handoff](../../architecture/pipeline/value-access-declarations.md) owns actual frontend
coverage; editor integration needs separate evidence.

## Explicit Transfer Presentation

Show postfix source retirement separately from the receiving read/edit/take contract. A marked
Copy source is retired; an unmarked Copy source snapshots. Show the selected field/element or
whole owner and its commit boundary, including pending call reservations. Capture formation and
invocation refer to different slots. Inlay hints cannot insert or erase these operations, infer
a missing outside move, or present the private prototype as current source conformance.
