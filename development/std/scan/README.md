# Borrowed Scanning

The compiler-checked [`scan` module contract](index.nct) is the sole authority for exact public
declarations. `ByteCursor` and `TextCursor` retain a borrowed input and one forward byte offset.
They allocate no storage, never extend the input lifetime, and never turn an address into new
ownership.

`ByteWindow` and `TextWindow` retain the selected input and absolute half-open bounds rather than
manufacturing ownership from an address. A window selected by cursor advance is lent from that
active access; a caller must finish using it before advancing the same cursor again. A window
selected directly by `get` retains the supplied input provenance instead. Failed exact takes leave
the cursor unchanged. Delimiter takes exclude one matched delimiter and consume it; when no
delimiter remains, they return the complete suffix and finish the cursor.

`TextCursor` measures positions in UTF-8 bytes. Exact takes accept only scalar-aligned endpoints.
Text separator matching uses exact non-empty UTF-8 text. An empty separator fails with
`std.scan.empty_separator`; it never creates a non-progressing cursor operation.

Fixed-width numeric advances delegate byte interpretation to [`std/bytes`](../bytes/README.md).
Insufficient input returns `none` without advancing. A successful advance consumes exactly the
encoded width; the cursor does not retain a second byte-order or scalar-decoding implementation.
