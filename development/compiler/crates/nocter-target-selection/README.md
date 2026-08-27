# nocter-target-selection

## Responsibility

Own the single syntax-backed activity decision for items and imports guarded by `#target`.

## Contract

The builder consumes syntax trees, source storage, and one selected compilation target. It publishes
an immutable `TargetSelection` used by discovery and declaration lowering. Unknown authored targets
remain typed source errors while their items stay inactive for safe graph discovery.

## Invariants

- A target directive is decoded once.
- Complete target gates remain usable beside unrelated incomplete syntax.
- Incomplete or unknown gates never activate nested imports.
- Downstream stages cannot rescan syntax to choose a different activity result.
