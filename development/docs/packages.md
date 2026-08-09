# Packages, Modules, Dependencies, and Locks

This document records the compiler architecture for package and directory-module loading. Public
syntax and command behavior are specified in the language specification.

## Three Independent Identities

A package root is a directory that directly contains `nocter.nct`. The compiler keeps three
concepts separate:

- `PackageId` identifies an exact package graph node.
- `ModuleId` identifies a directory module inside one package.
- `SourceId` identifies one physical source file for spans, invalidation, and editor navigation.

`nocter.nct` is the package file. It contains package documentation and directives only and is
never an `AstFile`. The package root module is the directory module rooted at `index.nct` beside
the package file.

A directory containing `index.nct` defines a module. The index is its module root source and the
only source allowed to declare public API. Other explicitly imported `.nct` files belong to the
nearest enclosing directory module; their basenames do not create module identities. A directory
without `index.nct` is only a source folder.

## Package and Module Identity

`#name` is presentation metadata. Filesystem paths and dependency aliases are not semantic package
identity.

- root and path packages derive a stable digest from their canonical package root
- Git packages derive identity from the declared source URL plus exact commit
- archive packages derive identity from the declared source URL plus SHA-256 content digest
- `ModuleId`, `ExecutableId`, and `TestTargetId` contain `PackageId`
- dependency aliases are scoped by the declaring `PackageId`

`ModuleKey::PackageRoot` identifies the root directory module. `ModuleKey::Path` stores a
normalized directory path for a child module. Equivalent path spellings and in-package symbolic
links cannot split one module identity.

## Source Composition

A private top-level `use ./search` that resolves `search.nct` is a source-composition edge. It does
not introduce a namespace or imported symbol. Source edges are canonicalized, idempotent, and may
form cycles. Semantic analysis first joins their connected source graph into one module namespace,
then collects declarations and resolves bodies while retaining every declaration's physical span.

Source composition is constrained at the frontend boundary:

- both files must have the same nearest enclosing `index.nct`
- only a private top-level bare `use ./path` may compose a source
- implementation sources cannot contain any non-private declaration
- a file and child directory module with the same logical path are an error
- unreferenced source files are not discovered or compiled

External, package-absolute, dependency, and standard-library imports resolve directory modules
only. Their public surface is therefore rooted in `index.nct`; physical implementation files never
become importable identities.

## Hierarchical Visibility

`SourceScopeMap` assigns every loaded physical source an exact package identity and directory
module path. All access consumers use that one map:

- private names require the same directory module, including its composed implementation sources
- `pub(./)` admits the declaring module subtree
- every `../` moves the subtree boundary to one semantic ancestor module
- `pub(/)` requires the same exact `PackageId`
- bare `pub` crosses package boundaries

The AST retains the authored relative scope. Resolver facts retain the source whose module anchors
that scope, which is distinct from the original declaration source after a re-export. A re-export
may narrow its target boundary but cannot widen it, including through a chain of re-exports.

## Implicit Standard-Library Package

The package graph attaches `<Nocter-home>/std` exactly once as an immutable toolchain package and
reserves dependency alias `std` for every graph node. User manifests cannot declare or lock that
alias. Imports inside the standard library resolve it back to the same package identity.

Standard-library authority is a package role in the graph and source scope map. It is not inferred
from a dependency name, display path, module spelling, or visibility. `pub(/)` is therefore an
ordinary package boundary; only the exact implicit `std` node may declare registered primitives or
satisfy closed runtime roles.

## Callable Contract Composition

`CallableBodyIndex` is built once after source composition and before symbol collection. It joins a
bodyless public callable in `index.nct` to one signature-identical private body in a reachable
source. Matching uses directory-module identity and canonical authored callable structure; later
stages do not repeat the match by name, path, or presentation text.

The index records two related identity layers:

- callable declaration and implementation spans share the public contract as their canonical
  callable identity
- implementation receiver, parameter, and literal-capture spans map to their corresponding public
  input identities

The second mapping is required because provenance and retained-mutation summaries cross the
callable boundary. Their body-local evidence retains physical source spans while their exported
summary keys and input origins use contract identities. Type checking, specialization,
buildability, and lowering inspect the physical body through the shared relation. Symbol surfaces,
calls, presentation, references, and rename use the contract identity. Definition and
implementation navigation select the appropriate side explicitly.

Source-backed callables do not form a separate compilation unit. All composed ASTs and their
resolved facts remain available to whole-module analysis, including recursive generic
specialization and drop dependency discovery.

## Responsibility Boundaries

`package/graph.rs` owns graph traversal, cycles, scoped aliases, and the CLI/LSP snapshot.
`package/store.rs` locates an already identified package in the package-local store and then the
shared Nocter-home store. `package/fetch.rs` alone executes Git, downloads archives, verifies
content, and installs immutable package trees. `package/lockfile.rs` alone rewrites generated lock
data in `nocter.nct`. `package/modules.rs` resolves target directory modules.

`package/targets.rs` owns common target fields and executable declarations;
`package/test_targets.rs` owns test validation and typed test identity. Targets use `module`; an
omitted executable module means `.`, while tests require an explicit module. Both CLI and LSP
consume resolved modules instead of reinterpreting directive strings.

Graph construction computes effective locks in memory and validates the complete transitive graph
before requesting any package-file rewrite. A failed graph may populate immutable cache entries,
but it does not leave a partial generated `#lock` in a mutable root or path package.

## Dependency Sources and Stores

Manifest validation produces typed source requests and typed exact locks before graph traversal.
Inspection and LSP loaders use the same graph model with lock publication disabled.

Downloaded Git metadata is removed before installation. Archive paths, extracted canonical paths,
package-file presence, root-module presence, symbolic-link escape, nested package crossing, and
target-module escape are validated before a package can enter analysis.

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
`ModuleId`, while the frontend independently deduplicates physical sources by canonical path.

## Editor Contract

The LSP locates the nearest containing `nocter.nct`, bounded by the opened workspace, and loads the
same locked graph as the CLI in offline mode. Nested packages are independent package roots.
Hover, completion, definition, references, diagnostics, semantic tokens, and invalidation retain
physical source locations while sharing directory-module identity. Directive names, dependency
aliases, and target `module` values use exact source-backed semantic ranges and the same resolver as
the CLI.
