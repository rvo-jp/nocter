# nocter-package-cache

## Responsibility

Own the filesystem representation and content-integrity contract of one exact package cache entry.

## Contract

The crate seals an acquisition-owned package tree with a deterministic content manifest and returns
only roots whose current physical tree matches that manifest and its expected exact package
identity. Package resolution and package-state publication consume the same verification entry. The
crate does not select dependencies, interpret source declarations, fetch packages, publish cache
directories, or parse Nocter source.

## Internal Responsibilities

- deterministic streaming package-tree hashing
- exact package manifest encoding and validation
- physical file, directory, and symlink validation
- verified exact-package root capability construction

## Invariants

- A verified root contains a regular root `index.nct` and a valid root manifest.
- Every directory and source artifact included in the package tree contributes to one deterministic
  digest; filesystem enumeration order cannot change it.
- Symlinks, special files, non-Unicode names, and a reserved root-manifest collision are rejected.
- A cache identity mismatch or any content change invalidates the root before resolution can select
  it.
- Validation never follows a package-owned symlink or mutates package content.
