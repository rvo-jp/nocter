# Asynchronous HTTP Client Boundary

This document defines the shared protocol and ownership boundary for the asynchronous HTTP client
introduced in v0.42.0. Exact public declarations remain owned by `development/std/http/index.nct`,
observable behavior by `development/std/http/README.md`, and async computation semantics by the
[Asynchronous Computation Boundary](asynchronous-computation-design.md).

## Outcome

The synchronous and asynchronous HTTP clients use one request policy, one canonical body cursor,
and one response owner. They differ only in how transport progress is driven. Neither path may
create another implementation of header policy, response selection, body framing, limits, or
connection cleanup.

One private transport sum owns either plain TCP or authenticated TLS. It is not a public extension
interface: its purpose is to make both transports satisfy the same response ownership, progress,
timeout, cancellation, and close boundary without exposing that choice in HTTP values.

## Shared Protocol Authority

The private HTTP exchange layer is a pure state transition over owned HTTP values and received
bytes. It owns:

- rejection of unsupported schemes and methods;
- reserved request-field policy and generation of `Host`, `Connection`, and `Content-Length`;
- request-head encoding through the existing codec;
- consumption of ordinary informational responses;
- rejection of status 101 while no upgraded-connection owner exists; and
- conversion of the final parsed head into the unique canonical body cursor.

The exchange layer does not open sockets, resolve names, wait for readiness, apply I/O timeouts, or
close descriptors. Synchronous and asynchronous orchestration supply bytes to it and consume only
its explicit `incomplete`, `ready`, or `failed` result. The codec remains the sole authority for
HTTP syntax and body framing.

## Deferred Work

The asynchronous request operation is an `async` method whose invocation returns
`future Response!`. Request validation, encoding, provider-backed host resolution, connection,
complete request transmission, and final-response-head reception all occur while that computation
is driven. The asynchronous path never invokes the synchronous system resolver.

The operation therefore has one recoverable failure layer:

```nct
let pending = client.send(move request)
var response = await move pending?
```

Destruction of an unstarted computation releases its captured request and client borrow.
Destruction after connection has begun cancels its owned connection attempt or stream through the
ordinary async lifecycle.

## Response Ownership

One `Response` uniquely owns one connection and one canonical body cursor. That cursor owns the
selected decoder and pending received range; its decoder state is the sole completion authority.
Synchronous and asynchronous body reads advance that same state, so they are not independent views
and cannot be used concurrently. Completion, explicit close, decoding failure, network failure, or
destruction releases the connection exactly once.

The public response type does not expose whether its private connection is plain or secure. Both
variants supply the same read, cancellation, and close ownership facts before entering the response
representation.

## Deadlines and Cancellation

The timeout-bearing synchronous and asynchronous APIs apply a duration to connection and
individual I/O progress. A whole-request deadline is a different contract and must not be inferred
from those names. `send_with_timeout` delegates its connection deadline and each write or
response-head idle wait to async TCP. `read_with_timeout` supplies the same explicit idle
timeout for response-body transport input. HTTP does not implement a second timer or readiness
race.

Every suspended operation owns either a candidate connector or the response stream, never both
after a transfer. Cancellation removes readiness registrations before destroying an owned
connector or stream. A body-read computation borrows the response instead: cancelling it releases
the borrow while the response remains the unique stream owner. HTTP state contains no scheduler
identity, and the executor contains no HTTP parser state.

## Derived Convenience Layer

Practical request and response operations depend only on the public value and cursor contracts.
Named GET, HEAD, and POST constructors call the sole general request constructor. Textual field
mutation constructs validated `HeaderName` and `HeaderValue` values before appending anything, so
failure cannot partially mutate the request. Text-body mutation copies bytes into the same owned
body accepted by the general byte operation; it does not infer media type or character encoding.

Whole-body asynchronous reads repeatedly call the public asynchronous response-read operations.
They do not access the body cursor, pending-input offsets, decoder, or stream. The
timeout-bearing collector passes the same duration to each read and therefore preserves the
established idle-timeout contract. UTF-8 collection is a final conversion of the owned byte result,
not a second transport or framing path.

The collector owns each consumed prefix rather than lending it back to the response. Cancellation
after progress drops that prefix and leaves the response at its one advanced cursor position;
neither cancellation nor UTF-8 failure can rewind transport state.

## Responsibility Matrix

| Decision | Sole authority | Consumers |
|---|---|---|
| URL syntax and canonical components | `std/url` | HTTP request policy |
| Request policy and final-head selection | private HTTP exchange layer | sync and async orchestration |
| HTTP syntax, framing, and limits | private HTTP codec and canonical body cursor | exchange layer, response reads |
| Host validation and candidate order | `std/net` resolver | async request construction |
| Descriptor progress and operation timeout | `std/net` async TCP | async orchestration |
| Computation lifecycle and cancellation | async runtime contract | HTTP computation owner |
| Response connection and cursor ownership | private `Response` representation | sync and async body reads |
| Derived request and body collection conveniences | request and response contract adapters | applications |
| Public declarations | `development/std/http/index.nct` | compiler, editor, applications |

## Rejected Shortcuts

- Do not duplicate the synchronous client and replace each socket call with `await`.
- Do not resolve host names while polling an async computation.
- Do not expose the private plain/TLS transport sum as a public extension interface.
- Do not let an async adapter parse status lines, select body framing, or count informational
  responses.
- Do not create separate sync and async response owners over one stream.
- Do not silently reinterpret an operation timeout as a whole-response deadline.
