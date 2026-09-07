# Numeric Network Addresses

The compiler-checked [`std/net` contract](index.nct) is the sole authority for exact public
declarations. v0.39.0 builds synchronous TCP and UDP over these target-independent address values;
this guide expands only observable behavior already present in the checked contract.

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

## Current Boundary

The current checked module contains numeric address values only. TCP streams, listeners, UDP
datagrams, and synchronous deadlines enter in later v0.39.0 phases after the shared descriptor and
target-adapter boundary is implemented. Name resolution, URLs, HTTP, TLS, async I/O, and public
nonblocking sockets are outside v0.39.0.
