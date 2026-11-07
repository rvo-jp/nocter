# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

Refine the [v2.0.0 language design plan](history/milestones/v2.0.0.md) around faster generated-program
execution than Rust through compile-time computation and optimization, with only the syntax needed
to express behavior and information the compiler cannot derive. Establish comparable performance
baselines and close the detailed contracts for adopted capture inference, explicit callable type
arguments, and contract-only `index.nct` with authored `use` re-exports and retained direct `see`.
The declaration-attached `#visibility` directive with `/`, `./`, and repeated `../` boundary paths
is adopted; close its remaining grammar and tooling details. A special `internal.nct` role has not
been adopted. Reducing advanced annotations in ordinary applications remains an adopted goal.
Unifying constant eligibility across value/type positions, extending tuple destructuring to function
and closure parameters and `for` bindings, and grouping item directives with order-independent
attachment are adopted directions. Close their dependency, ownership, grammar, and tooling contracts.
Generic constraint spelling unification remains unadopted.
Interface requirements for receiverless construction functions are adopted, preserving separate
`func` and `method` calls without UFCS. Grouping type-owned behavior under one container remains a
candidate; ordinary receiverless utilities and the broader interface redesign are not adopted.
Other syntax changes and ownership-transparent memory models remain unadopted proposals or
investigations.

Await explicit publication authorization. Publication must tag the exact v0.73.0 release-content
commit and upload the retained qualified archive without rebuilding it, then record an independent
post-publication audit. Do not reopen v0.72.0; preserve its checked safety dispositions, qualified
artifact, public tag, and external audit as one immutable boundary.

Preserve the v0.72.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains. `notrap async` is deliberately rejected because `future T` does not retain a
drive-time trap contract.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
