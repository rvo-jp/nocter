# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

The user delegates Value Access Model semantics, analysis, and optimization design to the agent.
Resolve remaining model choices without approval, even when they affect accepted programs or
execution. Preserve adopted contracts and safety/resource requirements; record decisions,
research evidence, counterexamples, proof obligations, and distinguishing cases in the v2.0.0
plan/review. Consult only on source-design alternatives needing human evaluation of writing,
reading, or learning. Call omission, owned-capture/callable forms, interface disambiguation,
`throw`, construction entries, and S2's callable-name result mode with unmarked use sites are
selected. All six principal source directions and `throw reason` without an extra `^` are adopted.
Optional/unit construction spellings remain pending. Do not block
independent internal closure on these forms.
See the model's [closure scope](history/reviews/v2.0.0-values-and-access-operations.md#design-responsibility-and-closure)
and the scoped rule in `development/AGENTS.md`; unrelated language decisions/publication are not
delegated by this instruction.

Use the v2.0.0 plan's human-working-set criteria when closing design: directory structure, root
contracts, and direct `see` should reduce reading; inference should reduce writing and unnecessary
decisions without losing semantic or performance information. Type-driven automatic call borrowing
and omission of explicit transfer on owned returns remain deferred. The newly adopted access model
instead gives unmarked inputs one meaning and keeps updates and transfers explicit.
Continue internal inference work; evaluate disjoint-access and signature-maintenance proposals as
pending candidates, not already adopted syntax.

Close the detailed contracts and establish Nocter execution evidence in the
[v2.0.0 language design plan](history/milestones/v2.0.0.md). Prioritize compile-time optimization
and minimal application syntax while preserving the distinction between adopted design and current
compiler support. Rust comparisons and short-term delivery are not prerequisites; implementation
effort must not disqualify a better semantic model. Close the adopted Value Access Model
across parameters, receivers, projections, captures, and deferred execution. Unmarked input,
`&` update, and `^` transfer are adopted on declarations. Source-review S1 now adopts uniform
declaration-driven calls, with no optional receiver/argument markers. S3 adopts `^name` in the
closure capture segment for forced owned capture and `func`/`&func`/`^func` invocation types.
S5 adopts interface implementation maps for real binding/default/collision decisions and
`value.(Interface.member)(...)` only when ordinary lookup is ambiguous, with no redundant aliases.
S4 and its follow-up R1 adopt `throw reason`; the failure exit supplies transfer without extra `^`.
S6 adopts construction functions and `literal { field_names }` as public structural initializer
grants; public fields alone do not grant construction or announce a complete representation.
S2 adopts `func ^open(...)` / corresponding method result prefixes and contract-derived calls
and bindings. A binding receiving an owned result retains ownership without another assertion.
Standalone transfer in returns/storage stays explicit. The model's six semantic decisions are adopted: independent owned
storage and uniform transfer across copyability; owned-parameter update permission without a new
mutable local and no automatic failure rollback; left-to-right preparation/reservation before
invocation-time transfer/update; explicit projection dependencies; deferred access and ownership
from operation construction; and preservation of modes through abstraction. Close their remaining
details using the now-recorded [semantic closure](history/reviews/v2.0.0-value-access-semantics.md),
which resolves Copy eligibility, writable parameter slots, nested reservations, projection
dependencies, capture ordering, and deferred cleanup. The bounded root-place model check passes
3,456 states / 14,144 allowed transitions and rejects four invalid-state mutants; it is not a full
language proof or compiler conformance. Use the [readiness review](history/reviews/v2.0.0-implementation-readiness.md)
for conservative observation-preserving optimization, inferred-effect evidence, CTFE/type
dependencies, and contract/body identity gates. Review only the human-facing alternatives in
the [source-design record](history/reviews/v2.0.0-source-design-decisions.md) only for genuinely
unresolved source forms. The next agent-owned work is result-role closure, not another selection
of S2: specify result permission/origins and Copy, generic and interface forwarding, callback
types, outcome payload/carrier layers, async output and cancellation, and ordinary access-return
safety without `view`/`yield`. Follow the readiness review's
[remaining-work audit](history/reviews/v2.0.0-implementation-readiness.md#remaining-work-after-the-six-source-decisions).
Do not apply the never-adopted I-only ordinary `let` proposal, retain optional result assertions,
or choose ownership from copy/move implementation history. Keep optional/unit questions open. The
[pending syntax recommendations](history/reviews/v2.0.0-source-design-decisions.md#remaining-syntax-recommendations)
record adopted `throw reason` and propose ordinary `some(...)` and the `()` unit literal.
Migrate the selected rules
into their owning specification chapters and adjacent compiler contracts before implementation;
retain all proof and conformance obligations. Next close explicit outcome branch
selection and nested optional states, storable one-value `void`, direct comparison without view
fallback, and unhandled-failure/resource completion rules alongside unified `impl Type` and
interface operation contracts. Resolve their
interaction with contract-only roots, visibility, ownership, and cleanup before implementation.
Preserve the single `error` failure payload and cause-preserving propagation; per-API typed failure
payloads are excluded from the v2.0.0 plan, including when error information is successful data.
Audit the derived-ordering selector/text discrepancy separately; the planning finding does not
authorize altering qualified or published artifacts. Retain separate `func`/`method` calls without
UFCS and both outer/inner `impl` roles.

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

No blocker remains for the qualified v0.73.0 release candidate. v2.0.0 still needs source-design
selection and specification/implementation-contract migration as described above. The current
language deliberately rejects `notrap async` because `future T` has no drive-time trap contract;
v2.0.0 removes the authored modifier rather than adding one to future types.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
