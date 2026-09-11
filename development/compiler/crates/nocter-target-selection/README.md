# nocter-target-selection

## Responsibility

Own the single syntax-backed activity decision for items and imports guarded by `#target`.

## Contract

The builder consumes syntax trees, source storage, and one selected compilation target. It publishes
an immutable `TargetSelection` used by discovery and declaration lowering. Unknown authored targets
remain typed source errors while their items stay inactive for safe graph discovery. The completed
selection retains its target and exact syntax-root multiset so a compile input can reject a stale,
reparsed, or foreign-source selection before semantic lowering.

## Invariants

- A target directive is decoded once.
- Complete target gates remain usable beside unrelated incomplete syntax.
- Incomplete or unknown gates never activate nested imports.
- Downstream stages cannot rescan syntax to choose a different activity result.
- Activity keys retain complete `NodeId` values; they never discard the owning tree identity into a
  source-and-index tuple.
- Activity queries verify that the supplied node belongs to one selected syntax tree. A node from a
  reparsed or foreign tree is inactive rather than relying on the caller to establish membership.
- One selection admits at most one syntax tree for a physical source identity.
