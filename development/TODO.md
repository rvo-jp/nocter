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

The bounded front end includes closed records, named/computed fields, local named function values,
[statically witnessed inputs](history/reviews/v2.0.0-callable-input-witnesses.md), and
[input-dependent callable results](history/reviews/v2.0.0-dependent-callable-results.md).
The source corpus has 283 cases: 147 accepted units, 122 checking rejections, 8 header rejections
and 6 syntax rejections. Returning a taking formal now preserves that input witness; each call
application substitutes its own selected actual bindings. Distinct calls cannot conflate their
results, including through joins/replacement or recursive permutations. Generic formal values
retain transfer and symbolic cleanup; an exact named caller result can separately establish Copy.

Body-owned application types replay over the immutable header prefix into one canonical authority.
A shared visitor projects every body data slot; header source binding remains separate from type
lookup. Result summaries use reachable returned values and prepared producers. Static application
instantiation uses those established summaries and frozen bindings. Both use one finite equation
solver; no retyping, signature search or expanding witness names. The same private instance graph
consumes closed named/formal results through its existing binding/payload maps. No CLI/native or
performance claim follows.

[Deferred supply](history/reviews/v2.0.0-deferred-value-supply.md) closes Copy-versus-move before
initialization over one unchanged control graph. Do not recover Copy for a generic formal from a
known concrete caller or make a header cache depend on body syntax.

The [type foundation and interruption checkpoint](history/reviews/v2.0.0-type-foundation-and-interruption.md)
closes signature-only evidence for interrupted applications. The unchanged former r110 source is
now accepted as `a137-dependent-result-unknown-input.nct`. Unproduced structural contracts remain
distinct from actual payloads through the accepted Unit and private instance graph. They cannot
enter reached value use, input preparation or invocation, and acquire no Copy/move/environment
cleanup operation. Conditional typing retains the normally producing branch's identity while
checking both known shapes. The [type-system foundation](spec/language/values-and-types.md#type-system-foundation)
owns identity/compatibility/inference boundaries; full generic/interface/capability closure remains
explicitly separate work rather than an implicit released-language fallback.

The [owned-capture checkpoint](history/reviews/v2.0.0-owned-capture-producers.md) admits a first
single-value environment with explicit parameter/result data. Formation uses Copy-or-move;
reached body operations infer func/mut/once over one retained graph before ordinary initialization,
access and resource admission. Nested bodies share canonical types, source byte/locator guards and
flow admission with named bodies. Returned captured-code witnesses, static callback input supply,
partial consuming return and interrupted preparation are source-checked. Uninvoked environment
payloads project without creating code instances. Captured-code invocations/bindings remain an
explicit UnsupportedCapturedCode private-target boundary; no executable or native claim follows.

Next close captured code in the existing private instance graph. Introduce a code identity that
selects a named body or checked closure body, retain explicit environment versus ordinary inputs,
and specialize already selected witnesses without retyping or erased dispatch. Preserve the
inferred body invocation separately from its supplied adapter view and account for adapter-owned
environment cleanup. Add direct/value/formal calls, returned environments, recursion, distinct
lexical producers and uninvoked-code cases before claiming this target admission. Closure bodies
with nested producers or factory-result applications need code-scoped operation identities; they
must not borrow the enclosing named function's result-application domain.

Then expand multi-value environments, contextual closure completion and inferred read/update
captures with retained-state/write footprints. Preserve R/D/S and source-context Copy through every
carrier; the current environment's empty outside D/S follows only its exact closed data schema.
Do not generalize that property to capability/callable captures. Keep each source scope, schema,
body proof and target consumer closed together, without hidden clones or a second owner ledger.

Then close physical environment/argument realization and executable cleanup before MIR/native
admission. General captured producers remain separate until R/D/S, retained-state/write footprints and
destruction are proved under the same authority. No human-decision blocker remains for this work.

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

- `spec/` owns published behavior; `development/spec/` owns adopted unreleased behavior in its stated scope. No cross-version fallback or duplicate editable rule is permitted.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
