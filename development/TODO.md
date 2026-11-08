# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

S12–S16 define the adopted value-binding direction. The user delegated the input-default decision
and subsequent v2.0.0 source/design/planning/work to the agent. The selected default remains
unmarked read, with edit/take explicit; no explicit-read input alias or type/name/body-based
default. Uncontextualized closure inputs default to read; a selected expected contract can supply
omitted operations. See the [decision record](history/reviews/v2.0.0-parameter-defaults.md).
Do not stop for approval of ordinary design alternatives or work order; publication remains separate.

The [bounded value-binding grammar](../spec/language/syntactic-grammar.md#bounded-value-binding-fragment)
now closes the next private-module source/checking scope. The
[planned source corpus](compiler/tests/fixtures/value-bindings/README.md) contains thirty intended
verdicts and is explicitly unconsumed. Next replace the production `V2CoreUnit` parser,
declarations/value-result metadata, binding/input typing, shared flow/init/access/resource plans
as one sealed accepted-unit boundary; wire the planned corpus into production tests and retire
incompatible old expectations then. No new CLI version switch, parallel parser/checker, old-mode
adapter, duplicate owner-survival set or partial accepted-unit publication is permitted.

Read each owning crate README before editing. The
[implementation design](history/reviews/v2.0.0-value-binding-implementation.md) retains the broader
36 adversarial requirements, capability/result/state-footprint and proof/qualification gates.
No S12 implementation/native conformance follows from source inventories or documentation builds.
After this bounded replacement, extend projections/nominals, generic/callable/interface schemas,
outcomes/async and public modules under their own closed owning contracts; then realize target/ABI
and native lifecycle behavior. Keep whole-source grammar assembly a draft until its semantic
subjects migrate coherently; it does not block the closed four-type private fragment.

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
