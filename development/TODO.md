# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

Continue the [first access front-end slice](history/reviews/v2.0.0-first-access-slice.md) with
carried payload/storage dependencies and complete cleanup validation for prepared owned values,
ordinary exits and returned results. Consume the existing control/evaluation graph, named-slot
facts and referent/permission report; do not reconstruct control, parsing, types or selection.

Production syntax/declarations, typed bodies, control flow, named-slot initialization,
permission-demand liveness and alternative-world referent validation are implemented for the bounded
private goal. Of the [50-case manifest](compiler/tests/fixtures/v2-access/README.md), six fail
syntax, six fail headers, eight fail structural body typing, and thirty produce typed-only evidence.
Seven of those thirty report named-slot initialization failures; four fail referent/permission
validation. Nineteen positive units pass these intermediate checks. The full 50-unit corpus now
reaches its expected distinguishing validation stages, but passing R does not prove D/S or actual
resource cleanup. Add the remaining dependency/resource proofs before constructing any accepted
checked product. In particular,
reserve direct-place call access until commit, retain earlier explicit operations during operand
preparation, validate RHS-first reinitialization, and prove access returns against surviving inputs.
Source recipes rebind only under the same immutable declaration authority and exact own body.

Do not wait for all optimization research, add a parallel Python compiler, silently mix released
std/borrow semantics, or expose a public source-version compatibility mode. A complete v2.0.0
compiler remains unimplemented. No human source choice blocks this bounded next task. The inventory
checker validates registration/shapes/ranges, not source semantics. The owning review records
verification evidence and the earlier non-reproduced TLS failure; do not infer a runtime repair
from these front-end changes.

Continue the [readiness audit](history/reviews/v2.0.0-implementation-readiness.md#remaining-specification-and-validation-work)
for projections/nominals, full access type trees, outcome/async/callable composition, contract/body
matching, module/construction, effect/CTFE, platform/tooling and standard-library migration. Migrate
each owning specification before its implementation. Audit the derived-ordering selector/text
discrepancy separately without changing qualified or published artifacts.

The [v2.0.0 milestone](history/milestones/v2.0.0.md) and
[source decisions](history/reviews/v2.0.0-source-design-decisions.md) own adopted syntax. Keep complete
implementation signatures, type-side qualifiers, operation-directed local inference, explicit
standalone transfer/update, declaration-driven marker-free calls, canonical receivers and
`func`/`mut`/`once`. Do not reopen adopted source choices as approval blockers. Consult the user only
on new writing/reading/learning alternatives; Value Access Model semantics and analyses are delegated
under `AGENTS.md`. Record autonomous closure separately from implemented support.

Follow the [research-to-experiment plan](history/reviews/v2.0.0-runtime-performance-research.md#turning-research-into-nocter-experiments)
and [storage closure](history/reviews/v2.0.0-storage-optimization.md) for later interprocedural
storage/effect certificates, byte-use, cleanup, materialization deadlines and drive-safety proofs.
Keep R/D/S separate from physical placement; preserve recoverable/required requests, effects,
resource identity and logical cleanup. Reproduce real artifacts before claiming performance;
finite assumed-certificate checks are not compiler validation. This research does not block the
bounded front-end task, authorize implicit clones/GC, or impose a Rust comparison gate.

Await explicit publication authorization. Publication must tag the exact v0.73.0 release-content
commit and upload the retained qualified archive without rebuilding it, then record an independent
post-publication audit. Do not reopen v0.72.0; preserve its checked safety dispositions, qualified
artifact, public tag, and external audit as one immutable boundary.

Preserve the v0.72.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No external or human-decision blocker remains for the bounded next access-validation task or the
qualified v0.73.0 release candidate. Full v2.0.0 still requires the remaining grammar, flow/safety
checking, formal validation and specification/implementation-contract migration described above. The current
language deliberately rejects `notrap async` because `future T` has no drive-time trap contract;
v2.0.0 removes the authored modifier rather than adding one to future types.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
