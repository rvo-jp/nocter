# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

The user adopted the value-binding foundation as
[S12–S15](history/reviews/v2.0.0-source-design-decisions.md#s12-separate-values-binding-operations-and-capabilities).
The [implementation design](history/reviews/v2.0.0-value-binding-implementation.md) owns replacement
work, a whole-source grammar assembly draft, 36 additional adversarial requirements and
proof/qualification gates. Owning future chapters
now state value/alias/input/callable/result/state-origin rules and tooling requirements. This is
specification/design work; no S12 compiler or executable acceptance is implemented.

Next close the named declaration/module/generic grammar dependencies relevant to a first S12
private unit, derive fixtures from the 44 reviewed and 36 additional cases, and replace the current
parser/declarations/typing/init/access/resource boundary coherently. Read each crate README before
changing it. Preserve one selected-operation/control graph, one resource-survival authority and
sealed whole-unit acceptance; do not retain old qualified results, source prefixes, implicit aliases
or a compatibility adapter under the new parser. Capability schemas and result/edited-state
relations must survive abstract/generic/erased use. No human source choice blocks this design task.

The [first-access slice](history/reviews/v2.0.0-first-access-slice.md) and
[checkpoint snapshot](history/reviews/v2.0.0-access-checkpoint-contracts.md) preserve the existing
implementation's scope: 50 source cases, nineteen accepted whole private units; all definitions
checked; exact source rebinding; init/access/cleanup evidence. This is pre-S12 qualification,
not a compatible subset or new-language conformance. The inventory checker is registration-only.
No target, ABI, runtime/error-storage realization, CLI mode, native execution or complete generic
D/S analysis is claimed. Keep current released behavior and release artifacts unchanged.

Full v2.0.0 still needs remaining item/module/visibility/construction, generic/interface/CTFE/effect,
outcome/async, library and platform realization, plus matching formatting/diagnostics/editor
implementations. Migrate each owning subject before accepting it in the compiler. Audit the
previously noted derived-ordering selector/text discrepancy separately from release artifacts.

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

No external or human-decision blocker remains for the next S12 design/migration task or the
qualified v0.73.0 release candidate. Full v2.0.0 still requires the grammar, compiler replacement,
formal validation and full-language migrations described above. The current
language deliberately rejects `notrap async` because `future T` has no drive-time trap contract;
v2.0.0 removes the authored modifier rather than adding one to future types.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
