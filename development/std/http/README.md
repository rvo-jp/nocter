# Synchronous HTTP/1.1

`std/http` owns validated HTTP/1.1 message values, the protocol's single transport-independent
framing authority, and a synchronous one-request-per-connection client. The client composes the
public URL, name-resolution, TCP, and I/O contracts; the codec remains independent of sockets,
descriptors, DNS, and connection policy.

`Request` owns a parsed `Url`, method, ordered user fields, and complete byte body. `Client` adds a
canonical `Host`, `Connection: close`, and one computed `Content-Length`. Callers cannot supply
those fields or `Transfer-Encoding`, so request framing has one authority. HTTPS is rejected before
name resolution because this release has no TLS transport. CONNECT is rejected because the API
does not transfer tunnel ownership.

`Client.send` opens one connection and returns a uniquely owned `Response`. Ordinary informational
responses are consumed before the final response is exposed; protocol-switching status 101 is
rejected because the API does not transfer the upgraded stream. `Response` exposes the final status
and fields and implements `Reader` for decoded body bytes. Completion, decoding or network failure,
explicit `close`, and destruction of an unfinished response all close the connection. There is no
pooling, redirect following, request replay, decompression, or connection reuse.

`send_with_timeout` applies one duration to the host-connection deadline and then as the timeout of
each stream read and write operation. It is not a wall-clock deadline for the complete response.

`Method` preserves the exact case-sensitive token. `HeaderName` accepts the HTTP token alphabet,
stores one lowercase canonical spelling, and hashes that canonical identity. `HeaderValue` stores exact bytes after removing wire
optional whitespace at its edges; CR, LF, NUL, and other forbidden controls are rejected. `Headers`
is ordered and preserves duplicates instead of silently combining fields.

`RequestHead` accepts only non-empty ASCII origin-form targets beginning with `/`; percent escapes
must be complete hexadecimal triplets and characters outside the path/query grammar are rejected. The private wire
encoder adds exactly one `Content-Length` selected from the bounded body and rejects caller fields
that could introduce a second framing interpretation.

The response-head decoder accepts only HTTP/1.1 status lines and strict CRLF field lines. It owns
its scan cursor, completed fields, framing evidence, and informational-response count across
transport reads. Bytes and fields already accepted are therefore not rescanned or reconstructed
when the next fragment arrives. It rejects obsolete line folding. It selects body framing once:
HEAD, informational, 204, and 304 responses are bodyless; a supported final `Transfer-Encoding`
selects chunked framing; one unambiguous `Content-Length` selects fixed framing; otherwise
connection closure delimits the body. Equal comma-separated or repeated content lengths are
accepted as one value. Conflicting lengths, simultaneous transfer encoding and content length,
repeated chunked codings, unsupported codings, invalid decimal or hexadecimal sizes, and overflow
are stable errors.

Chunk extensions follow the token and quoted-string grammar even though their values are ignored.
Trailer fields are syntax-checked and bounded but are not merged into the response head;
`Content-Length` and `Transfer-Encoding` are forbidden in trailers so framing cannot be revised
after body bytes have been exposed.

Body decoding writes into caller-provided storage. Partial start lines, fields, chunk-size lines,
chunk data, delimiters, and trailers retain an explicit incomplete state; they are never treated as
end of input. Fixed and chunked bodies report premature EOF. Close-delimited bodies complete only
when the transport reports EOF. The selected framing and its limit set enter one private body
decoder together, so a transport consumer cannot reinterpret either decision.

`Limits` bounds the start line, each aggregate head, field count, informational-response count,
individual field or trailer line, chunk-size line, and decoded body. The decoder checks these bounds
before protocol-controlled growth. Bounding both each head and the number of informational heads
also bounds their cumulative retained input and field storage. The standard defaults permit an 8
KiB start line, 64 KiB per head, 128 fields, 16 informational responses, 8 KiB field lines, 1 KiB
chunk lines, and a 64 MiB decoded body.

The checked declarations in [the module contract](index.nct) are the API authority.
