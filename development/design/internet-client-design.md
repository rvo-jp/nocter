# Internet Client Foundation

This document defines the cross-responsibility URL, resolver, and HTTP foundation introduced by the
v0.40.0 synchronous Internet client. Exact implemented declarations remain owned by the checked
standard-library `index.nct` files, and observable behavior remains owned by their assigned guides.
The asynchronous transport extension is defined separately by the
[Asynchronous HTTP Client Boundary](asynchronous-http-client-design.md).

## Outcome

An ordinary Nocter program can parse an absolute HTTP URL, resolve its host through the operating
system, connect over the existing synchronous TCP foundation, send a bounded HTTP/1.1 request, and
consume a correctly framed response body. The complete path is deterministic under a local test
fixture and does not require an external command, compiler plugin, bundled runtime, or editor-only
interpretation.

TLS, HTTPS transport, public nonblocking sockets, HTTP/2, HTTP/3, WebSocket, proxying, cookies,
automatic content decoding, and a public HTTP server remain outside this foundation. Asynchronous
I/O was outside v0.40.0 and now composes the same values and protocol authorities through the
separate boundary linked above.

## Existing Boundary

The [network I/O boundary](network-io-design.md) remains authoritative for logical numeric
addresses, descriptor ownership, native socket records, retry policy, and monotonic deadlines.
This design adds three independent consumers above it:

1. `std/url` owns RFC 3986 parsing and canonical URL values;
2. `std/net` owns host resolution and address-candidate connection policy;
3. `std/http` owns HTTP message syntax, framing, limits, and client behavior.

Neither URL nor HTTP may inspect a descriptor, native address, resolver record, or syscall result.
Name resolution may produce only existing logical `SocketAddress` values. HTTP may perform
transport only through `TcpStream`, `BlockingReader`, and `BlockingWriter` contracts.

## Trusted Target Services

Darwin name resolution is an operating-system service, not a raw syscall. Reimplementing DNS over
UDP would bypass the host's resolver order, hosts database, multicast resolution, and per-domain or
VPN policy. Running an external resolver command would add an unstable process and text protocol.
The compiler therefore gains one private mechanism for calling a finite catalog of trusted target
services already supplied by the selected operating system.

This mechanism is not public FFI and introduces no user-facing declaration form:

- standard-library target adapters declare source-private `primitive func` callables;
- the selected toolchain profile is the sole authority mapping each exact declaration identity to
  a target-service descriptor;
- target closure validates the exact callable type once and produces a closed service-call plan;
- Machine lowering consumes that plan without looking up a source path, declaration spelling, or
  standard-library implementation;
- the native selector applies the already chosen foreign calling convention;
- the executable serializer owns dylib imports, symbol spellings, stubs, and loader binding data.

A target-service binding connects one stable compiler role and exact semantic callable to a closed
descriptor. The descriptor contains the target identity, library identity, external symbol,
validated calling convention, and argument/result ABI classes. No later stage may reconstruct one
of these fields from another. Source-controlled strings cannot select a symbol or library.

The initial foreign-call subset admits only fixed-arity C integers, machine words, and pointers,
one scalar result or no result, the platform C calling convention, and ordinary caller-clobber
effects. It does not admit variadic calls, callbacks, aggregate values by value, foreign ownership,
or user declarations. Future services must extend this contract deliberately instead of treating
an arbitrary C signature as compatible.

`getaddrinfo` and `freeaddrinfo` are the first consumers. Their native records and linked-list
layout belong exclusively to the Darwin resolver adapter. The generic import mechanism must not
contain host-name, address-family, resolver, or HTTP vocabulary.

## Host Resolution

The public resolver accepts an ASCII host spelling and a logical `u16` port. A numeric host uses
the existing numeric parser and requires no system lookup. A name delegates to the system resolver
with service-name lookup disabled, then copies every usable IPv4 or IPv6 result into logical
`SocketAddress` values before releasing all native results.

The resolver:

- preserves the system's usable result order;
- removes exact duplicate logical addresses without sorting by family;
- never exposes canonical-name pointers, native address records, `EAI_*` values, or resolver-owned
  memory;
- rejects embedded NUL and unsupported non-ASCII host spelling before the target boundary;
- releases the complete native result list exactly once on every post-success path; and
- maps native resolver outcomes to one stable `std.net.*` error exactly once.

The ordinary operation allocates result storage in the current allocation context. A recoverable
variant accepts `TryAllocator`; native resolver failure and recoverable Nocter allocation failure
remain distinguishable through stable error codes.

Host connection consumes one resolved sequence in order. It uses one optional monotonic deadline
for the whole operation: resolution and failed address attempts cannot each restart the caller's
duration. Each failed partial connection releases its descriptor before the next candidate. Empty
results and complete candidate exhaustion have specified stable outcomes.
`TcpStream.connect_blocking` for one numeric address remains the primitive synchronous public
operation and is not reimplemented.

## URL Values

`std/url` owns an immutable, parsed, absolute hierarchical URL. v0.40.0 follows RFC 3986 rather than
browser-specific WHATWG state-machine behavior. The representation retains components, not parser
cursors or offsets into caller storage.

The initial accepted surface is deliberately exact:

- `http` and `https` schemes are recognized as URL values, while the v0.40.0 client transports only
  `http`;
- authority is required and user information is rejected for HTTP client use;
- hosts are numeric IPv4, bracketed IPv6, or ASCII registered names;
- non-ASCII registered names require an explicit future IDNA contract and are rejected now;
- a port is an optional decimal `u16` with scheme defaults applied by projection;
- path, query, and fragment obey RFC 3986 component character and percent-escape rules;
- invalid or incomplete percent escapes are rejected; and
- fragments never enter an HTTP request target.

Canonical generation lowercases the scheme and ASCII registered-name host, preserves logical IPv6
address meaning through the existing address formatter, uses uppercase hexadecimal percent escapes,
removes dot segments, and omits a port only when it equals the recognized scheme default. Parsing,
component projection, relative-reference resolution, request-target generation, equality, hashing,
and formatting consume this single parsed representation. HTTP cannot parse the original text a
second time.

## HTTP Message and Framing Authority

`std/http` separates value syntax from transport policy. Header names are validated ASCII tokens
with case-insensitive equality and hashing. Header values are validated byte sequences and cannot
contain CR, LF, NUL, or forbidden controls. Headers remain an ordered sequence so duplicate fields
are neither lost in a map nor silently combined.

One private HTTP/1.1 codec owns request-line, status-line, header, chunk, and trailer syntax. It is
transport-neutral and can be reused by a future server. A private exchange layer owns request
normalization and final response-head selection for every client orchestration. The synchronous
and asynchronous client adapters own DNS, connection, timeout, and connection-lifecycle progress;
the codec and exchange layer own none of those transport operations.

For each response, the codec selects exactly one body framing before body bytes are exposed:

1. the request method or response status forbids a body;
2. a valid final transfer coding selects chunked framing;
3. one unambiguous content length selects fixed framing;
4. otherwise the response is delimited by connection close.

Conflicting content lengths, unsupported transfer codings, invalid field syntax, premature EOF,
overflow, and framing ambiguity are errors. `Transfer-Encoding` and `Content-Length` cannot be
interpreted independently by different layers. Configured start-line, aggregate-header, field,
chunk-line, and buffered-body limits are checked before unbounded allocation.

## Client Ownership

`Client` owns immutable request policy and produces a uniquely owned `Response`. `Response` exposes
status and headers and implements `BlockingReader` for the decoded response body. The reader
consumes fixed, chunked, or close-delimited framing without exposing framing bytes. Closing or destroying an
unfinished response releases its TCP stream.

Requests own their method, URL, headers, and bounded byte body. v0.40.0 does not type-erase an
arbitrary request-body reader and does not replay a body implicitly. Automatic redirects and
connection pooling are excluded because they require replay and cross-response ownership policies
that the initial API cannot express honestly. Each request owns one connection and sends
`Connection: close`; a later pool may be added behind a separate ownership contract.

The client rejects HTTPS before resolution or connection. It adds the required Host field from the
parsed URL, validates user fields through the same header types, computes one request body length,
and writes through the existing complete-write contract. Partial request transmission remains
observable to the peer but never yields a successful response.

The timeout-bearing operation shares one deadline across connection candidates. After connection,
the same duration bounds each individual stream read and write; it is not a deadline for the whole
response lifetime. A future whole-operation deadline must be a distinct contract rather than an
undocumented reinterpretation of this API.

## Error Boundary

Each layer contributes stable errors in its own namespace:

- URL syntax and unsupported URL features use `std.url.*`;
- name resolution and candidate connection use `std.net.*`;
- HTTP syntax, limits, framing, and policy use `std.http.*`.

Wrapping may add context to the built-in error but must retain the originating stable code. A
native resolver value, errno, parser cursor, TCP descriptor, or internal codec state never becomes
the public error payload. Allocation exhaustion follows the existing ordinary or `TryAllocator`
contract rather than being relabeled as a protocol failure.

## Responsibility Matrix

| Decision | Sole authority | Consumers |
|---|---|---|
| Trusted service identity and foreign ABI contract | selected toolchain target-service catalog | target closure, Machine |
| Native import encoding and symbol binding | executable serializer | loader |
| Darwin resolver records and native result cleanup | Darwin resolver adapter | private resolver policy |
| Host validation, logical results, and candidate order | `std/net` resolver | applications, HTTP client |
| URL grammar and canonical components | `std/url` | applications, HTTP client |
| HTTP syntax and body framing | private transport-neutral HTTP codec | client exchange and response progression, future server |
| Request policy and final-head selection | private HTTP exchange layer | client orchestration |
| Connection lifecycle progress | synchronous and asynchronous HTTP adapters | applications |
| Descriptor and monotonic deadline policy | existing private network substrate | resolver connection policy, HTTP client |
| Public declarations and documentation | module `index.nct` and assigned guide | compiler, editor, applications |

## Delivery Invariants

- no custom DNS protocol may masquerade as system name resolution;
- no network-specific operation enters CheckedProgram, MIR, or generic Machine semantics;
- no source spelling or module path is rediscovered after target-service closure;
- every native resolver list and partial socket has one explicit owner and one cleanup path;
- URL text is parsed once into one authoritative value;
- HTTP framing is selected once before public body reads;
- limits are applied before unbounded allocation;
- tests use deterministic local resolver and HTTP fixtures rather than public Internet services;
- CLI and LSP consume the same checked declarations as ordinary source; and
- rejected compatibility spellings and duplicate convenience implementations are not retained.
