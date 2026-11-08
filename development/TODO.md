# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

The user is reviewing a [value-binding source candidate](history/reviews/v2.0.0-value-binding-candidate.md)
after the first access checkpoint. Its 44 reviewed cases cover values, view returns, dependent
records and callbacks. `let`/`var`, declaration-side edit/take and implicit value-boundary transfer
remain unadopted candidate syntax; do not silently migrate the specification/compiler to it.
The review records the required internal dependency/callback obligations and recommends concrete
advanced origin/capture spellings for human evaluation. Existing implemented behavior below
remains the qualified checkpoint while this source choice is under review.

The [first access front-end slice](history/reviews/v2.0.0-first-access-slice.md) now has closed
private-unit acceptance. Production syntax/declarations, body typing, control flow, named-slot
initialization, permission-demand liveness, referent validation, exact first-fragment payload
contracts and logical ownership/cleanup transitions are implemented. The 50-case corpus rejects
six syntax, six header and nineteen checked failures, and accepts nineteen whole private units.
Every definition is checked, including unreachable and recursive callees. Rebinding requires the
same immutable declaration authority and every original body.

Extend the same model through projections/nominals and public contract matching, then
callable/generic/outcome/async composition under the slice's work order. Close the target consumer
contract before executable integration. A native consumer
must realize the existing ordered ownership/disposal/invocation transitions and checked primitive
operations; do not reconstruct source, dispatch, initialization or cleanup decisions. The current
accepted unit has no target, ABI, runtime/error-storage realization, CLI mode, released-program
adapter or native execution. Do not call it a complete v2.0.0 compiler.

The bounded payload certificate is exact: scalar/unit and the opaque owned-error snapshot have
empty D/S under their owning contracts. Error remains move-only and owns graph cleanup. This is
not a generic dependency analysis and must not be extended to aggregates, closures, futures or
regions by Copy/ownership heuristics. Fresh X inputs can supply I without Copy; their temporary
roots end at the enclosing evaluation. Prepared owned prefixes are disposed in reverse order on
interruption; named reservations survive. Staged results and replacements retain their obligations.
General destructor effects, recoverable outcomes and async cancellation need separate migration.

No human source choice blocks the next bounded task. Keep whole-unit acceptance sealed; never
publish partial bodies after a sibling rejection or add a parallel Python compiler. The inventory
checker still checks registration/shapes/ranges only. The owning review records verification and
the earlier non-reproduced TLS failure; these front-end changes do not claim a runtime repair.

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

No external or human-decision blocker remains for the bounded next implementation task or the
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
