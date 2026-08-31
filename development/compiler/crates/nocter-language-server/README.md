# nocter-language-server

## Responsibility

Own LSP process lifecycle, document events, workspace orchestration, and projection of typed analysis
results into protocol responses.

## Contract

The server consumes `nocter-lsp` messages, updates workspace documents, requests
`nocter-workspace-analysis` generations, and maps protocol-independent query results to LSP. It
cannot name checking, declaration, target-program, syntax-tree, or source-index storage.

## Internal Responsibilities

- initialize/shutdown and JSON-RPC request routing
- document open/change/save/close handling
- watched-file and workspace configuration handling
- semantic feature response projection
- atomic workspace-diagnostic projection
- workspace edit version projection

## Invariants

- Feature handlers do not implement semantic fallback or lookup.
- Expected unavailability is an ordinary result; only integrity failure becomes an internal error.
- One response uses one current immutable generation.
- Diagnostics use one complete post-transition workspace view. Contributions from shared physical
  sources are merged and deduplicated before one URI-global replacement notification is emitted.
- Removing one analysis scope cannot clear a diagnostic still contributed by another active scope.
- Protocol edits preserve analysis-validated grouping and accepted document versions.
