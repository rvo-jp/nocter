# nocter-macho

## Responsibility

Write one deterministic ARM64 Mach-O executable image from an already encoded `Arm64Program`.

## Contract

The crate owns Mach-O headers, segments, load commands, offsets, alignment, entry metadata, and final
bytes. It does not perform instruction selection, semantic linkage, package validation, or artifact
publication.

## Invariants

- File offsets and virtual addresses derive only from the closed encoded program.
- Image writing cannot introduce or select executable symbols.
- Equal input produces byte-identical output.
