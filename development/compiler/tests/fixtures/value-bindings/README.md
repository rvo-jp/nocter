# Planned Value Binding Inputs

These sources are derived from the owning future
[bounded value-binding grammar](../../../../../spec/language/syntactic-grammar.md#bounded-value-binding-fragment)
and [input/value rules](../../../../../spec/language/value-access.md). Each file represents one
supplied private module. It is not a runnable package or a contract-only `index.nct`.

**Compiler status: planned, not consumed.** The production `V2CoreUnit` path still implements the
pre-S12 checkpoint. No production parser/checker test currently reads this manifest. These expected
verdicts are design requirements, not passing compiler tests. Do not point the old parser at them
and reinterpret its verdicts as new-language semantics. Do not implement a parallel source-text
parser/checker in a corpus inventory script.

`manifest.json` identifies each source once and records the intended acceptance/rejection and
its distinguishing reason. The owning specification supplies semantics; the eventual consuming
crate tests own assertions, source projections and diagnostics. The corpus has fourteen intended
acceptances, twelve checking rejections and four syntax rejections. It covers default-read and
taking inputs, local values/aliases, Copy/move, fixed/mutable state, RHS-first replacement, unit,
branch/loop initialization, direct reservations and evaluation-temporary cleanup. An `error` input
supplies opaque move-only data without a constructor or dependency-bearing nominal extension.

Replace parser/declarations/typing/flow/access/resources as one accepted-unit boundary, then wire
these sources into production consumers and record actual results. Retire incompatible pre-S12
expectations at that boundary. The intended parse goal remains `V2CoreUnit`; no user-visible
source version, compatibility mode, released-language glob or native path is added.

The [parameter-default review](../../../../history/reviews/v2.0.0-parameter-defaults.md) records the
choice and tradeoffs. Generic/closure cases discussed there require later grammar extensions;
they are not smuggled into this four-type corpus. Full value-access D/S, public modules, projections,
capability schemas, outcomes/async and native execution require their own conformance.
