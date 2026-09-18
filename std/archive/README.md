# Streaming Tar Archives

The compiler-checked [`archive` module contract](index.nct) is the sole authority for public
signatures. This guide records the state, path, and body guarantees shared by those declarations.

`TarReader` recognizes strict POSIX ustar headers. It retains one 512-byte header and one normalized
entry path, but never retains caller input or a complete entry body. `TarStep.body` reports an exact
range of the current input, so callers can inspect, copy, or discard that range before resuming.
Entry metadata remains available through `TarReader.entry` after an `entry` event.

Header checksums and octal sizes are validated before an entry is published. The name and prefix
fields are combined, required to be UTF-8, and normalized by removing empty and `.` components.
Absolute paths, `..` components, empty normalized paths, malformed UTF-8, and paths beyond the
ustar capacity are rejected. This lexical validation performs no filesystem access.

Regular files and directories receive dedicated `TarEntryKind` values. Other type flags remain
observable as `unsupported(code)` while their declared bodies and padding use the same bounded
progress rules. Extraction policy decides whether to reject or skip them; parsing does not create
files, follow links, choose destinations, or publish partial state.

Two consecutive zero blocks terminate an archive. `finish` declares transport EOF so a partial
header, body, padding region, or single end marker becomes a precise terminal failure. Later calls
to a terminal reader consume zero bytes and reproduce the same classification.
