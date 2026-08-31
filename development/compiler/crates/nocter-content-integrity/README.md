# nocter-content-integrity

## Responsibility

Project physical files and regular directory trees into deterministic SHA-256 content identities.

## Contract

Consumers select an exact physical root and an explicit root-entry policy. The crate rejects
symlinks, special entries, non-Unicode relative paths, and files that change length while being
read. It publishes a typed digest; it does not assign package, installation, or release meaning.

The companion binary exposes the same library operations to release packaging. It contains no
second hashing implementation.

## Invariants

- A regular file digest is SHA-256 over its exact bytes.
- A tree digest includes normalized relative paths, entry kinds, file lengths, and file bytes in
  deterministic order.
- Excluded entries are explicit root entries; descendants and pattern-based exclusions do not
  exist.
- Package cache, installation validation, and packaging consume this one physical-content model.
