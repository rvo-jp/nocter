# Nocter Repository Rules

## Durable Context and One Authority

Keep the information needed to continue work in the repository. A new session must be able to
establish current goals, contracts, implementation limits, next work and blockers from the owning
documents and code, without recovering conversation history. Update those owners as work changes.

Each fact, contract and semantic decision has one authoritative owner. In code, keep one source
for each decision and derive downstream representations from it. Caches, indexes and generated
views must remain derived consumers, never independently maintained sources of meaning. In
documentation, link to the owner instead of copying its content into summaries, plans or catalogs.
Choose clear responsibilities and a coherent whole over a convenient local patch.

## Current State and Git History

The directory structure and documents must describe the current project. Git commits and tags
are the only archive of earlier states. Do not create or retain history directories, superseded
design documents, chronological decision ledgers, session logs, completed-work reports or old
specification copies. Replace or remove obsolete content; use Git to inspect its earlier form.

Keep rationale, alternatives and evidence only when they explain a current decision or unresolved
problem, and place them with that subject's owner. Active plans contain remaining work and
blockers; remove completed entries. Current release readiness and verification requirements may
be documented, but test-run journals and past release-audit narratives belong in Git history.
Preserve published tags and artifacts; changing the working documentation does not authorize
replacing a release. Published and unreleased contracts remain distinct active scopes.

## Design, Review and Bugs

Question existing code, rules and directory boundaries before extending them. Evaluate them
against the user's purpose, rather than treating their existence as justification. Propose a
better design when concrete benefits outweigh its costs; explain consequences with examples.

Review the affected code and documentation during implementation and before each coherent
checkpoint. During sustained work, periodically reassess the broader design, duplication,
directory structure and documentation placement. Refactor and reorganize proactively when it
improves the whole result; avoid superficial churn and unnecessary abstractions.

Do not leave a discovered bug unattended. Fix a locally bounded defect when it can be corrected
and meaningfully verified without a larger redesign. If it needs substantial design work, report
its impact and record the unresolved problem, affected owner and next action in the current work
plan. Keep implementation gaps visible instead of hiding them behind compatibility paths.

## Goals, Progress and Commits

Periodically confirm the user's goals and priorities during sustained work, especially when new
findings or tradeoffs could change the intended outcome. Propose better approaches proactively.
Use the documented goals to guide routine decisions; ask concise questions when intent or
requirements remain unclear. Purpose checks must not become repeated approval requests for
already delegated work. Continue independent authorized work while awaiting clarification.

Create commits at appropriate coherent, verified boundaries so progress remains recoverable and
reviewable. Do not accumulate unrelated work into one commit or stop merely to report progress.
Before editing, inspect `git status --short`; preserve unrelated user changes and never stage,
revert or rewrite them. Verification must exercise the behavior affected by the change; passing
unrelated tests is not evidence of correctness.

## Public Documentation Language

Write all public-facing documentation in English, including the repository `README.md`, `spec/`,
`development/spec/`, `development/README.md`, `development/architecture/` and website content.
Internal instructions and work plans excluded from publication, such as `AGENTS.md` and
`development/TODO.md`, may use another language when it improves communication.

Edit source Markdown rather than generated HTML. After changing public Markdown, run the
documentation generator tests and build into an explicit directory outside the repository. Do
not commit generated website output; GitHub Pages receives an artifact built from `main`.

## Documentation Placement

Keep root public documentation user-facing. The root `README.md` explains Nocter, installation,
use of the released product and where to read more. Put contributor setup, repository-local
workflows, implementation status, active plans and compiler-development documentation under
`development/`. Root documentation may link there without duplicating it. Internal control files
such as `AGENTS.md` are not public documentation.

Published language, platform and tooling semantics belong under `spec/`. Adopted unreleased
contracts belong under `development/spec/`, which is excluded from website publication and
search. Each active version scope has one normative owner per subject. Promote qualifying
contracts by moving them, without retaining editable copies in both trees. Pending alternatives
belong with the current unresolved design work, not in the normative specification or an archive.
Adopted unreleased library changes belong under `development/spec/library/`.

Published standard-library declarations and declaration documentation belong to each root
`std/**/index.nct`. `std/README.md` assigns each longer observable subject to one behavior guide.
Keep product sources and their API documentation together under `std/`; contributor-only
implementation contracts belong under `development/`.

Within `spec/`, keep source semantics under `language/`, target and ABI contracts under
`platform/`, and observable compiler/editor behavior under `tooling/`. File names describe
concepts. README catalogs recommend reading order and link to owners without duplicating their
content or registering pages; generated navigation follows published source directories.

Keep complete runnable user packages under `examples/`. Diagnostic fixtures and deliberately
invalid source belong under `development/compiler/tests/fixtures/`.

## Version Terminology

Use exact releases or milestones such as `v0.2.0` or `v0.3.0 Phase 0` when identifying a contract
or active delivery scope. Do not use bare `v0` as a release name or compatibility boundary.
Distinguish adopted design, implemented behavior and verified support. Git owns prior versions;
the working tree must not retain competing historical specifications or status records.
