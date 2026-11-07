# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

S8/S9 now adopt type-side ordinary input/results and operation-directed local inference:
`let first = Buffer.create(...)?` owns its fresh result; `let next = ^first` transfers it;
`let inspect = next` reads it. Copy never switches these inferred modes. A U-returning call
supplies U, while reading that U variable supplies I. Explicit destination ownership cannot
supply a missing standalone move. Calls keep their contract-driven X transfer and prep/commit.

The user also explicitly selects canonical `self`/`&self`/`^self` receivers and `func`/`mut`/`once`
invocation types. Do not offer expanded receiver aliases or old invocation-prefix meanings.
Retain forced capture `(^buffer; ...)` as the same explicit transfer operation, not an identifier
annotation. The proposed `buffer: ^_` capture form is withdrawn. `once` consumes an owned instance
at most once; it does not require invocation or imply owning captures.

Autonomous initialization closure gives freshly constructed numeric/Boolean/unit and computed
primitive scalar values X, permitting `let count = 0; count += 1` without later-use inference.
Declared I outputs retain I. An explicit I destination can materialize a fresh Copy answer with
normal temporary cleanup and preserved D/S; it cannot retain a hidden move-only X owner.
Assignment checks start after earlier RHS actions and do not roll an explicit move back on failure.

The owning future value-access/callable/token/syntax sections now migrate these selected forms;
released semantics remain unchanged. Full compound-type/precedence, outcome/async, pattern,
module/construction, effect/CTFE, platform, tooling, and compiler-contract migration remain gates.
No v2.0.0 compiler or complete proof is claimed. The finite typing exploration now covers S9;
it does not parse sources or validate general CFGs, loan graphs, type constructors, or cleanup.

S10 closes the two latest source questions: qualifiers bind to the following type atom before
outcome suffixes (`&File?` versus `&(File?)`), and implementations retain complete signatures.
C1's abbreviated implementation heads are rejected because they force a body reader to open
another file to recover inputs and permissions. Keep the root authoritative, repeat full body
headers, and diagnose canonical mismatch/duplicates without private-overload fallback. Do not
reopen these selections as approval blockers. General stored access-qualified components remain
a separate extension, not implied by qualifier scope. C2's destination-directed standalone
transfer was not selected; retain explicit `^place`.
The earlier [four-goal proposal](history/reviews/v2.0.0-syntax-coherence.md) records its original
alternatives; the [source decisions](history/reviews/v2.0.0-source-design-decisions.md) own current
adoption status and [typing review](history/reviews/v2.0.0-access-typing.md) records validation scope.

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

The [typing/migration review](history/reviews/v2.0.0-access-typing.md) now records the revised
operation-boundary recipe. Its finite check covers 8,928 attempts, 1,848 accepted results, 5,760
specialization checks, 2,784 Copy-independent binding checks, six assignment cases, and one slot
transfer. It rejects 15 invalid-result and four invalid-assignment mutants. Root reservation and
result handoff explorations remain separate; no finite count establishes general soundness.
Continue general CFG/delegation/join/loop/cleanup validation and type/outcome/async composition.
Follow the [readiness audit](history/reviews/v2.0.0-implementation-readiness.md#remaining-specification-and-validation-work)
for all remaining module/construction, effects/CTFE, platform/tooling, and contract/body identity
gates. Migrate each owning specification before implementation. Audit the derived-ordering
selector/text discrepancy separately without changing qualified or published artifacts.

The [storage-optimization closure](history/reviews/v2.0.0-storage-optimization.md) now resolves
the first research prerequisite under delegation. Separate semantic R/D/S from physical placement
P; optional compiler-private backing may be removed with only its implementation-specific OOM
termination when a checked contract permits it. Preserve required/recoverable requests, validation,
callbacks, budgets, capacity/ABI deadlines, origin/allocator affinity, and logical cleanup. No
new source annotations, GC, implicit clones, or caller controls are introduced. Do not infer an
elision certificate from a name, X result, or missing authored modifier.

The owning future memory, callable/async, and std/mem/std/vec behavior sections now state the
boundary. The new finite storage check covers 64 storage profiles, accepts eight narrow elision
observations, rejects 62 unsafe schemas/eight mutants, checks 768 reuse cases (27 accepted), and
720 fusion trace pairs (200 accepted, 13 unsafe unconditional schemas rejected). These are assumed
certificate/finite-trace checks, not real summary inference, Nocter compilation, a general proof,
or performance measurements. It exposes a first-failure counterexample even with no I/O.

Next implement/validate interprocedural storage/effect certificates and path-sensitive byte-use,
cleanup, materialization-deadline, and drive-safety obligations. Account for runtime helpers by
executable graph closure. A stronger reuse/fusion plan may preserve metadata or logical cleanup;
do not turn this simple model's rejected cases into permanent language restrictions. No new
human source choice is pending from this audit.

The [research-to-experiment plan](history/reviews/v2.0.0-runtime-performance-research.md#turning-research-into-nocter-experiments)
continues to prioritize structural work removed over annotations. Preserve released contracts;
no broader resource-request elision, numerical speed claim, or mandatory comparison gate follows.
No external artifact has yet been reproduced for Nocter.

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
