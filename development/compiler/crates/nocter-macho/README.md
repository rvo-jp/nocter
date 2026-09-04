# nocter-macho

## Responsibility

Write one deterministic ARM64 Mach-O executable image from an already encoded `Arm64Program`.

## Contract

The crate owns Mach-O headers, segments, load commands, offsets, alignment, entry metadata, and final
bytes. It consumes explicit ARM64 data-pointer fixups and emits the loader rebase metadata needed
for position-independent readonly data. It does not perform instruction selection, semantic
linkage, package validation, or artifact publication.

## Invariants

- File offsets and virtual addresses derive only from the closed encoded program.
- Image writing cannot introduce or select executable symbols.
- Readonly data is writable only during loader fixups and is protected after rebasing.
- Equal input produces byte-identical output.
