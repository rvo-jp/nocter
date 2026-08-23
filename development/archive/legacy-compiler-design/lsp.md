# Language Server Implementation

This document owns the compiler/LSP implementation boundary. Public capabilities and protocol
behavior are defined only by [Tooling and Editor
Integration](../../../spec/14-tooling-editor-integration.md). Snapshot lifetime and invalidation are
defined by [Immutable LSP Snapshots](lsp-snapshots.md).

## Architecture

The language server is a protocol adapter over compiler analysis:

```text
JSON-RPC request
  -> lifecycle and parameter validation
  -> URI/UTF-16 conversion
  -> immutable package snapshot
  -> analysis query
  -> presentation model
  -> LSP response
```

Protocol code must not parse Nocter declarations, walk syntax to rediscover semantic identity,
format signatures from source fragments, or infer types from display text. When analysis cannot
establish a semantic fact, the server returns no semantic result rather than inventing one.

## Semantic Occurrences

Resolver and typecheck facts are projected into exact `SemanticOccurrence` values. Each occurrence
contains a stable declaration identity, use/declaration role, semantic kind, exact focus range,
readonly state where relevant, and its owning source/package identity.

Hover, definition, references, rename, and semantic tokens consume those occurrences. Feature code
may filter by role or kind but may not perform an independent cursor walk. This keeps keywords,
owners, delimiters, whitespace, and member names from accidentally sharing one hover or token range.

Cross-compile-unit indexes translate transient numeric source IDs into package/path/span identities
before joining occurrences. Dependency and standard-library ownership remains attached to those
identities so refactoring cannot edit read-only sources merely because their filesystem path is
near the root package.

## Presentation

`analysis/presentation` renders normalized declarations from resolved symbols and specialized type
facts. It receives the visible owner spelling separately from canonical identity, allowing user
output to stay concise without losing exact navigation targets.

Resolved method signatures retain the number of owner generic parameters separately from method
generic parameters. Presentation therefore specializes an owner such as `Box<T>` without inventing
method arguments, and hover, completion detail, and signature help use the same method renderer.

Presentation owns:

- canonical type notation and precedence
- specialized generic/member signatures
- semantic owner qualification
- construction surfaces and default ordering
- callable source contracts and result provenance
- documentation extracted from declaration comments

Callable presentation is indexed by compiler declaration identity. Each entry retains separate name,
return-type, explicit-provenance, and semantic-signature anchors; editor hints never locate clauses
by scanning declaration text. Typecheck provenance remains lossless, while presentation projects it
to bounded result-storage summaries. That projection removes storage-independent branches and
scalar dataflow, deduplicates origins, and never exposes private aggregate field names.

LSP transport converts presentation blocks to Markdown and protocol structures. It does not retain
a second signature formatter or fall back to raw declaration source.

## Completion and Signature Planning

Completion candidates are analysis values with semantic identity, visibility, specialized type,
documentation, insertion text, and optional source-edit plans. Lexical scope, shadowing, receiver
capability, generic bounds, imports, and construction defaults are decided before protocol
conversion.

Automatic imports use compiler-owned edit planners. Planners preserve leading documentation and
import grouping, and their edits are reparsed in compiler tests. Protocol code only translates
source offsets and document versions.

`PackageSemanticIndex` stores each non-private export with the semantic visibility boundary
resolved by `SourceScopeMap`. Candidate selection compares that boundary with the requesting
module's exact package and module identity. It does not reconstruct access from source text or
special-case a path component named `std`; the implicit standard-library graph node supplies the
ordinary dependency path.

Signature help consumes resolved callable candidates and the parser's active argument context.
Recovery overlays may supply missing delimiters or placeholder operands long enough to run the
ordinary query, but cannot manufacture declarations, conformance, callable identities, or types.

## Diagnostics and Code Actions

Diagnostics originate from compiler phases as structured values with source spans, related
information, stable codes, and optional fix plans. The server groups and publishes them by snapshot
generation. It clears documents absent from the next complete publication set and never mixes facts
from different generations.

Code actions expose only compiler-produced edits. Unresolved imports, missing interface members,
and outcome-contract fixes share planners with direct compiler tests. Version checks occur at the
request boundary before edits are returned.

## Inlay Hints

Inlay hints are projections of retained binding types and source-visible provenance facts. Explicit
source annotations suppress redundant hints. The server does not rerun inference, reconstruct
provenance from hover text, or expose compiler-owned allocation dataflow. Callable semantic hints
attach to the indexed signature anchor after the return type or explicit result-provenance clause,
not to the callable name.

## Recovery

Open, incomplete source is analyzed through a temporary recovery overlay that reuses the current
snapshot's package graph and all other open document texts. Recovery is feature-local and never
replaces the authoritative generation.

Recovery helpers are narrowly scoped by syntax responsibility: delimiter closure, incomplete call
arguments, imports, member access, iteration headers, interpolation bodies, and provenance clauses.
They may make a parseable temporary source but must preserve original byte-to-source mappings and
must not publish recovered text. If resolution remains unavailable, degraded declaration labels are
rendered from normalized AST type notation; raw declaration substrings are not a signature format.

## Protocol Boundary

The driver owns initialization, shutdown, exit, document synchronization, workspace folders,
watched-file registration, request framing, and JSON-RPC errors. Accepted document versions advance
snapshot state; stale changes are ignored. Requests before initialization and after shutdown are
rejected without invoking analysis.

All source ranges are byte offsets internally. UTF-16 conversion occurs only at the protocol edge
against the exact source text held by the request's snapshot generation.

## Verification

Every editor feature needs two layers of evidence:

- focused analysis tests proving semantic identity, specialization, visibility, presentation, and
  exact source ranges
- framed JSON-RPC integration tests proving lifecycle, parameter validation, UTF-16 conversion,
  advertised capabilities, and response shape

Package-wide features additionally require open/closed document coverage, dependency read-only
coverage, invalidation coverage, and installed-home LSP smoke tests without repository-local
configuration. Raw-source fallback behavior is not an acceptable test oracle.
