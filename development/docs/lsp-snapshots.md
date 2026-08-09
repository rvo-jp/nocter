# Immutable LSP Snapshots

This document defines the compiler boundary adopted for v0.4.0 Phase 2. Public editor behavior is
specified in the tooling specification.

## State Ownership

`LspSnapshot` is the only analyzed state visible to normal LSP requests. One immutable generation
owns:

- the exact open-document texts and versions
- one locked, offline `PackageGraph` for each open package root
- compiler analysis rooted at each open document
- source dependencies retained by each document analysis
- diagnostics derived from those same compiler results

The mutable server document map is only an input to the next generation. Hover, completion,
signature help, definition, references, semantic tokens, and diagnostics read an `Arc<LspSnapshot>`
and cannot observe a partially rebuilt state.

A snapshot contains one document-root compile unit per open document rather than one artificial
compile unit containing every package file. This preserves normal import visibility and permits an
unreferenced file to be edited as its own module. Package identity, dependency namespaces, open
overlays, and generation remain shared across those analyses.

Every valid package graph also contains the immutable implicit `std` package selected by the active
Nocter home. Standard-library overlays participate in invalidation and navigation through that
exact graph identity; a directory name or dependency spelling does not confer standard-library
authority.

## Build and Publication

`SnapshotStore` owns the current generation. Document open, accepted document change, saved text,
document close, workspace initialization, and watched-file changes build a successor before
replacing the current `Arc`. Requests with unchanged inputs reuse the same generation.

The server is currently a serialized message loop. A build therefore cannot race a request or be
published after a newer edit. Stale `didChange` versions are rejected before construction.
Diagnostics carry the accepted document version, and semantic-token results carry the snapshot
generation as `resultId`. Background construction and cancellation become necessary only if the
server later introduces concurrent request execution.

## Invalidation

Successor construction reuses immutable analysis objects where their inputs remain valid.

| Change | Invalidated state |
|---|---|
| Open document text or version | That document and analyses whose retained source set contains it |
| Imported disk file notification | Analyses whose dependency trace contains a logical or canonical alias of the path |
| Open or closed overlay | The overlay document and its reverse importers |
| `nocter.nct` change | Its package graph and every document analysis using that graph revision |
| Workspace-root change | Every package context and document analysis |
| Unrelated file | No retained analysis |

Source dependency sets come from the frontend loading trace, not from every open document preloaded
into its `SourceMap`. The trace retains reached source identities and every attempted module
candidate, including `module.nct` and `module/index.nct` when neither exists. Failed parsing and
failed import resolution therefore preserve the exact dependency prefix reached before failure.
Creating a missing import, repairing a malformed import, or deleting a symlinked import invalidates
its reverse importers without rebuilding unrelated analyses.

Path identity retains three forms where available: lexical normalization, complete
canonicalization, and canonicalization of the nearest existing ancestor with a missing suffix
reattached. The third form is necessary for deletion notifications because the changed leaf can no
longer be canonicalized after removal.

Nested packages have separate package contexts and graph revisions. A manifest change in one does
not invalidate an unrelated package.

## Package-File Overlays

Open `nocter.nct` text is passed to package loading through `PackageSourceOverlay`. The overlay is
read-only and is accepted only by the locked, offline graph entry point. It never fetches a package,
generates a lock, or rewrites a package file. Ordinary open `.nct` documents are already supplied
to frontend loading through the shared `SourceMap` overlay.

Package parsing recognizes `nocter.nct` by its source role even when graph validation fails. This
lets malformed or incomplete package directives produce package diagnostics instead of falling
back to ordinary-module parsing.

Package graph loading also returns the set of manifests it attempted to visit. A failed transitive
manifest therefore remains an invalidation dependency after the graph itself has been discarded.
Repairing that manifest reloads the graph instead of reusing a permanently failed snapshot. No
previous successful graph is exposed while current manifest text is invalid.

## Protocol Boundary

The server advertises full-text synchronization and saved text. If a client includes text in
`didSave`, that text becomes the next immutable input before diagnostics are published. Clients
that support dynamic watched-file registration receive one `**/*.nct` registration after the
`initialized` notification; the existing watched-file handler remains valid for clients that send
the notification through static configuration.

The JSON-RPC loop has an explicit `Uninitialized`, `Running`, and `Shutdown` lifecycle. Requests
before initialization, repeated initialization, and requests after shutdown receive protocol
errors. Client responses to server-initiated registration requests are recognized as responses and
are not misclassified as unsupported requests.

## Recovery Analyses

Incomplete-call, literal, interpolation, region, and block recovery creates an ephemeral text
overlay derived from the current snapshot. It reuses the snapshot's package graph and all other
open document texts. Recovery never replaces the current generation and never reloads package
metadata.

## Package Semantic Index

v0.5.0 Phase 3 attaches one immutable semantic index to each package snapshot generation. The
index is built from root modules and explicit executable/test entries in the exact locked graph;
normal compiler loading follows their imports. It never scans the directory for otherwise
unreferenced `.nct` files.

Each compile-unit analysis has an independent `SourceMap`, so a numeric `SourceId` is never used as
a cross-unit key. The index normalizes occurrence identity to package identity, canonical source
path, declaration span, semantic role, and exact focus span. It can therefore join an open importer
with closed reached modules without weakening resolver identity.

Package-wide references, rename, automatic imports, code actions, and inlay hints all read the same
generation as diagnostics. Rename also checks the active `PackageId` as its write boundary;
filesystem containment alone is insufficient because path dependencies and package stores may be
nested below the package root. Source-edit planners live in compiler analysis and return byte edits;
the LSP layer only adds document versions and converts positions.

## Deliberate Limits

- The semantic index does not scan unreferenced unopened modules; exact graph reachability is its
  completeness boundary.
- Analysis objects are retained in memory only; there is no persistent on-disk semantic cache.
- Background parallel analysis and incremental parsing remain separate features.
- Filesystem module-segment completion may enumerate directories, but dependency ownership and
  aliases come only from the snapshot graph.
