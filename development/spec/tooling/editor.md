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
