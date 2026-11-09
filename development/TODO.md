# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

The user requested source-design closure before resuming executable lifecycle implementation.
The [module/operation checkpoint](history/reviews/v2.0.0-module-and-operation-contracts.md)
migrates directory/see/use/visibility, root/source roles, schema ownership, type behavior,
interface/default binding and file/item envelopes into owning development chapters. It fixes the
missing associated-binding production with one mixed conformance block. Closed public structural
records need no private schema copy; partial/opaque contracts require one private representation.
These are source design contracts, not additional compiler acceptance.

The [generic/constant/opaque checkpoint](history/reviews/v2.0.0-generic-and-constant-contracts.md)
closes selected argument/inference/type-selector, CTFE/final-data and result-identity rules. A
constant is a fresh resource-free value recipe; a static is an immutable place. Closed const
results can determine array types through the same evaluator. Proof, evaluator, serializer and
opaque-result compiler qualification remain unimplemented.

The [outcome access checkpoint](history/reviews/v2.0.0-outcome-access-contracts.md) closes
selected construction/forwarding, throw, nested optional, scoped payload projection and
failure-loss warning rules. Read/edit access to a payload does not create an owner; failure
propagation still needs an owned error. The ordinary some prelude contract has a development
owner. These rules and their cases are not admitted by the current compiler.

The [async/effect checkpoint](history/reviews/v2.0.0-async-and-effect-contracts.md) separates
construction/drive/cleanup, pending cancellation and ready-output disposal. Completion uses
authored edited-state bound B; pending cancellation permits Entry union B after cleanup, rather
than asserting that an unstarted body completed. Ready-output disposal also composes its own
outside-write schema. Conditional wait/progress effects remain inferred without modifiers.
Producer/await source roles are selected; runtime solvers, frames, library task/iteration and
entry/platform realization are unimplemented.

The [enum/pattern checkpoint](history/reviews/v2.0.0-enum-and-pattern-contracts.md) selects
complete versus opaque schemas, Copy policy and read/edit/take match/if-is operations. Tuple
leaves keep one input position plus semantic paths; named values are staged before remainder
cleanup. A destructor-bearing carrier stays complete. Async input receipt and pattern-entry
discard remain separate phases. Finite eligible enum values/patterns join the same const domain.
These are adopted contracts, not compiler, resource, evaluator or target qualification.

Next close literal/pack/expansion/coercion and iteration contracts, then remaining lexical/
expression/control, capability/library and package/platform subjects, including task/async
iteration. Audit the assembled grammar and integrated reading examples, then resume the bounded
executable-lifecycle task below. Keep draft productions and
future conformance obligations distinct from the 283 production corpus cases. No human-decision
or external-authorization blocker is established for this design work.

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
payloads project without creating code instances.

The [captured-instance checkpoint](history/reviews/v2.0.0-captured-code-instances.md) closes named
and captured bodies through one code identity and one private instance arena. Checked input layouts
freeze the environment channel and explicit local-slot order/operations. Edges retain prepared input
bindings, concrete result obligations and normal environment exits. The supplied view stays separate
from body invocation; an owned read/edit adapter environment is disposed before answer delivery,
whereas a consuming body owns its remaining cleanup. Recursive bindings share reserved instances;
runtime environment contents and adapter views do not duplicate a code body. The former
UnsupportedCapturedCode boundary is removed for this bounded producer scope. Source-selected
Copy/move and interrupted/unproduced evidence remain unchanged. No ABI/native claim follows.

Next lower this bounded graph into a closed executable lifecycle product. Realize environment
pack/unpack, prepared-bundle input receipt, staged answers, existing conditional resource steps and
normal adapter exits under one authority. A read/edit body invoked through once must dispose the
received post-call environment after the body finishes and before handing off its protected answer;
a consuming body must not receive a second destruction. Preserve caller snapshots, partial moves,
interrupted prefixes and exact body/input identities without another lookup, typing or Copy choice.
Keep trap/divergence behavior separate from normal cleanup. Only then admit physical layout/ABI and
MIR/native lowering for the explicitly closed data domain; do not claim executable cleanup from a
logical graph alone. Nested producer/factory-result closure bodies need code-scoped application
identities before expansion and cannot borrow their enclosing function's application domain.

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

Extend compiler acceptance of generic/callable/interface contracts, capabilities, outcomes/async
and public modules only under their closed owning contracts, followed by target/ABI and native
lifecycle realization. The four-type fragment's empty D/S evidence is an exact contract, not a
generic inference result. Remaining whole-language grammar assembly stays a draft.

Read each owning crate README before editing. The
[implementation design](history/reviews/v2.0.0-value-binding-implementation.md) retains the broader
36 adversarial requirements and capability/result/state-footprint proof obligations. Current
production conformance does not establish native execution, full-language safety or speedup.
Earlier [pre-S12 contracts](history/reviews/v2.0.0-access-checkpoint-contracts.md) remain historical
evidence; their incompatible executable fixtures and source paths have been retired. Keep current
released behavior and release artifacts unchanged.

Full v2.0.0 still needs module/type-operation source implementation and the remaining
generic/interface/CTFE proof and remaining effect/enum/outcome/async, library and platform closure/realization, plus
matching formatting/diagnostics/editor implementations. Migrate each remaining owning subject
before accepting it in the compiler. Audit the
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
