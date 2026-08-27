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
- manifest and component validation
- compiler/standard-package compatibility
- installed package-root location

## Invariants

- Installation identity comes from validated content, not directory spelling alone.
- Environment and executable-path reads occur at the outer boundary.
- Invalid or mixed-version homes cannot supply a toolchain snapshot.
