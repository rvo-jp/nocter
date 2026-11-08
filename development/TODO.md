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

The bounded front end includes closed records, named/computed fields, local named function values
and now [statically witnessed callable inputs](history/reviews/v2.0.0-callable-input-witnesses.md).
The source corpus has 189 cases: 96 accepted units, 81 checking rejections, 6 header rejections
and 6 syntax rejections. Formal contracts are checked once independently of caller/body use;
selected argument plans retain formal-to-actual witness bindings. Forwarding/recurse uses finite
semantic identities. Owning formals preserve symbolic cleanup without acquiring Copy or empty D/S.
The producer domain remains named code/formal forwarding, with no captured environment producer.
No CLI mode, native output or performance claim follows.

Next close callable results and executable witness specialization together: represent a result
witness as a checked relation over named/formal producers, preserve return/break/conditional joins,
reject heterogeneous/ambiguous or unsupported recursive witness equations, and instantiate already
selected targets by substitution rather than retyping bodies or searching contracts. Preserve each
formal's authored operation and source-level capabilities even for a currently Copy named caller.
Add source-to-unit proof for returned forwarding, recursive closure/termination, distinct witness
rejection, interruption and cleanup. The current argument binding recipes are not a concrete
specialization graph or a native certificate. Then admit captured environments only after R/D/S,
retained-state/write footprints and environment destruction are closed under that same authority.

Use direct source tests and affected-crate checks; select broader downstream tests by their actual
consumption of the changed path under the [verification selection policy](verification/README.md#selecting-evidence-for-a-change).
The current private fragment does not reach LSP or native products, so their runtime tests are not
feature evidence. Workspace static checks still verify API compatibility.

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
