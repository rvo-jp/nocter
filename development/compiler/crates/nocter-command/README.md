# nocter-command

## Responsibility

Plan and execute user-facing Nocter commands by composing package, compiler, source-tooling, native,
and package-state contracts.

## Contract

The crate consumes parsed command arguments plus validated installation and filesystem inputs. It
publishes command results, artifacts, graph/inspection output, or typed failures. It does not own
terminal rendering, language rules, or stage internals. Program arguments accepted after the
`run` separator remain opaque native OS strings and are never decoded by this crate.

Every source-analyzing command creates one ephemeral `nocter-compiler-computation` owner. Package
resolution and discovery obtain syntax through that owner's computed provider, and target
construction consumes its closed unit-analysis product through the sole session adapter. Commands
cannot invoke or order semantic stages.

## Internal Responsibilities

- command schema and input planning
- one-time `run` compiler/program argument partition and opaque launch transport
- package versus explicit single-file selection
- ephemeral compiler-computation lifetime and query-backed package/discovery composition
- check, build, run, test, format, fetch, graph, and init composition
- output path and artifact publication plans

## Invariants

- A command selects its compilation mode explicitly and once.
- `run` partitions at its first standalone `--`; only compiler arguments enter generic option
  parsing, and only the process launcher opens the resulting program-argument vector.
- Package-only and single-file-only commands resolve through typed input boundaries; they do not
  recover a narrower mode by matching a general package/file result.
- `check`, `build`, and `run` share the same target acceptance boundary.
- Command and workspace analysis differ only in computation-owner lifetime and presentation policy.
- Command presentation consumes the session's closed failure envelope; it does not reconstruct or
  narrow semantic recovery diagnostics from a primary error.
- Artifact publication happens only after a complete successful result.
- Package mutation uses `nocter-package-state` transactions rather than ad hoc writes.
