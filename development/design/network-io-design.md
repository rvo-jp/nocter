# Network I/O Boundary

This document defines the current cross-responsibility contract for network I/O. It began as the
v0.39.0 synchronous design and includes the later asynchronous and provider-backed TCP migration;
implemented availability remains defined by the exact
checked declarations in `development/std/net/index.nct` and the behavior guide assigned by
`development/std/README.md`.

## Purpose

Network I/O crosses value semantics, provider and descriptor ownership, callback progress,
blocking policy, target-specific ABI layout, stable errors, and native services. Those decisions
must not be rediscovered independently by TCP, UDP, a target adapter, and the compiler.

The foundation therefore has five authorities:

1. `std/net` owns public address, stream, listener, datagram, timeout, and error behavior.
2. a package-private stream policy owns TCP connection/listener state, ordered provider-event
   reduction, synchronous/asynchronous progress, and terminal cleanup;
3. a package-private datagram policy owns the UDP descriptor and readiness engine;
4. `std/internal/os` owns target-independent raw operating-system error classifications;
5. the selected target adapter owns provider objects, callback ABIs, syscalls, native constants,
   and the byte layout of native records.

Closed runtime roles and target contracts expose only the fixed operations required by these
policies. CheckedProgram, MIR, and general Machine lowering do not interpret sockets, address
families, ports, provider objects, or target record structures.

## Scope

The current layer provides numeric IPv4 and IPv6 addresses, system host resolution, synchronous
and asynchronous TCP client/listener operations, synchronous and asynchronous UDP datagrams, and
monotonic deadlines. URL and HTTP build above this layer. TLS composes with the same provider owner
in its own design boundary. A public nonblocking socket mode remains excluded.

All release qualification uses loopback communication. Normal compilation and tests must not
depend on an external network service.

## Address Values

Public addresses are target-independent values:

- `Ipv4Address` stores exactly four octets.
- `Ipv6Address` stores exactly sixteen octets.
- `IpAddress` distinguishes IPv4 from IPv6 without inspecting byte patterns.
- `SocketAddress` combines an IP address with one logical `u16` port.

No public address stores a native `sockaddr`, native family number, native byte order, pointer, or
scope identifier. A port remains a logical host value until a target adapter writes a native
socket-address record. The adapter performs the sole host/network byte-order conversion.

Address parsing is allocation-free and consumes the complete input. Invalid numeric text is
optional absence, not a recoverable operating-system error. Formatting has one canonical generator
per address family and is shared by direct text conversion and `Format`.

Canonical IPv4 text contains four decimal components, no redundant leading zeroes, and no omitted
components. Canonical IPv6 text uses lowercase hexadecimal, suppresses leading zeroes within a
component, and compresses the first longest run of at least two zero components. A socket address
uses `address:port` for IPv4 and `[address]:port` for IPv6. Host names, service names, and IPv6 zone
identifiers are outside this milestone.

Equality and hashing use the address family and exact address bytes; socket addresses additionally
use the port. Any public ordering contract compares the same logical values and never native
structure padding.

## Transport Ownership

Every public network wrapper is move-only and owns exactly one private transport state. TCP streams
and listeners own an opaque provider record; UDP sockets own one descriptor. A successful
constructor transfers that owner into exactly one wrapper. Moving the wrapper transfers ownership;
copying is forbidden.

Explicit close is terminal and idempotent. A TCP close requests provider cancellation, consumes the
typed terminal state, crosses the serial callback-queue barrier, and only then releases the provider,
queue, channel, and callback resources. Destruction performs the same sequence for an open owner.
Operations on a closed wrapper return one stable closed-socket error.

UDP destruction closes an open descriptor at most once and never retries a failed close: descriptor
reuse makes such a retry unsafe. Every partial constructor records ownership immediately and
releases each acquired resource on failure. No provider object, callback pointer, event descriptor,
or socket descriptor escapes into a public value, and no operation infers ownership from an integer
sentinel.

UDP descriptors are close-on-exec. Provider-backed TCP writes cannot terminate the process through
`SIGPIPE`. The target adapter supplies both guarantees without exposing their native mechanisms.

IPv6 listeners are IPv6-only in v0.39.0. Code requiring both families creates one listener per
family. This avoids making platform-dependent IPv4-mapped address defaults part of the public
contract.

## TCP Semantics

`TcpStream` is a byte stream and conforms to the canonical asynchronous `Reader` and `Writer`
contracts as well as the explicit `BlockingReader` and `BlockingWriter` contracts.

- A read returns zero only after clean peer EOF or for an empty destination buffer.
- A successful write consumes the complete supplied buffer.
- A provider send copies the supplied view into provider-owned storage before the source borrow
  ends; completion is observed through the connection's ordered event channel.
- Local shutdown and terminal close have distinct states. Shutdown prevents the selected direction
  without releasing the provider owner; close releases ownership after the cancellation fence.

`TcpListener.accept` returns a newly owned `TcpStream` and the peer `SocketAddress`. Binding port
zero is supported, and a listener can report its effective local address. Tests and applications
therefore do not need to predict an unused port.

Connection establishment, acceptance, reads, and writes all use the same provider owner, ordered
event reduction, and deadline policy in synchronous and asynchronous calls. A protocol wrapper does
not implement its own callback, retry, or elapsed-time loop.

## UDP Semantics

`UdpSocket` does not conform to `BlockingReader` or `BlockingWriter`. Those contracts describe a
byte stream and cannot preserve datagram boundaries or distinguish an empty datagram from EOF.

One send operation transmits one complete datagram or returns an error. One receive operation
returns the source address, copied byte count, and whether the datagram exceeded the destination
buffer. A zero-length datagram is a successful receive with a zero copied count. Truncation is
reported explicitly and never masquerades as a complete message.

Connected UDP may constrain the peer used by later sends and receives, but it does not acquire TCP
stream semantics. Unconnected and connected operations share the same datagram result and deadline
rules. Bind, numeric peer selection, and address observation are immediate operations. Canonical
send and receive methods are asynchronous; `_blocking` twins retain explicit synchronous access.

## Blocking and Deadlines

Potentially waiting public operations distinguish asynchronous canonical names from explicit
`_blocking` twins. Neither surface exposes a descriptor mode. Immediate setup and observation carry
neither execution modifier.

Internally, every potentially waiting operation uses an optional absolute monotonic deadline.
Relative public durations are converted once at the operation boundary. Provider events, readiness
interruptions, partial datagram progress, and ordered connection candidates reuse that deadline;
none restart the requested timeout.

The package-private policies are the sole authorities for:

- reducing target events and descriptor results into logical progress or terminal failure;
- waiting on the provider event channel or UDP descriptor;
- recomputing remaining time from the monotonic clock; and
- maintaining configured synchronous timeout state without exposing platform socket options.

An absent deadline means wait without a time limit. Configured socket timeouts apply only to
synchronous twins; asynchronous timeout variants take one explicit relative duration. A zero
duration still permits one immediate attempt. Expiration returns one stable timeout error.
Wall-clock adjustments cannot extend or shorten a network deadline.

Allocation and blocking are independent properties. Socket operations do not gain or lose a
`noalloc` guarantee merely because they may wait. Exact callable modifiers follow the actual public
implementation, including error construction, rather than an assumed syscall property.

## Error Boundary

Public failure remains the built-in `error` carried by `T!`. `std/net` owns stable network error
codes and messages. Callers never receive a raw `errno`, Darwin constant, native address family, or
polling event mask.

`std/internal/os` expands its target-independent classification only when callers can usefully
distinguish a condition, including connection refusal or reset, not-connected use, address conflict,
unreachable network, timeout, and unsupported operation. Target adapters map raw codes to that
closed internal classification. `std/net` maps the classification and operation context to stable
public codes exactly once.

Invalid numeric address text remains optional absence. Invalid socket configuration, unavailable
addresses, connection failures, I/O failures, and timeouts are recoverable errors. Resource
exhaustion during infallible error construction follows the existing built-in error termination
contract.

## Target and ABI Boundary

The target adapter exposes closed operations over logical inputs, opaque owners, and explicitly
sized mutable byte regions. It alone owns:

- provider imports, object retain/release, fixed Block signatures, and callback record layout;
- provider connection/listener construction, dispatch queues, and terminal callback barriers;
- syscall numbers and invocation arity for the UDP descriptor substrate;
- native address-family, socket-type, protocol, shutdown, option, and event constants;
- native socket-address length, alignment, discriminant placement, port byte order, and address
  byte placement;
- native polling-record layout and readiness-mask decoding;
- target raw-error decoding; and
- target mechanisms for close-on-exec and suppressed broken-pipe signals.

The package-private network policies request only complete adapter operations. They cannot write
offsets into an opaque native record, compare provider states or target constants, retain native
objects, or construct callbacks. The adapter cannot decide public timeout, candidate ordering,
datagram truncation, or stable error-code policy.

Closed primitive roles and runtime-storage roles are the only compiler/runtime boundary. Target
closure validates their exact signatures and ownership shape; higher compiler representations
transport already resolved calls and opaque storage without reconstructing network meaning.

## Responsibility Matrix

| Decision | Sole authority | Consumers |
|---|---|---|
| Public types, methods, and stable errors | `std/net` contract | applications, compiler checking, editor presentation |
| Address text grammar and canonical generation | `std/net` address implementation | parsing, formatting, `Format` |
| TCP provider state, event reduction, and cleanup | private stream policy | TCP wrappers |
| UDP descriptor state, readiness, and cleanup | private datagram policy | UDP wrapper |
| Absolute deadline policy | private network substrate | connect, accept, stream I/O, datagram I/O |
| Target-independent raw error kind | `std/internal/os` | public I/O policy modules |
| Provider ABI, native socket, callback, and polling layouts | selected target adapter | private network policies |
| Source meaning and callable guarantees | compiler semantic pipeline | CLI, LSP, executable lowering |
| Generic syscall execution | runtime primitive roles and target lowering | selected target adapter |

No consumer may reconstruct another row's decision from source text, integer constants, layout
offsets, or a later representation.

## Delivery Invariants

Each implementation phase must preserve these invariants:

- one public declaration authority and one implementation of each observable rule;
- one opaque provider owner per TCP value and one descriptor owner per UDP value;
- one address parser and one canonical generator per address family;
- one target conversion between logical addresses and native records;
- one monotonic deadline representation consumed by both transport policies;
- one raw-error classification followed by one public error mapping;
- no external-network dependency in ordinary tests;
- no compiler or editor-only network meaning; and
- no compatibility form for rejected or superseded API spellings.

## Non-goals

- DNS and service-name resolution
- URL, HTTP, WebSocket, or TLS protocol policy
- readiness streams or a public nonblocking mode
- multicast, broadcast, ancillary data, interface enumeration, or IPv6 zones
- Unix-domain or raw sockets
- platform-native socket structures in public APIs
- another native target
