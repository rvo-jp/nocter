# Network I/O

The compiler-checked [`std/net` contract](index.nct) is the sole authority for exact public
declarations. This guide expands observable address, TCP, and UDP behavior already present in that
checked contract, including finite monotonic operation timeouts.

## Address Values

`Ipv4Address` contains four octets. `Ipv6Address` contains sixteen octets. `IpAddress` retains the
address family explicitly, so IPv4 and IPv6 values never become equal through an implementation
mapping. `SocketAddress` combines one address with a logical `u16` transport port. Every type is
copyable and contains no native socket structure, pointer, target family number, or target byte
order.

IPv4 parsing accepts exactly four decimal components from 0 through 255. Components have no sign,
cannot be empty, and cannot contain redundant leading zeroes. Canonical output is dotted decimal.

IPv6 parsing accepts eight hexadecimal components, one `::` compression of at least one component,
and an optional final dotted-decimal IPv4 address occupying the final two components. Hexadecimal
input is case-insensitive. Canonical output follows RFC 5952: lowercase hexadecimal, no redundant
component zeroes, and compression of the first longest run containing at least two zero components.
Canonical output represents an accepted embedded IPv4 tail as ordinary hexadecimal components.

`IpAddress.parse` selects IPv6 when the complete input contains `:` and otherwise selects IPv4.
`SocketAddress.parse` accepts `IPv4:port` and `[IPv6]:port`. Brackets are mandatory around IPv6 so
the port delimiter is unambiguous. Parsing consumes the complete input and returns absence for
invalid text; it does not allocate and does not report an operating-system error.

Equality compares the family and exact address bytes, plus the port for socket addresses. Ordering
places IPv4 before IPv6 and then compares bytes lexicographically; socket-address ordering compares
the logical address before the port. Hashing contributes the same equality-relevant values, never
native padding. Direct string conversion and the shared `Format` contract use the same canonical
generators.

## Host Resolution

`net.resolve` accepts one non-empty ASCII host without a NUL byte and returns owned logical
`SocketAddress` values using the current allocation context. `net.try_resolve` provides the same
operation with recoverable result storage from an explicit `TryAllocator`. A numeric IPv4 or IPv6
host bypasses the system resolver. Other hosts use the operating-system resolver with TCP-family
hints, retain its usable IPv4 and IPv6 order, apply the requested logical port, and remove only
exact duplicate addresses. Both operations are explicitly `blocking` because that system resolver
may synchronously wait for external progress.

Native resolver records never cross the target adapter. The adapter validates each native record,
copies only its logical address, and owns the complete result list until destruction releases it
exactly once. Empty or malformed results and native resolver failures become stable `std.net.*`
errors; native status codes and pointers are not observable.

`TcpStream.connect_host_blocking` resolves and tries candidates in returned order.
`TcpStream.connect_host_with_timeout_blocking` creates one monotonic deadline before resolution
and does not
restart it for each candidate. The platform resolver is a synchronous operating-system service and
cannot itself be interrupted by this timeout. Time spent resolving still consumes the deadline, so
connection work cannot receive a fresh duration afterward. As with other zero-duration network
operations, one immediate connection attempt is permitted and later candidates require remaining
time. Both host constructors carry the resolver's `blocking` effect.

`net.connect_host` and `net.connect_host_with_timeout` validate and copy NUL-terminated
host and decimal service text while their lazy computation is driven. Invalid input, provider
resolution, and connection failure all belong to the single awaited `TcpStream!` result. Driving
the future does not synchronously invoke the resolver or wait for socket readiness. The timeout
form starts its single deadline when the future begins and retains that deadline across validation,
provider resolution, and connection.

## TCP Streams and Listeners

`TcpStream` is a uniquely owned byte stream implementing `BlockingReader` and `BlockingWriter`.
Connecting accepts one numeric `SocketAddress`, while the host constructors compose the separate resolution contract
with ordered candidate connection. `net.connect_tcp` performs numeric connection without
blocking the executor thread. Reads initialize at most the supplied mutable byte view and return
zero at peer EOF. Writes complete the entire byte view or return a failure after any already-
written prefix remains observable. Empty transfers follow the ordinary stream contracts.

`TcpListener` binds one numeric address and accepts uniquely owned streams. Port zero asks the
kernel to select an available port; `local_address` reports the effective address. `accept` also
returns the connected peer address. IPv6 sockets are explicitly IPv6-only, so code serving both
families owns one listener for each family.

`TcpStream.read`, `TcpStream.write`, and `TcpListener.accept` use the same
provider-backed owner, ordered event channel, and error substrate as the synchronous operations.
`net.bind_tcp` also acquires its listener owner before suspending for provider readiness, so
cancellation cannot strand a partially initialized listener. The `_with_timeout` variants and
`net.connect_tcp_with_timeout` bound one complete operation with an explicit `Duration`.
They suspend on the provider event descriptor until ordered callback progress is available. A
direct `await` retains each receiver and buffer borrow in stable parent-computation storage. The
checked ownership model rejects moving the pending child computation beyond the lifetime of that
parent storage.

Native provider objects and callback descriptors are never exposed. A successful constructor,
connection, or acceptance transfers one closed provider owner into one move-only public value.
Explicit `close` is terminal and idempotent; destruction transfers a still-open owner through the
same exact-once disposal boundary. That transfer returns without waiting. A private cleanup worker
requests cancellation, drains ownership-bearing events, crosses the serial callback-queue barrier,
and then releases native state, so no callback can retain or access a released owner. `shutdown`
changes the selected stream direction without releasing ownership.

Synchronous TCP twins end in `_blocking`. `connect_with_timeout_blocking` bounds connection
establishment. `read_blocking`, `write_blocking`, and `accept_blocking` use the timeout configured
by `set_read_timeout`, `set_write_timeout`, or `set_accept_timeout`. Passing absence to a setter
restores unlimited waiting. The corresponding observation methods return the exact configured
`Duration?`.

These configured timeouts apply only to the synchronous operations. Asynchronous operations never
inherit mutable timeout configuration: their ordinary forms wait without a deadline, and their
`_with_timeout` forms accept one explicit relative timeout. A timed async wait publishes descriptor
readiness and its monotonic deadline in one runtime wait set. Waking either interest removes the
other registration before the operation retries, so no stale timer remains attached to the task.
The implementation never calls a blocking adapter on the executor thread.

TCP uses a closed provider adapter and UDP uses nonblocking datagram descriptors; neither substrate
creates a public nonblocking mode. Public failures use stable `std.net.*` codes rather than native
provider or errno values. The stable categories include closed sockets, timeout, connection refusal,
connection reset or abort, address conflict or
unavailability, unreachable networks, permission denial, broken pipes, oversized datagrams,
unsupported operations, and invalid target results.

## UDP Datagrams

`UdpSocket` is a uniquely owned datagram endpoint and deliberately does not implement
`BlockingReader` or `BlockingWriter`. Binding port zero and reporting the effective local address behaves like TCP listeners.
`send_to` supplies an address for one message. `connect` selects one peer for later `send`
operations and makes that peer available through `peer_address`; it does not turn UDP into a byte
stream. Binding, numeric peer selection, and address observation complete immediately without
publishing a future.

Every `send` or `send_to` asynchronously transmits one complete datagram or returns an error. Every
`receive` asynchronously consumes at most one datagram and returns one `DatagramRead` containing
its source address, the number of bytes copied, and an explicit truncation flag. Explicit
`_with_timeout` variants bound one async operation. The `send_blocking`, `send_to_blocking`, and
`receive_blocking` twins provide synchronous access. If a datagram exceeds the destination buffer,
the unread suffix is discarded and cannot appear in the next receive. A zero-length datagram is a
successful receive with copied length zero, not end of stream.

Destroying a pending async transfer cancels only its readiness wait. It does not close or duplicate
the borrowed socket, and a receive cancelled while waiting has not consumed a datagram. The same
socket may be used by a later operation after cancellation releases its exclusive borrow.

UDP read timeouts bound `receive_blocking`; UDP write timeouts bound `send_blocking` and
`send_to_blocking`. Async operations ignore this mutable configuration and use only an explicit
timeout argument when requested. `connect` is immediate and has no timeout. Timeout configuration
does not change datagram boundaries or expose native socket options.

## Timeout Semantics

Each finite relative timeout becomes one fixed monotonic deadline when an operation begins.
Interruption, readiness waits, and partial stream writes reuse that deadline; none restart the
relative duration. A zero timeout still performs one immediate operation attempt, so already-ready
data or capacity succeeds without waiting. If the operation would wait after its deadline, it
returns `std.net.timed_out`.

Timeouts measure the complete public operation rather than idle time between progress events.
Consequently a full-stream write may return `std.net.timed_out` after a prefix has reached the peer.
That prefix remains externally observable, as it does for another write failure. Wall-clock changes
cannot affect these deadlines.

## Current Boundary

Numeric addresses, explicit synchronous system host resolution, ordered synchronous host
connection, synchronous TCP, provider-resolved asynchronous host connection, asynchronous numeric
TCP connection and transfer, synchronous and asynchronous boundary-preserving UDP, and monotonic
operation timeouts are implemented. Async host resolution and connection are one provider operation; the standard
library does not materialize or retry a second address list on that path.
Native public-surface qualification covers peer EOF, idle-read timeout, and backpressure during a
timed complete write. Structured race and timeout composition is public through `std/task`;
standalone detached tasks and explicit task handles are not. URLs are provided by `std/url`, while
HTTP and TLS have separate public modules. No raw nonblocking socket mode is exposed.
