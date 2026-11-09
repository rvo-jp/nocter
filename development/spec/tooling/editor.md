# Tooling and Editor Integration

## Value Access Presentation

Present the selected [value-access contract](../language/value-access.md) without redoing selection.
Hover/signature help exposes input operations, callable invocation, delivered value type, result
origins and edited-state bounds. Distinguish default-read inputs from Copy-or-move value supply,
and fixed `let` holders from mutable `var` holders. Optional hints can show transfers, snapshots
and retained roots; those hints are not mandatory source tokens or a second type system.

An explanation of a conflict links the attempted operation, access/capability formation and later
use/cleanup. Explain generic evidence at its source boundary, not by inspecting specialization.
Contract navigation connects a complete implementation header to its `index.nct` declaration;
readers do not need an editor to recover an omitted signature. Semantic tokens/references follow
resolved identities, not contextual words or source coordinates as semantic keys. The existing
private-unit checkpoint does not yet provide S12 editor behavior.

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
