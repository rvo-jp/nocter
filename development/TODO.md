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

The [bounded value-binding front end](history/reviews/v2.0.0-value-binding-front-end.md) now
replaces the old private checkpoint. Its thirty source cases run through production syntax,
declarations, typing and shared initialization/access/resource checking: fourteen acceptances,
twelve checking rejections and four syntax rejections. No CLI mode, released-program adapter,
parallel accepted authority or duplicate owner-survival set was added. Ordinary results are
values; input operations and local mutability/aliasing remain separate from data TypeId.

Closed nominal record metadata shares that private declaration/type authority. The same resource
ledger now handles logical field places, sparse partial moves/restoration, complete replacement,
remaining-region cleanup and evaluation frames. Container presence and complete initialization
are distinct queries over that one ledger; intact record schemas remain shared. These Rust-level
checks do not yet admit nominal source or body operations. See the
[schema checkpoint](history/reviews/v2.0.0-nominal-payload-schemas.md) and
[partial-resource checkpoint](history/reviews/v2.0.0-partial-payload-resources.md).

Next introduce selected field places coherently into typed bodies and the shared control graph,
initialization, permission-demand liveness, reservations and source diagnostics, then admit
private record headers/operations under their owning specification. Preserve schema sharing and
one selected operation/control/access/resource authority. Prove partial-move paths, disjoint and
overlapping accesses, replacement cleanup and survival together before accepting these shapes.

The next integration target must be real `V2CoreUnit` source cases, not another
resource-only completion claim: declare a two-error-field record; accept extraction and return of
one field with cleanup of its sibling; accept restoration before whole-value return and partial
`drop`; reject whole-value use after extraction and overlapping call accesses. Admit the source
only with typing, projected initialization, permission and resource evidence together. Use direct
source tests and affected-crate checks during this work; reserve the complete downstream gate for
integration closure under the [verification selection policy](verification/README.md#selecting-evidence-for-a-change).

Then extend generic/callable/interface contracts, capabilities, outcomes/async
and public modules under closed owning contracts, followed by target/ABI and native lifecycle
realization. The four-type fragment's empty D/S evidence is an exact contract, not a generic
inference result. Full-language grammar assembly remains a draft.

Read each owning crate README before editing. The
[implementation design](history/reviews/v2.0.0-value-binding-implementation.md) retains the broader
36 adversarial requirements and capability/result/state-footprint proof obligations. Current
production conformance does not establish native execution, full-language safety or speedup.
Earlier [pre-S12 contracts](history/reviews/v2.0.0-access-checkpoint-contracts.md) remain historical
evidence; their incompatible executable fixtures and source paths have been retired. Keep current
released behavior and release artifacts unchanged.

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

No external or human-decision blocker remains for the next v2.0.0 extension task or the
qualified v0.73.0 release candidate. Full v2.0.0 still requires the remaining grammar, compiler extensions,
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
