# nocter-installation

## Responsibility

Locate and validate one installed Nocter home and install one already acquired, explicitly trusted
release artifact through a recoverable filesystem transaction.

## Contract

The crate consumes explicit or process-derived installation paths and returns immutable validated
installation facts, or an explicit local artifact-install request and result. Command and workspace
layers use those contracts; compiler stages never inspect the environment or installation layout
themselves. This crate neither downloads artifacts nor chooses which digest to trust.

## Internal Responsibilities

- real executable and Nocter-home resolution
- exact manifest v2 decoding and component digest validation
- compiler/standard-package compatibility
- installed package-root location
- exact archive-byte verification and candidate-home validation
- same-parent fresh publication, active-home replacement, and interrupted-transaction recovery

## Invariants

- The manifest binds the exact compiler file and complete regular standard-library tree.
- Standard-library validation consumes the language-owned module-root file name.
- A configured home accepts the running compiler only when its digest equals the manifest-bound
  compiler digest.
- Environment and executable-path reads occur at the outer boundary.
- Invalid, corrupted, or partially updated homes cannot supply a toolchain snapshot.
- The bytes checked against the caller's digest are the same immutable bytes passed to extraction.
- A candidate is completely validated before the destination changes.
- An existing destination can be replaced only when it is the active validated home.
- Unmarked transaction paths are never treated as installer-owned state.
