# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. v0.73.0 Relational
Safety and Verified Traversal is a qualified release candidate. Relational facts, place-sensitive
invalidation, repeatable loop invariants, borrowed range construction, verified traversal
adoption, the cross-stage implementation audit, and installed-artifact qualification are complete.
Linux target work remains deliberately deferred.

## Next Work

Continue the [first access front-end slice](history/reviews/v2.0.0-first-access-slice.md): integrate
its declaration/type/permission and body-checking contracts and consume the
[44-case manifest](compiler/tests/fixtures/v2-access/README.md) in semantic phase tests.
Production `nocter-syntax` now implements the bounded `V2CoreUnit` goal: 38 parse with shape
snapshots and six reject at syntax. Syntax, target-selection, diagnostics, and source-tooling
focused tests pass (204 unit tests). No semantic acceptance or rejection is implemented for the
future goal. The inventory checker checks registration/shapes/ranges, not source semantics.
Close checked actions, source liveness, joins/backedges and conditional cleanup before claiming
this slice complete.

Do not wait for all optimization research, add a parallel Python compiler, silently mix released
std/borrow semantics, or expose a public source-version compatibility mode. A complete v2.0.0
compiler remains unimplemented. No human source choice blocks this bounded next task.

Verification follow-up: the full compiler gate passed its workspace/CLI/command stages, then
reported 131/132 native cases passing. The TLS plain-peer/HTTPS ALPN test failed with image exit
1; it subsequently passed alone and within all 12 TLS/network tests at four threads. No runtime
or test assertion was changed. The exact cause is unproved; keep the failed full invocation
visible. The remaining 118 LSP tests, no-default-features check, Rust documentation, and website
validation passed separately. See the first-slice review for the evidence boundary; this is not
one clean full-gate or release-qualification run.

S11 adopts `let edit = &buffer`. It creates active U at expression evaluation from an initialized
writable X/U place, not a fresh result or I access. A bare read of `edit` still infers I. Direct
call markers remain absent, including under grouping; call U activation still follows reservation
and operand preparation. This source choice is closed, not pending approval. Future owning
assignment/flow rules now specify RHS-first replacement, same-slot transfer/reinitialization,
conditional result modes, alternative access worlds, and conservative loop fixed points.
The new finite flow model reaches 56 states / 364 transitions, checks 1,640 accepted join
operations and four selected loop scenarios, and rejects ten state/transition mutants. It assumes
explicit access-end facts and does not calculate source liveness, execute cleanup, or prove the
full model sound.

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

The syntax goal has distinct root/access/update/transfer/unit nodes and no fallback to released
borrow productions. Ordinary `lex`/`SourceFile` preserve their current language; released target
selection rejects a future root rather than silently ignoring it, and formatting rejects it before
spacing. Do not bypass these integrity gates to reuse old lowering for the new model.

The owning future value-access/callable/token/syntax sections now migrate these selected forms;
released semantics remain unchanged. Full compound-type/precedence, outcome/async, pattern,
module/construction, effect/CTFE, platform, tooling, and compiler-contract migration remain gates.
No complete v2.0.0 compiler or proof is claimed. The finite typing exploration now covers S9;
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
operation-boundary recipe. Its finite check covers 9,920 attempts, 2,088 accepted results, 6,400
specialization checks, 3,744 Copy-independent binding/update checks, six assignment cases, and one slot
transfer. It rejects 18 invalid-result and four invalid-assignment mutants. Root reservation and
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

No blocker remains for the qualified v0.73.0 release candidate. v2.0.0 still needs the remaining full grammar beyond the first access fragment,
production checking/formal validation closure, and
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
