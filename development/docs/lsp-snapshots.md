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

## Build and Publication

`SnapshotStore` owns the current generation. Document open, accepted document change, document
close, workspace initialization, and watched-file changes build a successor before replacing the
current `Arc`. Requests with unchanged inputs reuse the same generation.

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
| Imported disk file notification | Analyses whose retained source set contains the canonical path |
| Open or closed overlay | The overlay document and its reverse importers |
| `nocter.nct` change | Its package graph and every document analysis using that graph revision |
| Workspace-root change | Every package context and document analysis |
| Unrelated file | No retained analysis |

Source dependency sets come from files actually retained in a successful compiler analysis, not
from every open document preloaded into its `SourceMap`. A failed root analysis conservatively
depends on its root source.

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

## Recovery Analyses

Incomplete-call, literal, interpolation, region, and block recovery creates an ephemeral text
overlay derived from the current snapshot. It reuses the snapshot's package graph and all other
open document texts. Recovery never replaces the current generation and never reloads package
metadata.

## Deliberate Limits

- Phase 2 does not scan unreferenced unopened modules solely to build a global symbol index.
- Analysis objects are retained in memory only; there is no persistent on-disk semantic cache.
- Rename, code actions, inlay hints, and background parallel analysis remain separate features.
- Filesystem module-segment completion may enumerate directories, but dependency ownership and
  aliases come only from the snapshot graph.
