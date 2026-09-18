# HTTP Client Boundary

This document owns the current cross-responsibility URL, resolver, HTTP protocol, transport, and
response-ownership boundary. Exact declarations and observable behavior remain in the checked
[`std/url`](../../../std/url/README.md), [`std/net`](../../../std/net/README.md), and
[`std/http`](../../../std/http/README.md) contracts. Asynchronous computation semantics belong to
the [asynchronous computation boundary](asynchronous-computation.md).

## Authority Layers

The client stack has four independent authorities:

1. `std/url` owns RFC 3986 parsing, canonical URL values, and request-target projection.
2. `std/net` owns host validation, system resolution, logical address candidates, and connection
   progress.
3. the private HTTP codec owns HTTP/1.1 syntax, framing, and protocol limits.
4. the private exchange layer owns request normalization, informational-response handling, and
   final-response selection.

Synchronous and asynchronous adapters drive those shared authorities through different progress
mechanisms. Neither adapter may parse URL text, resolve framing, normalize headers, or select a
response independently.

## Trusted Target Services

System name resolution is an operating-system service, not a raw syscall or a custom DNS client.
The compiler therefore exposes a finite private target-service mechanism rather than public FFI:

- target adapters declare package-private primitive callables;
- the selected toolchain profile maps exact semantic declarations to closed service descriptors;
- target closure validates each callable type once;
- Machine consumes the selected service plan without source paths or declaration spellings;
- native selection owns calling convention, imports, symbols, and loader records.

The Darwin resolver adapter alone knows native resolver records and releases their linked storage.
It copies usable IPv4 and IPv6 results into logical `SocketAddress` values before returning. Public
source never observes native pointers, family constants, or resolver error codes.

## URL Values

An absolute URL stores parsed components rather than source offsets or caller storage. Parsing,
component projection, relative resolution, request-target generation, equality, hashing, and
formatting consume that one representation. HTTP never reparses the original text.

The URL layer recognizes HTTP-family schemes, validates authority and port syntax, rejects user
information for HTTP client use, and owns percent-escape and dot-segment normalization. Host
resolution accepts only the validated host projection. Fragments never enter a request target.

## Shared Protocol Authority

Header names and values are validated values. Headers remain ordered so duplicate fields are not
silently lost or combined. One transport-neutral HTTP/1.1 codec owns request lines, status lines,
header syntax, chunk syntax, trailers, and configured protocol limits.

The exchange layer owns:

- unsupported scheme and method rejection;
- reserved request-field policy;
- generation of `Host`, `Connection`, and `Content-Length`;
- request-head encoding through the codec;
- ordinary informational-response consumption;
- rejection of status 101 while no upgraded-connection owner exists; and
- conversion of the selected final head into the canonical body cursor.

The exchange layer is a state transition over owned HTTP values and received bytes. It does not
open sockets, resolve names, wait for readiness, apply transport timeouts, or close descriptors.
Adapters supply bytes and consume only explicit incomplete, ready, or failed exchange results.

For each response, the codec chooses exactly one framing mode before body bytes are exposed:

1. method or status forbids a body;
2. a valid final transfer coding selects chunked framing;
3. one unambiguous content length selects fixed framing;
4. otherwise connection close delimits the body.

No transport adapter may reinterpret transfer coding, content length, or completion.

## Progress and Transport Ownership

`Client` owns request policy. A request owns its method, URL, headers, and bounded byte body. The
synchronous operation drives blocking resolver and stream contracts. The asynchronous operation
returns `future Response!` and performs validation, encoding, provider-backed resolution,
connection, request transmission, and final-head reception only while driven.

One private transport sum owns either plain TCP or authenticated TLS. It is not a public extension
interface. Both variants satisfy the same progress, timeout, cancellation, read, and close
contract before entering response storage. HTTP values do not expose the chosen transport.

Candidate connection order comes from `std/net`. Every failed partial candidate releases its owner
before the next begins. The asynchronous path never invokes a synchronous resolver or blocking
stream wrapper. Neither path shells out to another process or depends on a public Internet service.

## Response Ownership

One `Response` uniquely owns one transport and one canonical body cursor. The cursor owns framing
progress, pending input, and the selected decoder. Synchronous and asynchronous reads advance that
same state and cannot act as independent views.

Completion, explicit close, decoding failure, network failure, or destruction releases the
transport exactly once. Destroying an unstarted request future releases its captured request and
client borrow. Destroying one after connection begins cancels its owned connector or transport
through the ordinary asynchronous lifecycle.

A body-read future borrows the response. Cancelling it releases the borrow while leaving the
response as the unique owner at its already advanced cursor position. Transport state is never
rewound.

## Deadlines and Cancellation

Timeout-bearing operations use one fixed relative duration for connection progress and individual
I/O waits. They do not silently reinterpret it as a whole-response deadline. HTTP delegates waits
to the network or secure-transport layer and does not implement another timer or readiness race.

Every suspended operation owns either a connector or a response transport, never both after an
ownership transfer. Cancellation removes readiness registration before destroying that owner. The
executor contains no HTTP parser state, and HTTP state contains no scheduler identity.

## Derived Operations

Named request constructors, validated field mutation, text bodies, and whole-body collection are
derived from public values and cursor operations. They do not access transport-private records or
duplicate framing logic. Whole-body reads repeatedly drive the canonical response read operation;
UTF-8 conversion occurs only after byte collection.

## Error Boundary

Each layer contributes stable errors in its own namespace:

- URL syntax and unsupported URL features use `std.url.*`;
- resolution and connection use `std.net.*`;
- authentication and secure transport use `std.tls.*`;
- HTTP syntax, limits, framing, and policy use `std.http.*`.

Wrapping may add context but retains the originating stable code. Native resolver values, target
errors, parser cursors, descriptors, and codec state never become public payloads.

## Responsibility Matrix

| Decision | Sole authority | Consumers |
|---|---|---|
| URL grammar and canonical components | `std/url` | applications, HTTP policy |
| Host validation and candidate order | `std/net` resolver | client orchestration |
| Native resolver records and cleanup | selected target adapter | resolver policy |
| HTTP syntax, framing, and limits | private HTTP codec | exchange and body cursor |
| Request policy and final-head selection | private exchange layer | sync and async adapters |
| Descriptor progress and deadlines | network and TLS transports | client adapters |
| Response connection and cursor ownership | private `Response` representation | body operations |
| Public declarations and behavior | `std/**/index.nct` and assigned guides | compiler, editor, applications |

## Required Invariants

- URL source text is parsed once into one authoritative value.
- Host resolution produces logical values and releases every native result exactly once.
- HTTP framing is selected once before public body reads.
- Synchronous and asynchronous clients share protocol policy and response storage.
- Each connector, transport, response, and body cursor has one owner and one cleanup path.
- Limits are checked before unbounded allocation.
- Later compiler stages consume closed target-service identities without source-name lookup.
- Tests use deterministic local fixtures rather than public network services.
