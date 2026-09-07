# Network I/O Boundary

This document defines the cross-responsibility contract for synchronous network I/O. It is the
adopted implementation design for v0.39.0; implemented availability remains defined by the exact
checked declarations in `development/std/net/index.nct` and the behavior guide assigned by
`development/std/README.md`.

## Purpose

Network I/O crosses value semantics, descriptor ownership, blocking policy, target-specific ABI
layout, stable errors, and native syscalls. Those decisions must not be rediscovered independently
by TCP, UDP, a target adapter, and the compiler.

The foundation therefore has four authorities:

1. `std/net` owns public address, stream, listener, datagram, timeout, and error behavior.
2. a package-private network substrate owns descriptor state and one deadline-driven synchronous
   operation policy shared by TCP and UDP;
3. `std/internal/os` owns target-independent raw operating-system error classifications;
4. the selected target adapter owns syscall numbers, socket constants, and the byte layout of
   native socket-address and polling records.

The compiler supplies ordinary primitive-call and pointer capabilities. It does not know about
sockets, address families, ports, or target socket structures.

## Scope

v0.39.0 provides numeric IPv4 and IPv6 addresses, TCP client and listener operations, UDP
datagrams, and synchronous deadlines. It deliberately excludes name resolution, URL parsing,
HTTP, TLS, asynchronous readiness, and a public nonblocking socket mode.

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

## Descriptor Ownership

Every public socket wrapper owns one descriptor state with two terminal categories: open and
closed. A successful constructor transfers exactly one open descriptor into exactly one wrapper.
Moving the wrapper transfers that ownership. Copying is forbidden.

Explicit close is terminal even when the operating system reports failure. Destruction closes an
open descriptor at most once, ignores close failure, and never retries a failed close: after an
interrupted close, descriptor reuse makes a retry unsafe. Operations on a closed wrapper return one
stable closed-socket error.

Every partial constructor records descriptor ownership immediately. A later failure closes every
owned descriptor before returning. No raw descriptor may escape into a public value and no public
operation may infer ownership from an integer sentinel.

Descriptors are close-on-exec. Stream writes cannot terminate the process through `SIGPIPE`; the
target adapter supplies the target-specific socket policy needed to uphold that guarantee.

IPv6 listeners are IPv6-only in v0.39.0. Code requiring both families creates one listener per
family. This avoids making platform-dependent IPv4-mapped address defaults part of the public
contract.

## TCP Semantics

`TcpStream` is a byte stream and conforms to `Reader` and `Writer`.

- A read returns zero only after clean peer EOF or for an empty destination buffer.
- A successful write consumes the complete supplied buffer.
- A lower-level partial write is retried by the shared stream operation; if a later error occurs,
  the error is returned after the already-written prefix remains observable.
- Interrupted operations retry while their deadline still permits progress.
- Local shutdown and terminal close have distinct states. Shutdown prevents the selected direction
  without releasing descriptor ownership; close releases ownership.

`TcpListener.accept` returns a newly owned `TcpStream` and the peer `SocketAddress`. Binding port
zero is supported, and a listener can report its effective local address. Tests and applications
therefore do not need to predict an unused port.

Connection establishment, acceptance, reads, and writes all use the same synchronous readiness and
deadline substrate. A protocol wrapper does not implement its own retry or elapsed-time loop.

## UDP Semantics

`UdpSocket` does not conform to `Reader` or `Writer`. Those contracts describe a byte stream and
cannot preserve datagram boundaries or distinguish an empty datagram from EOF.

One send operation transmits one complete datagram or returns an error. One receive operation
returns the source address, copied byte count, and whether the datagram exceeded the destination
buffer. A zero-length datagram is a successful receive with a zero copied count. Truncation is
reported explicitly and never masquerades as a complete message.

Connected UDP may constrain the peer used by later sends and receives, but it does not acquire TCP
stream semantics. Unconnected and connected operations share the same datagram result and deadline
rules.

## Blocking and Deadlines

The public API is synchronous and may block. v0.39.0 therefore does not introduce `noblock`.

Internally, every potentially waiting operation is expressed as one state transition plus an
optional absolute monotonic deadline. Relative public durations are converted to an absolute
deadline once at the operation boundary. Retries after interruption or partial progress reuse that
deadline; they do not restart the requested timeout.

The shared substrate is the sole authority for:

- placing a descriptor in the internal mode required by deadline-aware operation;
- classifying immediate success, progress, readiness wait, timeout, interruption, and terminal
  failure;
- polling the target adapter;
- recomputing remaining time from the monotonic clock; and
- restoring no public platform-specific timeout state.

An absent deadline means wait without a time limit. A zero duration performs the operation only if
it can make immediate progress. Expiration returns one stable timeout error. Wall-clock adjustments
cannot extend or shorten a network deadline.

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

The target adapter exposes operations over logical inputs and explicitly sized mutable byte regions.
It alone owns:

- syscall numbers and invocation arity;
- native address-family, socket-type, protocol, shutdown, option, and event constants;
- native socket-address length, alignment, discriminant placement, port byte order, and address
  byte placement;
- native polling-record layout and readiness-mask decoding;
- target raw-error decoding; and
- target mechanisms for close-on-exec and suppressed broken-pipe signals.

The package-private network substrate may request an address record, socket operation, or readiness
wait only through that adapter contract. It cannot write offsets into an opaque native record or
compare target constants. The adapter cannot decide public timeout, retry, truncation, ownership,
or error-code policy.

Generic primitive roles remain the only compiler/runtime boundary. If a target operation fits the
existing primitive pointer and integer contract, adding it must not add socket vocabulary to
CheckedProgram, MIR, MachineProgram, or native lowering.

## Responsibility Matrix

| Decision | Sole authority | Consumers |
|---|---|---|
| Public types, methods, and stable errors | `std/net` contract | applications, compiler checking, editor presentation |
| Address text grammar and canonical generation | `std/net` address implementation | parsing, formatting, `Format` |
| Descriptor state and cleanup | private network substrate | TCP and UDP wrappers |
| Retry, readiness, and absolute deadline policy | private network substrate | connect, accept, stream I/O, datagram I/O |
| Target-independent raw error kind | `std/internal/os` | public I/O policy modules |
| Native socket and polling layouts | selected target adapter | private network substrate |
| Source meaning and callable guarantees | compiler semantic pipeline | CLI, LSP, executable lowering |
| Generic syscall execution | runtime primitive roles and target lowering | selected target adapter |

No consumer may reconstruct another row's decision from source text, integer constants, layout
offsets, or a later representation.

## Delivery Invariants

Each implementation phase must preserve these invariants:

- one public declaration authority and one implementation of each observable rule;
- one owned descriptor state per public socket value;
- one address parser and one canonical generator per address family;
- one target conversion between logical addresses and native records;
- one monotonic deadline and retry engine shared by TCP and UDP;
- one raw-error classification followed by one public error mapping;
- no external-network dependency in ordinary tests;
- no compiler or editor-only network meaning; and
- no compatibility form for rejected or superseded API spellings.

## Non-goals

- DNS and service-name resolution
- URL, HTTP, WebSocket, or TLS protocols
- asynchronous I/O, readiness streams, or a public nonblocking mode
- multicast, broadcast, ancillary data, interface enumeration, or IPv6 zones
- Unix-domain or raw sockets
- platform-native socket structures in public APIs
- another native target
