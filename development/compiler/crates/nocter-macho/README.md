# nocter-macho

## Responsibility

Write one deterministic ARM64 Mach-O executable image from an already encoded `Arm64Program`.

## Contract

The crate owns Mach-O headers, segments, load commands, offsets, alignment, entry metadata, loader
binding streams, and final bytes. It consumes explicit ARM64 data-pointer fixups and trusted typed
runtime-import slots, then emits the rebase and bind metadata needed for position-independent
readonly data. It is the sole authority mapping logical runtime-library identities to Darwin
install names and dylib ordinals. It does not perform instruction selection, select an imported
service, inspect a source declaration, validate package semantics, or publish artifacts.

## Invariants

- File offsets and virtual addresses derive only from the closed encoded program.
- Image writing cannot introduce or select executable symbols; it can only encode imports retained
  by the completed ARM64 program.
- One canonical loaded-library sequence drives load commands, bind ordinals, layout, and image
  identity; a framework cannot be added through a symbol-specific special case.
- Readonly data is writable only during loader fixups and is protected after rebasing.
- Imported symbols contribute to the deterministic image UUID and code signature.
- Equal input produces byte-identical output.
