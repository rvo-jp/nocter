# HTTP/1.1 Client

`std/http` owns validated HTTP/1.1 message values, the protocol's single transport-independent
framing authority, and synchronous and asynchronous one-request-per-connection operations. The
client composes the public URL, name-resolution, TCP, authenticated TLS, and I/O contracts; the
codec remains independent of sockets, descriptors, DNS, executor state, and connection policy.

`Request` owns a parsed `Url`, method, ordered user fields, and complete byte body. `Client` adds a
canonical `Host`, `Connection: close`, and one computed `Content-Length`. Callers cannot supply
those fields or `Transfer-Encoding`, so request framing has one authority. `http` selects a plain
provider stream and `https` selects a TLS stream authenticated for the URL host against the
operating-system trust store. CONNECT is rejected because the API does not transfer tunnel
ownership.

`Request.get`, `Request.head`, and `Request.post` are named constructors over the same validated
`Request.new` operation. `append_header_text` validates both textual components before mutating the
request, and `set_text_body` copies the exact UTF-8 bytes. These conveniences do not infer a
`Content-Type`, select an encoding, or bypass the reserved-field policy applied by `Client`.

`Client.send` opens one plain or authenticated connection and returns a uniquely owned `Response`.
One private transport sum owns that choice; request encoding, final-head parsing, body framing, and
the public response cursor operate on the sum rather than branching on the URL scheme. Ordinary
informational responses are consumed before the final response is exposed; protocol-switching
status 101 is rejected because the API does not transfer the upgraded stream. `Response` exposes
the final status and fields and implements `Reader` for decoded body bytes. Completion, decoding or
network failure, explicit `close`, and destruction of an unfinished response all close the
connection. There is no pooling, redirect following, request replay, decompression, or connection
reuse.

`Client.send_async` has an immediate outer result and a lazy inner computation. Request validation,
request-head encoding, and synchronous host resolution finish before the method returns. Awaiting
the inner computation connects, writes the complete request, and receives the final response head
without blocking the executor thread. Dropping that computation cancels the connection or stream
it uniquely owns through the ordinary async lifecycle.

`Response.read_async` uses the selected asynchronous transport while advancing the same decoder,
pending bytes, completion flag, and uniquely owned stream used by `read`. Synchronous and
asynchronous reads cannot form independent cursors or concurrently consume one response. Their
transport loops share one body progress operation, so EOF and decoding decisions are not
reimplemented by either adapter.

`read_to_end_async` repeatedly consumes that same asynchronous cursor into owned bytes, while
`read_to_string_async` additionally validates the completed bytes as UTF-8. Their timeout-bearing
forms delegate every needed read to `read_async_with_timeout`; the duration therefore remains a
per-input idle timeout rather than becoming a whole-body deadline. Collection remains bounded by
the `Limits` selected by the client and introduces no second body decoder.

Whole-body collection consumes the response cursor as it progresses. Destroying a collector after
one or more completed chunk reads discards its owned prefix and does not rewind the response.
Transport, framing, timeout, or final UTF-8 validation failure returns no partial collection;
UTF-8 failure occurs after the complete body has been consumed.

`send_async_with_timeout` applies its `Duration` to the existing shared connection deadline, each
complete request-head or request-body write, and each idle wait for more response-head input. Host
resolution remains synchronous and consumes time from the connection deadline; retaining the lazy
computation before awaiting it does not restart that deadline. Receiving head bytes begins a new
idle interval. The timeout is not a deadline for the complete response.

`read_async_with_timeout` bounds the idle interval before the next transport input needed by one
body-read call. Buffered decoded bytes return immediately. Each received framing or body fragment
begins another idle interval when further input is required. A timeout is a terminal network
failure for that response and closes its stream, matching synchronous response-read failure.

Destroying an unstarted send computation starts no connection. Cancelling a suspended send
computation removes its readiness interests before destroying the connector or stream it owns.
Destroying a body-read computation instead releases its exclusive borrow and leaves the `Response`
owner intact; a later read may continue from the same decoder state. HTTP stores neither task IDs
nor timer registrations.

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
