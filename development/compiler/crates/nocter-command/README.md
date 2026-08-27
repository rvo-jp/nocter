# nocter-command

## Responsibility

Plan and execute user-facing Nocter commands by composing package, compiler, source-tooling, native,
and package-state contracts.

## Contract

The crate consumes parsed command arguments plus validated installation and filesystem inputs. It
publishes command results, artifacts, graph/inspection output, or typed failures. It does not own
process argument decoding, terminal rendering, language rules, or stage internals.

## Internal Responsibilities

- command schema and input planning
- package versus explicit single-file selection
- check, build, run, test, format, fetch, graph, and init composition
- output path and artifact publication plans

## Invariants

- A command selects its compilation mode explicitly and once.
- `check`, `build`, and `run` share the same target acceptance boundary.
- Artifact publication happens only after a complete successful result.
- Package mutation uses `nocter-package-state` transactions rather than ad hoc writes.
