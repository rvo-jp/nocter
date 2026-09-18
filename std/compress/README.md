# Streaming Compression

The compiler-checked [`compress` module contract](index.nct) is the sole authority for exact public
signatures. This guide records the progress and state guarantees that span those declarations.

DEFLATE reads bits least-significant-bit first inside each byte. One decoder owns the bounded bit
reservoir, canonical Huffman state, and 32 KiB history required between calls. It never retains a
borrow of a caller's input or output slice. Consequently the same decoder can be driven by a file,
socket, fixed buffer, blocking adapter, or asynchronous adapter without changing representation
semantics. Stored, fixed-Huffman, and dynamic-Huffman blocks all pass through this same state and
history authority.

Every bounded operation reports exact input consumption and output production through
`InflateStep`. `needs_input` means all reported input was consumed before another semantic step
could complete. `needs_output` means the supplied output capacity was filled before the stream
completed. Callers retain any unconsumed input and observe only the initialized output prefix.

`finished` and `invalid` are terminal. Calling a terminal decoder again consumes zero bytes, writes
zero bytes, and reproduces the same terminal classification. Malformed compressed data is an
`InflateFailure`, not a recoverable `error`: invalid input is an ordinary format outcome and every
variant must retain the progress made before detection. I/O, allocation, and application-policy
failure remain recoverable errors in the layers that own those operations.

Raw DEFLATE provides no integrity protection. Gzip composition validates its trailer through the
single [`std/checksum`](../checksum/README.md) CRC-32 authority. Compression does not own archive
entry structure, external transport, deadlines, filesystem paths, extraction limits, or file
publication.
