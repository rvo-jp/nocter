# Asynchronous HTTP Client Boundary

This document defines the shared protocol and ownership boundary for the asynchronous HTTP client
planned for v0.42.0. Exact public declarations remain owned by `development/std/http/index.nct`,
observable behavior by `development/std/http/README.md`, and async computation semantics by the
[Asynchronous Computation Boundary](asynchronous-computation-design.md).

## Outcome

The synchronous and asynchronous HTTP clients use one request policy, one incremental response
decoder, and one response owner. They differ only in how transport progress is driven. Adding an
asynchronous API must not create a second implementation of header policy, response selection,
body framing, limits, or connection cleanup.

TLS is a later transport capability. This phase does not introduce a public transport interface or
a private wrapper around the sole existing `TcpStream` implementation. Such an abstraction would
have no independent contract to enforce. When secure transport exists, it may replace the private
connection field behind the same response ownership contract without changing public HTTP values.

## Shared Protocol Authority

The private HTTP exchange layer is a pure state transition over owned HTTP values and received
bytes. It owns:

- rejection of unsupported schemes and methods;
- reserved request-field policy and generation of `Host`, `Connection`, and `Content-Length`;
- request-head encoding through the existing codec;
- consumption of ordinary informational responses;
- rejection of status 101 while no upgraded-connection owner exists; and
- conversion of the final parsed head into the unique body-decoder state.

The exchange layer does not open sockets, resolve names, wait for readiness, apply I/O timeouts, or
close descriptors. Synchronous and asynchronous orchestration supply bytes to it and consume only
its explicit `incomplete`, `ready`, or `failed` result. The codec remains the sole authority for
HTTP syntax and body framing.

## Immediate and Deferred Work

The asynchronous request operation will return `(async Response!)!`, not `async Response!`.
Request validation and encoding happen before the outer result succeeds. Host resolution also
remains immediate because the current system resolver is synchronous and must never run on the
single-threaded executor. The returned lazy computation owns the resolved candidates and performs
connection, complete request transmission, and final-response-head reception through async TCP.

This split is observable and intentional:

```nct
let pending = client.send_async(move request)?
var response = await move pending?
```

Failure before `pending` exists cannot leave a computation or descriptor. Destruction of an
unstarted or suspended computation cancels its owned connection attempt or stream through the
ordinary async lifecycle.

## Response Ownership

One `Response` uniquely owns one connection, one selected body decoder, pending received bytes, and
completion state. Synchronous and asynchronous body reads advance that same state; they are not
independent views and cannot be used concurrently. Completion, explicit close, decoding failure,
network failure, or destruction releases the connection exactly once.

The public response type does not expose whether its private connection is plain or secure. A
future TLS implementation must supply the same read, cancellation, and close ownership facts before
the private representation is generalized.

## Deadlines and Cancellation

The existing timeout-bearing synchronous API applies a duration to connection and individual I/O
operations. A whole-request deadline is a different contract and must not be inferred from that
name. The asynchronous surface will keep per-operation timeout behavior distinct from any later
absolute request deadline.

Every suspended operation owns either a candidate connector or the response stream, never both
after a transfer. Cancellation removes readiness registrations before closing the descriptor.
HTTP state contains no scheduler identity, and the executor contains no HTTP parser state.

## Responsibility Matrix

| Decision | Sole authority | Consumers |
|---|---|---|
| URL syntax and canonical components | `std/url` | HTTP request policy |
| Request policy and final-head selection | private HTTP exchange layer | sync and async orchestration |
| HTTP syntax, framing, and limits | private HTTP codec and decoder | exchange layer, response reads |
| Host validation and candidate order | `std/net` resolver | async request construction |
| Descriptor progress and operation timeout | `std/net` async TCP | async orchestration |
| Computation lifecycle and cancellation | async runtime contract | HTTP computation owner |
| Response connection and decoder ownership | private `Response` representation | sync and async body reads |
| Public declarations | `development/std/http/index.nct` | compiler, editor, applications |

## Rejected Shortcuts

- Do not duplicate the synchronous client and replace each socket call with `await`.
- Do not resolve host names while polling an async computation.
- Do not introduce a public `Transport` interface before two implementations share a proven
  contract.
- Do not let an async adapter parse status lines, select body framing, or count informational
  responses.
- Do not create separate sync and async response owners over one stream.
- Do not silently reinterpret an operation timeout as a whole-response deadline.

