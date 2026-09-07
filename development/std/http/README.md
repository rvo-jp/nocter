# HTTP/1.1 Messages and Framing

`std/http` owns validated HTTP/1.1 message values and the protocol's single transport-independent
framing authority. It does not resolve names, open sockets, own descriptors, or apply client
connection policy.

`Method` preserves the exact case-sensitive token. `HeaderName` accepts the HTTP token alphabet,
stores one lowercase canonical spelling, and hashes that canonical identity. `HeaderValue` stores exact bytes after removing wire
optional whitespace at its edges; CR, LF, NUL, and other forbidden controls are rejected. `Headers`
is ordered and preserves duplicates instead of silently combining fields.

`RequestHead` accepts only non-empty ASCII origin-form targets beginning with `/`; percent escapes
must be complete hexadecimal triplets and characters outside the path/query grammar are rejected. The private wire
encoder adds exactly one `Content-Length` selected from the bounded body and rejects caller fields
that could introduce a second framing interpretation.

The response codec accepts only HTTP/1.1 status lines and strict CRLF field lines. It rejects obsolete
line folding. It selects body framing once: HEAD, informational, 204, and 304 responses are bodyless;
a supported final `Transfer-Encoding` selects chunked framing; one unambiguous `Content-Length`
selects fixed framing; otherwise connection closure delimits the body. Equal comma-separated or
repeated content lengths are accepted as one value. Conflicting lengths, simultaneous transfer
encoding and content length, repeated chunked codings, unsupported codings, invalid decimal or
hexadecimal sizes, and overflow are stable errors.

Chunk extensions follow the token and quoted-string grammar even though their values are ignored.
Trailer fields are syntax-checked and bounded but are not merged into the response head;
`Content-Length` and `Transfer-Encoding` are forbidden in trailers so framing cannot be revised
after body bytes have been exposed.

Body decoding writes into caller-provided storage. Partial start lines, fields, chunk-size lines,
chunk data, delimiters, and trailers retain an explicit incomplete state; they are never treated as
end of input. Fixed and chunked bodies report premature EOF. Close-delimited bodies complete only
when the transport reports EOF. The selected framing and its limit set enter one private body
decoder together, so a transport consumer cannot reinterpret either decision.

`Limits` bounds the start line, aggregate head, field count, individual field or trailer line,
chunk-size line, and decoded body. The codec checks these bounds before protocol-controlled growth.
The standard defaults permit an 8 KiB start line, 64 KiB aggregate head, 128 fields, 8 KiB field
lines, 1 KiB chunk lines, and a 64 MiB decoded body.

The checked declarations in [the module contract](index.nct) are the API authority.
