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

The parser returns `PackageFile { manifest, root_module }`. Manifest validation, package graph
loading, and generated lock rewriting do not enter ordinary semantic analysis. The module portion
follows the same resolver, typechecker, formatter, JSON AST, and LSP paths as every other module.

## Identity Model

`#name` is presentation metadata. Filesystem paths and dependency aliases are not semantic package
identity.

- root and path packages derive a stable digest from their canonical package root
- Git packages derive identity from the declared source URL plus exact commit
- archive packages derive identity from the declared source URL plus SHA-256 content digest
- `ModuleId`, `ExecutableId`, and `TestTargetId` contain `PackageId`
- dependency aliases are scoped by the declaring `PackageId`

`ModuleId` contains a typed `ModuleKey`. `ModuleKey::PackageRoot` identifies the code in
`nocter.nct`; `ModuleKey::Path` identifies an ordinary file or directory module. Consequently an
omitted executable `entry` selects `nocter.nct`, while the explicit path `entry: "."` selects
`index.nct`. No sentinel path string represents the package-root module. Path keys are derived from
the resolved canonical source path, not copied from manifest spelling, so equivalent spellings and
in-package symbolic-link aliases cannot split one source into multiple module identities.

Two packages may therefore bind the same alias to different package revisions without flattening
the graph or colliding in the package store.

## Responsibility Boundaries

`package/graph.rs` owns graph traversal, cycles, scoped aliases, and the CLI/LSP snapshot.
`package/store.rs` locates an already identified package in the package-local store and then the
shared Nocter-home store. `package/fetch.rs` alone executes Git, downloads archives, verifies
content, and installs immutable package trees. `package/lockfile.rs` alone rewrites generated lock
data in `nocter.nct`. `package/modules.rs` is the shared explicit-module resolver.
`package/targets.rs` owns common target fields and executable declarations;
`package/test_targets.rs` owns test validation and typed test identity. Both CLI and LSP consume
these results instead of interpreting manifest strings independently.

Graph construction computes effective locks in memory and validates the complete transitive graph
before requesting any manifest rewrite. A failed graph may populate immutable cache entries, but it
does not leave a partial generated `#lock` in any mutable root or path package.

The store never resolves an import by package name. It receives an exact `PackageId`. A package in
Nocter home with a matching display name or alias is irrelevant.

## Import Classification

Public import namespaces are defined by [Modules and Use
Declarations](../../spec/01-modules-use.md). The resolver classifies each parsed path into a typed
namespace before filesystem discovery. Downstream loaders receive the namespace and owning
`PackageId`; they never reinterpret path spelling or search unrelated roots.

## Dependency Sources and Generated Locks

Public dependency and lock behavior is defined by [Command Line
Interface](../../spec/15-command-line-interface.md). Internally, manifest validation produces typed
source requests and typed exact locks before graph traversal. Graph construction computes effective
locks in memory; only `package/lockfile.rs` may publish a validated generated lock. Inspection and
LSP loaders use the same graph model with publication disabled.

Downloaded Git metadata is removed before installation. Archive paths, extracted canonical paths,
manifest presence, symbolic-link escape, package module escape, package-file symlink escape, nested
package crossing, and target entry escape are validated before a package can enter analysis.

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

Package commands build a target plan before semantic analysis. The plan deduplicates modules by
`ModuleId`, so a package-root executable is not checked a second time as the ordinary root module;
distinct executable names may still emit distinct artifacts from one entry module.

## Editor Contract

The LSP locates the nearest containing `nocter.nct`, bounded by the opened workspace, and loads the
same locked graph as the CLI in offline mode. Nested packages are independent package roots.
Hover, completion, definition, references, diagnostics, and semantic module analysis use graph
identity. Manifest directive names, dependency aliases, and target entry values use exact
source-backed semantic ranges. Executable and test entries navigate through the shared module
resolver. Test execution is owned separately by `driver/test_command.rs`; it consumes resolved
`TestTarget` values and never reparses manifest metadata.
