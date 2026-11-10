# Nocter Agent Rules

These rules apply throughout the repository. Paths are relative to the repository root.
Maintain repository-wide agent instructions only in this file.

## Working Principles

- Keep the necessary current goals, contracts, implementation limits and remaining work in their
  owning documents so work can continue without session records.
- Keep one authority for information and responsibility in code. Derive downstream products and
  avoid unnecessary duplication, complexity and dependencies.
- Keep one authoritative document for each fact. Refer to its owner instead of independently
  maintaining copies elsewhere.
- Directories and documentation normally describe the current state. Git records changes; retain
  rationale and constraints needed to understand the current design in its owning documents.
- Commit at logical boundaries, preserving unrelated user changes and separating unrelated work.
  Continue across small implementation, verification and commit boundaries; progress reports alone
  are not a reason to stop.
- Address bugs and design problems when found. Establish their impact and fix tractable problems.
  Report substantial unresolved changes or decisions requiring the user, with causes and next work.
- Complete achievable work through implementation, appropriate verification and documentation.
  Report the cause and remaining work when a problem cannot be resolved.
- Evaluate user instructions and existing design against the actual goal; neither is automatically
  correct. Prefer accuracy and logic over agreement.

## Continuous Improvement

At appropriate points before, during and after implementation:

- Reassess the user's goals and requirements against the current design and implementation plan.
- Examine responsibilities, dependencies, directories, file placement and procedures for a more
  reasonable structure.
- Review changes for duplication, inconsistencies, unnecessary complexity and latent defects.
- Reconcile documentation with implementation and organize documents and directories when useful.
- Evaluate verification itself: identify missing cases, mistaken assumptions and defects the checks
  cannot detect, and improve the evidence where necessary.
- Weigh each improvement's benefit against its added complexity and choose a reasonable design.

## Quality

- Prioritize consistency and quality of the complete product over isolated local success.
- Do not equate a small patch with quality or omit changes needed to achieve the goal.
- Favor correctness, consistency, simplicity, maintainability and verifiability.
- Find root causes rather than adding symptom patches or unnecessary exceptions.
- Passing existing implementations and checks does not establish that their design is optimal.

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
  `development/design/` holds current unresolved design, not competing specifications. Retained
  milestone, review, release-audit and superseded-design records belong under `development/history/`;
  their catalogs and links remain valid without presenting them as current public guidance.

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
