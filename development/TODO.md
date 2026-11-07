# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

Establish comparable performance baselines and close the detailed contracts in the
[v2.0.0 language design plan](history/milestones/v2.0.0.md). Prioritize compile-time optimization
and minimal application syntax while preserving the distinction between adopted design and current
compiler support. Next close explicit outcome branch selection and nested optional states,
storable one-value `void`, direct comparison without view fallback, and unhandled-failure/resource
completion rules alongside unified `impl Type` and interface operation contracts. Resolve their
interaction with contract-only roots, visibility, ownership, and cleanup before implementation.
Preserve the single `error` failure payload and cause-preserving propagation; per-API typed failure
payloads are excluded from the v2.0.0 plan, including when error information is successful data.
Audit the derived-ordering selector/text discrepancy separately; the planning finding does not
authorize altering qualified or published artifacts. Retain separate `func`/`method` calls without
UFCS and both outer/inner `impl` roles.

Use the milestone for the complete adopted scope, pending grammar questions, and conformance gates,
including constant eligibility, tuple bindings, directive grouping, capture inference, and explicit
callable arguments. Do not treat remaining ownership-model or syntax proposals as adopted work.

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
