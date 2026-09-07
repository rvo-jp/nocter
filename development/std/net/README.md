# Synchronous Network I/O

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

## TCP Streams and Listeners

`TcpStream` is a uniquely owned byte stream implementing `Reader` and `Writer`. Connecting accepts
one numeric `SocketAddress`; host-name resolution is deliberately separate. Reads initialize at
most the supplied mutable byte view and return zero at peer EOF. Writes complete the entire byte
view or return a failure after any already-written prefix remains observable. Empty transfers
follow the ordinary stream contracts.

`TcpListener` binds one numeric address and accepts uniquely owned streams. Port zero asks the
kernel to select an available port; `local_address` reports the effective address. `accept` also
returns the connected peer address. IPv6 sockets are explicitly IPv6-only, so code serving both
families owns one listener for each family.

Socket descriptors are never exposed. A successful constructor transfers one descriptor into one
move-only public value. Explicit `close` is terminal and idempotent; destruction closes a still-open
descriptor at most once. `shutdown` changes the selected stream direction without releasing the
descriptor. Stream writes cannot terminate the process through `SIGPIPE`, and owned descriptors do
not leak across process execution.

TCP operations are synchronous. `connect_with_timeout` bounds connection establishment.
`set_read_timeout` and `set_write_timeout` bound later stream operations, while
`set_accept_timeout` bounds listener acceptance. Passing absence to a setter restores unlimited
waiting. The corresponding observation methods return the exact configured `Duration?`.

The implementation uses nonblocking descriptors internally only to centralize interruption,
readiness, and deadline handling; this does not expose a public nonblocking mode. Public failures
use stable `std.net.*` codes rather than native errno values. The stable categories include closed
sockets, timeout, connection refusal, connection reset or abort, address conflict or
unavailability, unreachable networks, permission denial, broken pipes, oversized datagrams,
unsupported operations, and invalid target results.

## UDP Datagrams

`UdpSocket` is a uniquely owned datagram endpoint and deliberately does not implement `Reader` or
`Writer`. Binding port zero and reporting the effective local address behave like TCP listeners.
`send_to` supplies an address for one message. `connect` selects one peer for later `send`
operations and makes that peer available through `peer_address`; it does not turn UDP into a byte
stream.

Every `send` or `send_to` transmits one complete datagram or returns an error. Every `receive`
consumes at most one datagram and returns one `DatagramRead` containing its source address, the
number of bytes copied, and an explicit truncation flag. If a datagram exceeds the destination
buffer, the unread suffix is discarded and cannot appear in the next receive. A zero-length
datagram is a successful receive with copied length zero, not end of stream.

UDP read timeouts bound `receive`. UDP write timeouts bound `connect`, `send`, and `send_to`.
Timeout configuration does not change datagram boundaries or expose native socket options.

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

Numeric addresses, synchronous TCP, boundary-preserving UDP, and monotonic operation timeouts are
implemented. Name resolution, URLs, HTTP, TLS, async I/O, and public nonblocking sockets are outside
v0.39.0.
