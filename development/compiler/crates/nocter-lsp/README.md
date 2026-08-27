# nocter-lsp

## Responsibility

Own the dependency-light Language Server Protocol data model, JSON codec, transport messages,
lifecycle state, URI handling, and request/response schemas.

## Contract

The crate converts protocol JSON into validated LSP values and encodes responses. It contains no
Nocter compiler, workspace, filesystem, semantic query, or feature-selection logic.

## Internal Responsibilities

- JSON-RPC and LSP message decoding/encoding
- initialization and lifecycle state
- UTF-16 protocol coordinates and URIs
- feature parameter and result schemas
- outbound request/session tracking

## Invariants

- Protocol validity is decided before semantic handlers run.
- LSP coordinate values do not enter compiler storage directly.
- The crate remains reusable without the Nocter compiler pipeline.
