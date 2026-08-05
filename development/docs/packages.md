# Packages, Dependencies, and Locks

This document records the compiler architecture adopted for v0.4.0 Phase 1. Public syntax and
command behavior are specified in the language specification.

## Package Boundary

A package root is a directory that directly contains `nocter.nct`. There is no source-root
concept and the compiler never derives a project import root from an entry file. `index.nct`
remains only a directory module.

`nocter.nct` combines two source-level roles without combining their compiler responsibilities:

- its leading directives form a `PackageManifest`
- its ordinary imports and declarations form the package root `AstFile`

The parser returns `PackageFile { manifest, module }`. Manifest validation, package graph loading,
and generated lock rewriting do not enter ordinary semantic analysis. The module portion follows
the same resolver, typechecker, formatter, JSON AST, and LSP paths as every other module.

## Identity Model

`#name` is presentation metadata. Filesystem paths and dependency aliases are not semantic package
identity.

- root and path packages derive a stable digest from their canonical package root
- Git packages derive identity from the declared source URL plus exact commit
- archive packages derive identity from the declared source URL plus SHA-256 content digest
- `ModuleId` and `ExecutableId` contain `PackageId`
- dependency aliases are scoped by the declaring `PackageId`

Two packages may therefore bind the same alias to different package revisions without flattening
the graph or colliding in the package store.

## Responsibility Boundaries

`package/graph.rs` owns graph traversal, cycles, scoped aliases, and the CLI/LSP snapshot.
`package/store.rs` locates an already identified package in the package-local store and then the
shared Nocter-home store. `package/fetch.rs` alone executes Git, downloads archives, verifies
content, and installs immutable package trees. `package/lockfile.rs` alone rewrites generated lock
data in `nocter.nct`.

Graph construction computes effective locks in memory and validates the complete transitive graph
before requesting any manifest rewrite. A failed graph may populate immutable cache entries, but it
does not leave a partial generated `#lock` in any mutable root or path package.

The store never resolves an import by package name. It receives an exact `PackageId`. A package in
Nocter home with a matching display name or alias is irrelevant.

## Import Classification

Import syntax selects one resolver namespace before filesystem module discovery:

- `./x` and `../x` are relative to the importing module and may not leave its package
- `/x` is relative to the owning package root
- `dependency/x` starts at the dependency bound to `dependency` by the owning package
- `std/x` starts at the compiler-matched standard library
- any other first segment is an undeclared-dependency error

Filesystem-absolute source imports and project-directory shadowing of `std` are not supported.
Single-file mode has no package root; it supports module-relative imports and `std` only.

## Dependency Sources and Generated Locks

Dependency declarations are user-authored. Locks are generated in the same `nocter.nct` so source
selection and its exact result appear in one review diff.

```nct
#dependencies: {
    json: {
        git: "https://github.com/example/json.git",
        revision: "main",
    },
    http: {
        archive: "https://nocter.dev/lib/http-v1.0.0.tar.gz",
    },
    local_math: {
        path: "./packages/math",
    },
}

#lock: {
    format: 1,
    dependencies: {
        http: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        json: "git:7db21c1000000000000000000000000000000000",
    },
}
```

Git revisions select an update candidate; compilation uses only the locked commit. Archives are
identified by downloaded bytes. Path dependencies are deliberately mutable and have no lock
entry. The alias `std` is reserved.

An ordinary package command may create a missing direct lock but never changes an existing one.
`--locked` rejects required lock generation. `--offline` prohibits source resolution and fetching.
LSP always uses locked offline graph loading and never writes `nocter.nct` or accesses the network.

Downloaded Git metadata is removed before installation. Archive paths, extracted canonical paths,
manifest presence, symbolic-link escape, package module escape, manifest symlink escape, and
executable module escape are validated before a package can enter analysis.

## Physical Stores

The package-local store is checked first:

```text
<package-root>/.nocter/packages/<PackageId>/
```

The shared fallback is exact-identity storage:

```text
<Nocter-home>/packages/<PackageId>/
```

These paths are caches, not source declarations. Removing them does not change the graph selected
by committed `nocter.nct`; an online fetch recreates the same identities.

## Editor Contract

The LSP locates the nearest containing `nocter.nct`, bounded by the opened workspace, and loads the
same locked graph as the CLI in offline mode. Nested packages are independent package roots.
Hover, completion, definition, references, diagnostics, and semantic module analysis use graph
identity. Manifest directive names, dependency aliases, and executable module values use exact
source-backed semantic ranges; executable module values navigate to their selected module.
