# nocter-filesystem

## Responsibility

Own one immutable read view that overlays accepted open-document bytes and versions over disk files.

## Contract

Package resolution, discovery, and analysis consume the same filesystem snapshot for one generation.
The crate publishes read operations and canonical overlay facts; it publishes no package-state,
download, write, or artifact-publication operation.

## Invariants

- One generation cannot observe different bytes for the same canonical path.
- Editor overlays are read-only and cannot enter persistent package transactions.
- Filesystem content never selects a semantic identity after discovery has closed its input.
