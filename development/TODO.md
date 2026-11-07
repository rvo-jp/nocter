# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

The user now adopts [S8's type-side access qualifiers](history/reviews/v2.0.0-source-design-decisions.md#s8-place-access-qualifiers-on-types).
Use `buffer: &Buffer`, `job: ^Job`, and `func open(...): ^File!` as direction; retire identifier-side
permission placement rather than retaining aliases. S2/S7 and the structural callable examples
remain marked pre-S8 records. Future specification sections explicitly flag their old fragments
as awaiting coherent migration; do not implement them as the selected source grammar.

Complete qualified-type inference is recommended, not yet a complete adopted replacement binding
policy: `let file = File.open(...)?` can infer `^File` without `: ^_`. That requires replacing
S7's fixed-I unannotated `let`. Preserve `var` removal and the I/U/X semantic obligations. Next
close local inference, existing-place expression disposition, receiver/forced-capture grammar,
callable-value authority versus invocation authority, qualifier scope through compound types,
and corresponding typing/diagnostic cases. Existing S7 finite checks validate their stated old
baseline, not the new qualified-type proposal. Do not count documentation checks as compiler or
soundness evidence.

The [four-goal syntax review](history/reviews/v2.0.0-syntax-coherence.md) otherwise remains an
unadopted candidate. C1 implementation-signature inheritance, C2 destination-directed ownership,
future-output grammar, and conservative origin defaults are not selected by S8. In particular,
do not silently remove explicit source transfers or infer moves from later uses. Present new
human-facing forms concretely; do not ask for approval of delegated internal model decisions.

The user delegates Value Access Model semantics, analysis, and optimization design to the agent.
Resolve remaining model choices without approval, even when they affect accepted programs or
execution. Preserve adopted safety/resource requirements and record evidence, counterexamples,
proof obligations, and distinguishing cases. Consult only on source alternatives needing human
evaluation of writing, reading, or learning. See the model's
[closure scope](history/reviews/v2.0.0-values-and-access-operations.md#design-responsibility-and-closure)
and `development/AGENTS.md`; unrelated language decisions and publication are not delegated.

Use the [v2.0.0 plan](history/milestones/v2.0.0.md) to reduce reading through directory structure,
contract-only roots, and direct `see`, and reduce unnecessary writing through inference. Rust
comparisons and short-term delivery are not prerequisites; implementation effort must not rule
out a better model. Keep adopted direction, future specification migration, and current compiler
support distinct. Retain declaration-driven calls without receiver/argument marker aliases;
separate `func`/`method` without UFCS; both outer/inner `impl` roles; interface mapping and
`value.(Interface.member)(...)` only for real ambiguity; factories and explicit
`literal { field_names }` grants. Public fields alone do not grant raw construction.

The [outcome/unit decisions](history/reviews/v2.0.0-source-design-decisions.md#outcome-and-unit-source-decisions)
remain adopted: `throw reason` without extra `^`, ordinary generic `some(...)`, storable `void`
with literal `()`, ordinary optional injection such as `let a: i32? = 10`, and implicit unit
completion. Preserve the single `error` failure payload and cause-preserving propagation,
including when an error value is successful data. Do not reopen these as approval blockers.

Continue the [semantic closure](history/reviews/v2.0.0-value-access-semantics.md) and
[result-contract design](history/reviews/v2.0.0-result-access-contracts.md): preserve distinct
referent, carried-payload, and storage dependencies; checked Copy evidence; reservation before
invocation-time commit; valid updates without automatic rollback; and ordinary callee cleanup
before output handoff. No suspended accessor or hidden guard extension. Owned closures/futures
can depend on outside owners; ready outputs and cancellation retain the required dependencies.
The finite root check covers 3,456 states / 14,144 transitions; the separate result check covers
its documented small handoff model. Neither is a general proof.

The [typing/migration review](history/reviews/v2.0.0-access-typing.md) records the pre-S8 boundary
recipe. Its finite check covers 4,864 attempts, 860 accepted results, 3,072 specialization checks,
1,152 Copy-independent I/U binding checks, six assignment cases, and one slot transfer; it rejects
11 invalid-result and four invalid-assignment mutants. Rework its local-source policy with the
selected inference rules, then validate general CFGs, delegation, joins, loops, cleanup, type
composition, outcome/async, captures, and full precedence. Follow the
[readiness audit](history/reviews/v2.0.0-implementation-readiness.md#remaining-work-after-the-six-source-decisions),
with its S8 status amendment, for remaining module/construction, effects/CTFE, platform, tooling,
and contract/body identity gates. Migrate each owning future specification before implementation;
do not claim all of v2.0.0 is specified. Audit the derived-ordering selector/text discrepancy
separately without altering qualified or published artifacts.

Close the newly adopted evaluator expansion, opaque return contract/body separation, and removal
of `noalloc`, `notrap`, and `blocking` through inferred effects. Preserve nonblocking future safety
and checked execution. Specify representation freedom and later-stage proof obligations before
changing fixed layouts, frame allocation, or stage boundaries. Lifetime automation remains a static
inference investigation; do not adopt implicit cloning, ARC/GC, or general implicit moves.

Use the milestone for the complete adopted scope, pending grammar questions, and conformance gates,
including expanded constant evaluation, tuple bindings, directive grouping, capture inference, and explicit
callable arguments. Investigation authorization alone does not adopt unresolved proposals. Record
autonomously resolved model rules separately from syntax that still needs human evaluation.

Await explicit publication authorization. Publication must tag the exact v0.73.0 release-content
commit and upload the retained qualified archive without rebuilding it, then record an independent
post-publication audit. Do not reopen v0.72.0; preserve its checked safety dispositions, qualified
artifact, public tag, and external audit as one immutable boundary.

Preserve the v0.72.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for the qualified v0.73.0 release candidate. v2.0.0 still needs exact grammar,
formal typing/validation closure, and
specification/implementation-contract migration as described above. The current
language deliberately rejects `notrap async` because `future T` has no drive-time trap contract;
v2.0.0 removes the authored modifier rather than adding one to future types.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
