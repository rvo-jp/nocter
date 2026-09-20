# HTTP/1.1

`std/http` owns validated HTTP/1.1 message values, the protocol's single transport-independent
framing authority, synchronous and asynchronous client operations, and an asynchronous server
lifecycle. The client composes URL, name-resolution, TCP, authenticated TLS, and I/O contracts. The
server composes TCP acceptance with the same field and body codecs. Codec state remains independent
of sockets, descriptors, DNS, executor state, and connection policy.

## Server Lifecycle

`Server.bind` and `Server.bind_with_limits` create an asynchronous listener over one numeric socket
address. `Server.accept` borrows the listener exclusively and returns a uniquely owned
`ServerConnection`. Acceptance does not start a detached task, parse bytes, or transfer listener
ownership. Destroying or explicitly closing the server closes only the listening socket; accepted
connections retain their independent ownership.

`ServerConnection.read_request(self)` consumes the accepted state. Success returns one
`IncomingRequest` that owns its validated method, origin-form target, ordered fields, body cursor,
TCP stream, peer identity, and immutable limits. No responder exists while that body is readable.
`IncomingRequest` implements `Reader` and `TimedReader`; each read writes decoded bytes directly
into caller storage instead of retaining a complete-body allocation. The consuming `finish_body`
operation drains any unread body through the same cursor and returns the only `Responder` for that
request. It has no caller-visible "already complete" precondition. This transition makes a response
beside an unfinished body and two responses on one connection unrepresentable. Failure or
cancellation of a consuming transition destroys the owner and closes its stream.

The request-head decoder accepts strict HTTP/1.1 request lines and CRLF fields. It requires exactly
one non-empty `Host`, rejects conflicting framing evidence, supports fixed and chunked request
bodies, and treats the absence of `Content-Length` and `Transfer-Encoding` as an empty body. Method
and target bytes are validated and copied once when the complete head is known. Incremental scan
offsets, retained fields, and framing evidence remain owned by one decoder across transport reads;
accepted bytes are not rescanned. Bytes received beyond the current body remain in the cursor's
pending range and transfer to the responder. They are neither discarded nor interpreted while the
current response is unfinished. Once response framing completes, the unread suffix transfers
inside a returned `ServerConnection`. Before the next head starts, it is compacted into the other
reusable transport buffer; the prior request is neither retained nor rescanned, and repeated
partial pipelining cannot grow a connection buffer without bound.

`ResponseHead` is the shared validated status-and-fields value for received and authored metadata.
`Responder.begin_fixed` or `begin_chunked` consumes that head, validates reserved fields and limits,
writes one frozen head, and returns the unique `ResponseWriter`. The writer implements `Writer` and
`TimedWriter`. Fixed output rejects a fragment beyond its remaining length and rejects premature
finish. Chunked output owns hexadecimal chunk spelling and emits exactly one terminal chunk at
finish. Empty writes emit no chunk. Both paths await complete transport writes and retain only one
32-byte protocol scratch allocation, independent of response size.

Callers cannot provide `Connection`, `Content-Length`, or `Transfer-Encoding`; the frozen response
plan is the sole framing authority. Status 204, 205, and 304 reject a non-empty or chunked body; 204
and 304 omit `Content-Length`. A response to HEAD accepts and counts caller body fragments but does
not transmit them. Transport failure or cancellation leaves the writer terminal and closes its
stream no later than owner destruction. A fragment rejected before transport progress leaves the
writer available for a corrected write.

`OutgoingResponse` owns a `ResponseHead` plus a complete body. `Responder.respond` derives a
fixed-length plan, writes that body through `ResponseWriter`, and finishes the same transition. It
does not encode a second response head or implement another body-output path. Successful finish
returns `ServerConnection?`: a value transfers the exact reusable stream and retained input;
`none` reports that the exchange completed with connection closure. `Responder.close_after_response`
forces a terminal response plan without exposing protocol-controlled `Connection` fields.

HTTP/1.1 connections are reusable by default. Validated comma-separated `Connection` values are
processed once while parsing the request, and any case-insensitive `close` token selects terminal
behavior. Unknown valid connection options do not create capabilities. Reuse remains strictly
sequential: bytes for a later pipelined request may already be buffered, but no later head is parsed
and no handler begins until the prior response has completed and returned the connection. The
server deliberately has no connection pool, concurrent pipelined execution, upgrade, CONNECT
tunnel, or implicit task spawning. Request and response bodies stream through bounded transfer
storage; complete owned response bodies remain a convenience rather than a separate protocol path.

## Server Deadlines and Capacity

Timeout-bearing acceptance, request-head decoding, body finalization, response begin, response
write, response finish, and complete-response operations compose their ordinary transitions with
`std/task`; the HTTP codec and TCP transport do not implement another clock. Each named operation
uses one fixed monotonic deadline. `ResponseWriter.write_with_timeout` covers the complete logical
fragment, including chunk prefix, bytes, and suffix. Expiry reports `std.http.timed_out`.

`IncomingRequest.read_with_timeout` instead applies one idle timeout when that read needs more
transport input. Buffered decoded bytes return immediately. A successful fragment does not attach
a deadline to later reads; applications select whether each later read, a generic timed collector,
or consuming whole-body finalization provides the appropriate timeout scope.

An expired accept cancels only its temporary borrowing computation and leaves `Server` available
for another accept. Request-head decoding consumes `ServerConnection`, so expiry destroys the
stream instead of returning a partial head. A cancelled borrowed body read releases its exclusive
borrow and leaves the request cursor at its exact progressed state. Expired consuming finalization
destroys the request. Expired response begin or consuming finish destroys its connection owner; an
expired borrowed writer operation marks the writer terminal and closes the transport. None exposes
a partially reusable stream. Transport and codec failures retain their original stable code while
adding operation context where appropriate.

Concurrent connection capacity belongs to application task ownership, not hidden listener state.
An application caps accepted work by checking `TaskGroup.len()` and awaiting `TaskGroup.next()`
before accepting when its selected limit is reached. This bounds accepted streams and child tasks
together. Request transfer storage is fixed per active cursor rather than proportional to accepted
body length; `Limits` still bounds decoded bytes and syntax-controlled storage. Kernel backlog
policy remains the TCP listener's responsibility. The
[bounded HTTP service example](../../examples/http-service/index.nct) demonstrates this complete
composition. It transfers a 32 KiB request and chunked response through 1 KiB application buffers,
reuses one connection for a retained pipelined request only after the first response completes,
forces terminal response policy, and rejects malformed framing plus an idle request under finite
deadlines. Its two-slot handler group remains the sole connection-capacity authority.

Graceful shutdown is the same application-owned composition. The application first consumes
`Server.close`, which stops listener admission without touching accepted connection owners. It then
moves its complete handler `TaskGroup` into one drain future and bounds that future with a single
`task.with_timeout` deadline. Completion exposes every handler result; expiry cancels the owned
drain future, whose ordinary destruction cancels all remaining children and closes whichever
linear HTTP typestate each child owns. HTTP keeps no parallel task registry or connection list.

The deadline covers the whole drain rather than restarting for each handler. A process signal is
only one possible application trigger and is not required by the HTTP lifecycle contract.

## Deterministic Routing

`Router` stores validated method-and-path registrations in insertion order while route selection
uses a construction-time precedence rule rather than that order. A pattern begins with `/`; each
slash-delimited segment is either exact text or one named parameter such as `:user_id`. Parameter
names use ASCII identifier spelling and may occur only once in a pattern. Patterns do not contain
queries. Exact segments compare the retained percent-encoded path spelling; selected parameter
values are percent-decoded and UTF-8-validated once into an owned `RouteMatch`.

The number of exact segments defines precedence. Registration rejects two routes for the same
method when they can match the same path at equal precedence. This makes every successful
selection independent of insertion order while still allowing `/users/me` to take precedence over
`/users/:id`. A path accepted by another method yields `RouteDispatch.method_not_allowed`; a path
accepted by no pattern yields `RouteDispatch.not_found`. Both variants return the still-owned
`IncomingRequest`, so application policy—not the router—decides how to finish or close it.

`Router<State>` owns exactly the application state supplied to `Router.new`. Every selected
`Handler<State>` receives a readonly borrow of that same value; registration and dispatch never
clone it or obtain state from a global. Applications choose the state contract explicitly. An
immutable configuration may be stored directly, while shared mutation can be represented by a
public synchronization value such as `Mutex<T>`. `state` exposes the retained value by borrow and
`into_state` recovers it when the router lifecycle ends.

`Handler<State>` is one `any &func` contract. The router can retain heterogeneous closures and
invoke the selected handler repeatedly through readonly erasure. Calling a handler creates a
`future`; that future borrows the router state and owns its request and route match until it is
awaited or cancelled. `Router.dispatch` awaits the computation directly, so the state borrow ends
before dispatch returns. It creates no task, clone, executor, responder, timeout, or connection
registry, and it does not reinterpret HTTP framing or persistence.

## Application Response Policy

`OutgoingResponse.text` and `OutgoingResponse.json` construct complete UTF-8 bodies over the same
owned response value used by `Responder.respond`. They add exactly one canonical `Content-Type`;
the JSON constructor deliberately does not parse, normalize, or certify the supplied document.
Named `Status` constructors cover the ordinary 200, 400, 404, 405, and 500 application outcomes
without creating a second status representation.

`IncomingRequest.respond` is a composition boundary, not a second server transition. It drains the
body with `finish_body` and passes the resulting unique `Responder` to `respond`.
`respond_with_timeout` applies the supplied duration independently to those two already-defined
consuming transitions; it does not silently widen the duration into an end-to-end deadline.

Applications handle `RouteDispatch` explicitly. A `handled` value contains the selected handler's
connection disposition. The `not_found` and `method_not_allowed` values retain the request so the
application can select a body, headers, timeout, and keep-alive policy before calling `respond`.
Once a handler owns a request, a failure cannot recover that request merely to synthesize a 500:
the connection closes through ordinary linear destruction. Handlers that want an error response
must catch application failures while they still own the appropriate request or responder state.
This prevents a router-level error hook from guessing whether response bytes were already sent.

## Application Data

`ApplicationLimits` is the shared finite policy for form pairs, media-type parameters, and request
cookies. It bounds retained item count, each decoded name or value, and total decoded bytes. Its
standard policy permits 128 items, 8 KiB per value, and 64 KiB in aggregate. Each `parse` operation
selects that policy; `parse_with_limits` is the explicit policy variant. A parser never sizes
temporary decoded storage from an untrusted source component beyond its selected per-value limit.
Syntax failures and limit failures have distinct stable codes.

`Form.parse` implements strict `application/x-www-form-urlencoded` decoding. It splits on `&`,
uses the first `=` in each nonempty item, turns `+` into a space, decodes only complete hexadecimal
percent triplets, and rejects decoded bytes that are not UTF-8. Empty split items are ignored; an
item without `=` has an empty value. Pairs and duplicate names remain in source order. `first`
selects the first exact decoded name and `count` exposes multiplicity, so the parser does not hide
a first-wins, last-wins, or map-combining policy.

URI query decoding remains deliberately distinct: `RequestTarget.query_pairs` preserves `+` as a
literal plus because URI query syntax does not imply form encoding. Both parsers use the same
private percent-triplet decoder for bounds and hexadecimal conversion; form decoding applies its
plus-to-space rule before invoking that narrower authority.

`MediaType.parse` accepts one token type, `/`, one token subtype, and zero or more semicolon-delimited
parameters. It stores the type, subtype, and parameter names in lowercase, preserves parameter
values, supports token and quoted-string values, and rejects duplicate parameter names
case-insensitively. `matches` and `parameter` compare ASCII-insensitively without reparsing the
source.

`Cookies.parse` accepts the request `Cookie` field's semicolon-separated cookie pairs, trims only
HTTP optional whitespace around each pair, validates token names and the cookie-octet value
alphabet, and accepts the standard quoted value wrapper without treating it as a general escape
language. It preserves pair and duplicate order. `Cookie.new` validates a single explicit pair;
`Cookie.render` emits the canonical unquoted `name=value` spelling, which is safe because stored
values already satisfy the unquoted cookie-octet grammar.

`SessionCookie` is the explicit boundary between opaque `std/session.SessionId` values and HTTP
fields. Its policy fixes one validated cookie name and absolute path. Set values are canonical
URL-safe unpadded Base64 and always carry `HttpOnly` and `SameSite=Lax`; `Secure` is selected at
policy construction. Clear values repeat the same path and security policy and add `Max-Age=0`.
`read_headers` rejects multiple `Cookie` fields rather than applying an implicit combination rule,
then parses the sole field through the ordinary bounded cookie authority. `read` accepts an already
parsed `Cookies` value when the application owns a separate field-combination policy. Both reject
duplicate session names and malformed identifier spellings.

## Client Lifecycle

`Request` owns a parsed `Url`, method, ordered user fields, and complete byte body. `Client` adds a
canonical `Host`, `Connection: close`, and one computed `Content-Length`. Callers cannot supply
those fields or `Transfer-Encoding`, so request framing has one authority. `http` selects a plain
provider stream and `https` selects a TLS stream authenticated for the URL host against the
operating-system trust store. HTTPS advertises only the `http/1.1` ALPN protocol and rejects a
connection unless the server negotiates that exact value. CONNECT is rejected because the API
does not transfer tunnel ownership.

`Client.new().with_trust_anchor(move anchor)` returns a client policy that adds one owned DER root
to the operating-system trust store for HTTPS. It does not replace system roots or disable hostname
authentication. Calling the method again replaces the client's previous additional root. Plain
HTTP ignores this TLS policy. The synchronous connection copies the root before returning from the
TLS constructor. An asynchronous send captures the client borrow, then copies the root while its
computation is driven; the borrow checker keeps the client alive until that computation is awaited
or destroyed.

`Request.get`, `Request.head`, and `Request.post` are named constructors over the same validated
`Request.new` operation. `append_header_text` validates both textual components before mutating the
request, and `set_text_body` copies the exact UTF-8 bytes. These conveniences do not infer a
`Content-Type`, select an encoding, or bypass the reserved-field policy applied by `Client`.

`Client.send_blocking` opens one plain or authenticated connection and returns a uniquely owned
`Response`. One private transport sum owns that choice; request encoding, final-head parsing, body
framing, and the public response cursor operate on the sum rather than branching on the URL scheme. Ordinary
informational responses are consumed before the final response is exposed; protocol-switching
status 101 is rejected because the API does not transfer the upgraded stream. `Response` exposes
the final status and fields and implements both `Reader` and `BlockingReader` for decoded body
bytes. Completion,
decoding or network failure, explicit `close`, and destruction of an unfinished response all transfer the
connection through its exact-once nonwaiting disposal boundary. There is no pooling, redirect
following, request replay, decompression, or connection reuse. `send_blocking` and
`send_with_timeout_blocking` are explicitly `blocking`; their synchronous transport setup uses the
synchronous system resolver.

`Client.send` returns one lazy computation with one `Response!` outcome. Request validation,
request-head encoding, owned host-endpoint construction, provider resolution, connection, complete
request transmission, and final-response-head reception all occur while that computation is driven
without blocking the executor thread. Dropping it releases the captured request before setup or
cancels the connection or stream it uniquely owns after setup begins.

`Response.read` uses the selected asynchronous transport while advancing the same canonical body
cursor and uniquely owned stream used by `read_blocking`. The decoder state is the only completion
authority; there is no mirrored Boolean. Synchronous and asynchronous reads cannot form independent
cursors or concurrently consume one response. Their transport loops share one body progress
operation, so EOF and decoding decisions are not reimplemented by either adapter.

The generic `Reader` defaults implement `read_to_end` and `read_to_string` once for every
asynchronous byte source. `Response` also implements `TimedReader`. Its timeout-bearing collection
methods create a borrowing `TimeoutReader` and reuse the same generic collector; no HTTP-specific
collection loop remains. The duration therefore remains a per-input idle timeout rather than
becoming a whole-body deadline. Collection remains bounded by
the `Limits` selected by the client and introduces no second body decoder.

Whole-body collection consumes the response cursor as it progresses. Destroying a collector after
one or more completed chunk reads discards its owned prefix and does not rewind the response.
Transport, framing, timeout, or final UTF-8 validation failure returns no partial collection;
UTF-8 failure occurs after the complete body has been consumed.

`send_with_timeout` applies its `Duration` to the existing shared connection deadline, each
complete request-head or request-body write, and each idle wait for more response-head input. The
connection deadline starts when the lazy computation begins and covers provider resolution and
connection; retaining the computation before awaiting it does not consume or restart that
deadline. Receiving head bytes begins a new idle interval. The timeout is not a deadline for the
complete response.

`read_with_timeout` bounds the idle interval before the next transport input needed by one
body-read call. Buffered decoded bytes return immediately. Each received framing or body fragment
begins another idle interval when further input is required. A timeout is a terminal network
failure for that response and closes its stream, matching synchronous response-read failure.

Destroying an unstarted send computation starts no connection. Cancelling a suspended send
computation removes its readiness interests before destroying the connector or stream it owns.
Destroying a body-read computation instead releases its exclusive borrow and leaves the `Response`
owner intact; a later read may continue from the same decoder state. HTTP stores neither task IDs
nor timer registrations.

`send_with_timeout_blocking` applies one duration to the host-connection deadline and then as the
timeout of each stream read and write operation. It is not a wall-clock deadline for the complete
response.

`Method` preserves the exact case-sensitive token. `HeaderName` accepts the HTTP token alphabet,
stores one lowercase canonical spelling, and hashes that canonical identity. `HeaderValue` stores
exact bytes after removing wire optional whitespace at its edges; CR, LF, NUL, and other forbidden
controls are rejected. `Headers` is ordered and preserves duplicates instead of silently combining
fields.

`RequestHead` accepts only non-empty ASCII origin-form targets beginning with `/`; percent escapes
must be complete hexadecimal triplets and characters outside the path/query grammar are rejected.
`RequestTarget.parse` also validates percent-decoded query bytes incrementally without allocating
decoded storage. It rejects invalid UTF-8 before construction, so subsequent query iteration is
infallible.
The resulting `RequestTarget` retains that exact spelling and records the first path/query boundary
plus every slash-delimited path segment once. `path`, `query`, `path_segment_count`, and
`path_segment` project borrowed ranges from that retained structure; none scans the request again.
`query_pairs` splits a present non-empty query on `&`, splits each item at its first `=`, and yields
one lending `QueryParameter` at a time. Names and values without percent escapes are borrowed
directly from the retained target. A percent-encoded component uses decoder-owned scratch text, so
the item must stop being live before the decoder advances and may reuse that storage. A missing `=`
means an empty value. Percent triplets decode to bytes, `+` remains `+` rather than becoming a
space. Invalid decoded UTF-8 is reported as `std.http.invalid_target_encoding` by construction,
before a `QueryIter` can exist. An absent or explicitly empty query yields no pairs. Empty items
inside a non-empty query remain observable rather than being silently discarded.
`QueryParameter.to_owned` is the explicit boundary for a
pair that must outlive the next decoder advance; plain iteration never copies unescaped component
text.

The private wire encoder adds exactly one `Content-Length` selected from the bounded body and
rejects caller fields that could introduce a second framing interpretation.

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
