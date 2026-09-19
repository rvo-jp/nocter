# nocter-archive-extraction

## Responsibility

Materialize one bounded gzip-compressed tar archive into an empty physical directory without
allowing archive data to escape or reinterpret the destination filesystem.

## Contract

Callers supply the compressed bytes, an empty staging directory, and explicit resource limits.
The crate validates physical entry kinds and normalized relative paths while materializing regular
files and directories. It does not interpret package, release, manifest, trust, or publication
semantics.

## Invariants

- Absolute, parent-relative, non-Unicode, duplicate, and non-portable paths are rejected.
- Symbolic links, hard links, devices, FIFOs, and other special entries are rejected.
- Entry count, expanded regular-file bytes, path depth, and compressed input bytes are bounded.
- Existing destination content is never overwritten.
- Executable intent is reduced to portable `0755` or `0644` file modes on Unix.
- Callers publish or discard the staging directory; extraction itself never selects a final path.
