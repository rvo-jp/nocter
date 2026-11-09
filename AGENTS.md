# Nocter Agent Rules

These rules apply throughout the repository. Paths are relative to the repository root.
Maintain repository-wide agent instructions only in this file.

## Working Principles

- Leave enough information in the owning documents for a new session to establish current goals,
  contracts, implementation limits, remaining work and blockers without conversation records.
- Keep one authority for each code decision. Derive downstream representations, caches and indexes
  from it; do not maintain parallel sources of meaning. Prefer a coherent design over a local patch.
- Keep one authoritative document for each fact or contract. Link to that owner instead of copying
  the same information into summaries, plans or catalogs.
- Keep directories and documents current. Git commits and tags are the only history: remove
  superseded specifications, history directories, chronological ledgers, session logs and completed
  work reports. Retain rationale and evidence only when they explain a current decision or problem.
- Commit at coherent, verified boundaries. Inspect `git status --short` before editing and preserve
  unrelated user changes. Do not stop merely to report progress or combine unrelated work.
- Question existing code and directory boundaries before extending them. Evaluate them against the
  user's purpose; their existence alone does not justify retaining them.
- Review affected code during implementation and before committing. Periodically examine the
  broader design, duplication and document/directory structure; improve them when concrete benefits
  outweigh disruption. Avoid superficial churn and unnecessary abstractions.
- Address bugs when found. Fix a bounded defect and verify its behavior. If a larger redesign is
  needed, report its impact and record the affected owner and next action in `development/TODO.md`.
- Periodically check the user's goals and priorities and propose better designs with concrete
  examples and tradeoffs. Use documented goals for routine decisions; ask when intent is genuinely
  missing. Do not repeatedly seek approval for delegated work.
- Before running a test, identify the behavior or risk it must verify and whether that test
  actually observes it. Question existing assertions and test selection; unrelated passing tests
  do not establish correctness. Use the smallest sufficient evidence for the affected consumers.
- Periodically challenge existing rules and procedures, including this file. Revise those that no
  longer serve the project's goals instead of following them mechanically. Do not weaken a product
  contract or test expectation merely to obtain a passing result.

## Task Context and Authorities

Start from [development/README.md](development/README.md) and
[development/TODO.md](development/TODO.md). Read relevant owners rather than unrelated documents as
a startup ritual. Before compiler changes, read `development/compiler/README.md`, the relevant
architecture contracts and each affected crate's colocated README completely.

- `development/spec/` owns adopted unreleased contracts; `spec/` owns published contracts.
  Derive behavior from the owning version's specification and explicitly selected platform standards.
  An unclosed v2.0.0 subject has no implicit fallback to the published language.
- Repository-root `std/**/index.nct` owns published library declarations and API documentation.
  Repository-root `std/README.md` assigns longer observable subjects to behavior guides.
  Adopted unreleased library changes belong in `development/spec/library/`.
- `development/architecture/overview.md` owns compiler-wide authority boundaries;
  `development/architecture/README.md` locates cross-component contracts. Follow one-way semantic
  ownership; diagnostic source locations and rendered names must not become semantic identity or
  an alternative lookup authority.
- Each compiler crate's colocated README owns its responsibilities and local invariants. Every
  workspace member must have one. Rust source/rustdoc owns exact APIs; manifests own membership
  and dependencies. Architecture documents must not duplicate crate internals.
- `development/TODO.md` owns remaining work, blockers and unresolved findings. Remove completed entries.
  `development/design/` holds current unresolved design, not competing specifications or archived
  decisions.

Do not use an archived compiler, its tests, diagnostics, generated structures, runtime behavior or
published binaries as a language oracle. Do not restore, copy, port, execute or depend on an archived
implementation, or copy an earlier standard-library implementation to bootstrap the new compiler.
Git history may explain a change but cannot fill a missing current contract.

## Delegated v2.0.0 Work

Language semantics, internal models, analysis, optimization, implementation, verification,
refactoring, documentation maintenance and ordinary work order are delegated. Resolve them against
the documented goals. Explain consequential choices without turning routine work into approvals.

Obtain approval before a major change of syntax-design direction, including revisiting an adopted
decision. Examples are replacing basic binding/input/result/transfer notation or its meaning,
changing a pervasive default, or introducing a central source concept that substantially changes
ordinary reading and writing. Judge the programmer's model, not the number of edited files.
Clarifying or completing syntax within the adopted direction remains delegated.

Prepare representative before/after code, the problem, alternatives, consequences, migration impact
and a recommendation before asking. Keep a pending proposal separate from adopted contracts; do
not implement it as accepted behavior before approval. Continue independent authorized work.

Audit the owning chapter and its references before calling a subject unspecified. Reduce a semantic
question to a minimal program, compare alternatives against the goals and resolve it within the
delegated scope. Write the adopted rule into its owning specification before implementation or
conformance admission. Keep current rationale, counterexamples and proof obligations with that
subject. Use primary research as evidence, not authority over the project's goals.

Distinguish adopted design, implemented scope and verified support. Document actual implementation
limits with the responsible component and link to that owner. Planned fixtures are not passing
conformance. Do not silently amend released behavior or claim unmigrated APIs are supported.

Release publication, replacement of qualified artifacts, external messages and unrelated product
decisions require separate authorization. Preserve qualified artifact contents and published tags;
directory cleanup does not authorize rebuilding or replacing a release.

## Documentation and Repository Placement

Write public documentation in English, including the root README, both specification trees,
contributor guides, architecture and website content. Internal instructions and
`development/TODO.md` may use another language when useful.

Keep the repository root user-facing. Its README explains the product, installation, released use
and further reading; it may link to development without copying contributor workflows or plans.
Keep compiler source, contributor documentation, local packaging and generated candidates under
`development/`. Keep this repository-wide policy at root. Scope Git exclusions and attributes to
the directory that owns the affected files.

Within each active version scope, give each normative subject one owner. Put language semantics in
`spec/language/`, target/ABI contracts in `spec/platform/` and observable compiler/editor behavior in
`spec/tooling/`. These paths apply within either specification tree. Promote qualifying contracts
by moving them, without retaining editable copies. Keep pending alternatives outside normative
specifications. README catalogs explain reading order and link to owners; generated navigation
derives membership from the directory structure.

Keep product sources and API documentation together under repository-root `std/`. Keep runnable
user packages in repository-root `examples/`; invalid and diagnostic fixtures belong under
`development/compiler/tests/fixtures/`. Use exact release or milestone names, not bare `v0`, when
identifying contracts or compatibility boundaries.

Edit source Markdown, not generated HTML. The website build excludes adopted unreleased contracts,
unresolved design and internal control files. Generated output belongs outside the repository and
must not be committed; GitHub Pages receives an artifact built from `main`.

## Verification

[development/verification/README.md](development/verification/README.md) owns command invocation,
evidence selection and required gates. Select checks only after the purpose assessment above.
Review semantic ownership and compatibility fallbacks as well as test results; do not count
unreached native/editor paths as evidence for a private front-end feature.

After public Markdown changes, run documentation generator tests, build into an explicit external
directory, and run `git diff --check`. Internal instruction/work-plan changes need consistency and
diff review; validate links when references change. Do not run compiler suites for a document or
path-only change without identifying an affected compiler behavior.
