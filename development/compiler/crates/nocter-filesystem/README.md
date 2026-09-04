# nocter-filesystem

## Responsibility

Own one generation-local read view that overlays accepted open-document bytes and versions over
first disk observations.

## Contract

Package resolution, discovery, and analysis consume clones of the same source view for one
generation. `observe_file` resolves and reads a regular file as one value, then retains that first
result—including absence or failure—for the view's lifetime. The crate publishes no package-state,
download, write, or artifact-publication operation.

## Invariants

- One generation cannot observe different bytes for the same canonical path.
- Cloning a source view shares disk observations instead of reopening the filesystem.
- Editor overlays are read-only and cannot enter persistent package transactions.
- Filesystem content never selects a semantic identity after discovery has closed its input.
