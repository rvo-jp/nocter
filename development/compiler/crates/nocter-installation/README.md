# nocter-installation

## Responsibility

Locate and validate one installed Nocter home and its compiler, manifest, standard package, and
package-store compatibility.

## Contract

The crate consumes explicit or process-derived installation paths and returns immutable validated
installation facts. Command and workspace layers use those facts; compiler stages never inspect the
environment or installation layout themselves.

## Internal Responsibilities

- real executable and Nocter-home resolution
- exact manifest v2 decoding and component digest validation
- compiler/standard-package compatibility
- installed package-root location

## Invariants

- The manifest binds the exact compiler file and complete regular standard-library tree.
- A configured home accepts the running compiler only when its digest equals the manifest-bound
  compiler digest.
- Environment and executable-path reads occur at the outer boundary.
- Invalid, corrupted, or partially updated homes cannot supply a toolchain snapshot.
