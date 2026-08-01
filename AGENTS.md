# Nocter Repository Rules

## Public Documentation Language

Write all public-facing documentation in English. This includes the repository `README.md`,
`spec/`, `development/README.md`, and `development/docs/`, plus release notes and generated website
content.

Internal agent instructions and handoff files that are excluded from the public documentation build,
such as `AGENTS.md` and `development/TODO.md`, may use another language when it improves team
communication.

Edit the source Markdown rather than generated HTML. After changing public Markdown, run
`node docs/build-docs.js` and commit the corresponding generated website changes.

## Documentation Placement

Keep public documentation authored at the repository root user-facing. The root `README.md` may
explain what Nocter is, how to install and use a released version, and where to find further
documentation, but it must not contain contributor setup, repository-local workflows,
implementation status, or milestone plans. Internal control files such as `AGENTS.md` are not
public documentation.

Keep all contributor and compiler-development documentation under `development/`. This includes
build and test instructions, repository-local packaging, architecture, implementation design,
milestone planning, maintenance policy, and handoff state. Root documentation may link to the
`development/` entry point, but must not duplicate its contents.

Public language and standard-library semantics belong under `spec/`; this is user-facing reference
material rather than development documentation.

## Version Terminology

Use an exact release or milestone such as `v0.2.0` or `v0.3.0 Phase 0` in public documentation.
Do not use bare `v0` as a release name or compatibility boundary. Released contracts remain
versioned records; active design documents must distinguish adopted direction from implemented
behavior.
