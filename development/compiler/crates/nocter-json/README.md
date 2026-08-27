# nocter-json

## Responsibility

Own the small deterministic JSON value, parser, and writer used by compiler protocols and metadata.

## Contract

The crate handles JSON syntax and representation only. LSP schemas, diagnostic schemas, manifests,
and command output policy belong to their respective consumers.

## Invariants

- Parsing rejects malformed input without partial domain interpretation.
- Serialization order is supplied explicitly by the owning domain.
- The crate has no dependency on compiler semantic stages.
