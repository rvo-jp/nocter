# Contributor Documentation

Start with the owner for the information needed by the task:

- [Current work](TODO.md): next actions and blockers.
- [Development specification](spec/README.md): adopted unreleased language, library and tooling contracts.
- [Current design](design/README.md): goals, unresolved source grammar, realization requirements and optimization experiments.
- [Compiler workspace](compiler/README.md): component entry points and verification commands.
- [Compiler architecture](architecture/README.md): current cross-responsibility contracts.
- [Performance measurement](benchmarks/README.md): repeatable measurements and evidence requirements.
- [Development verification](verification/README.md): disposable compiler gates and evidence selection.
- [Release packaging](packaging/README.md): local candidate assembly and installed-artifact qualification.
- [Documentation generator](site/README.md): publication, navigation and external builds.

The [published specification](../spec/README.md), [standard library](../std/README.md) and
[examples](../examples/README.md) describe the product. Development design does not amend those
contracts or establish implementation support. The private value-access implementation and its
current source gaps are owned by the [cross-stage handoff](architecture/pipeline/value-access-declarations.md).

## Work Policies

[Agent instructions](AGENTS.md) own repository-wide maintenance rules, information placement,
delegated scope and syntax approvals. The root agent file only points to that owner.
Crate READMEs, source/rustdoc and manifests own local responsibilities, APIs and exact
membership/dependencies. Read those owners rather than reconstructing policy from earlier work.

## Repository Layout

```text
development/
├── AGENTS.md
├── README.md
├── TODO.md
├── architecture/      # current cross-responsibility contracts
├── benchmarks/        # repeatable measurement tools
├── compiler/          # compiler workspace and consumed source fixtures
├── design/            # unresolved current design and realization requirements
├── dist/              # ignored local candidate archives and installed home
├── packaging/         # assembly and installed-artifact qualification
├── site/              # documentation generator and website-only inputs
├── spec/              # adopted unreleased contracts
├── unicode/           # generated Unicode source data and its generator
└── verification/      # disposable verification entry points
```

Rust, Cargo and Node.js are development requirements. Exact commands and qualification
requirements belong to the linked component guides.

Git exclusions and attributes for development files live in this directory. Local generated
outputs belong here or outside the repository; the root agent file remains only for discovery.
