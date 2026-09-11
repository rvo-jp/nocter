# Authenticated TLS

The compiler-checked [`std/tls` contract](index.nct) is the sole authority for exact public
declarations. This guide defines the authentication, ownership, progress, timeout, and failure
behavior shared by those declarations.

## Client Authentication

`TlsStream.connect_blocking` resolves one non-empty ASCII host through `std/net`, tries the
resulting numeric addresses in system order, and authenticates the requested host against the
operating system trust store. The host text remains the authentication identity even when
connection falls back between IPv6 and IPv4 candidates. TLS 1.2 is the minimum accepted protocol
version; the operating-system provider may negotiate a newer version. Every synchronous host
constructor is explicitly `blocking` because this path uses the synchronous system resolver.

`TrustAnchor.from_der` copies one non-empty DER certificate. The custom-trust connection
operations add that certificate to the operating-system roots; they do not replace system trust or
disable hostname authentication. Certificate parsing and path validation occur during connection,
so malformed non-empty DER is reported as `std.net.tls_failed`, not during byte ownership
construction. The asynchronous operations capture the supplied anchor borrow and copy its bytes
while the deferred computation is driven. The ordinary borrow checker therefore keeps the anchor
alive until that computation is awaited or destroyed.

`tls.connect` and `tls.connect_with_timeout` validate and retain the host endpoint while
their deferred computation is driven. Invalid host input, provider resolution, authentication, and
transport failure all belong to the single awaited `TlsStream!` result. The timeout variant starts
one monotonic deadline when the future begins and preserves it across validation, provider
resolution, and authentication. These operations do not call the synchronous resolver.

The `std/http` client selects this authenticated transport for `https` URLs. Its
`with_trust_anchor` policy carries one additional root through the TLS-owned connection boundary;
plain HTTP does not inspect it. The HTTP-specific TLS entry advertises only `http/1.1` and requires
the provider's negotiated application protocol to equal that value before exposing the stream.
Missing or different ALPN is a TLS failure. HTTP request and response parsing remain
transport-independent; selecting system or custom trust does not introduce a second HTTP codec or
response-body cursor.

The numeric connection address is never used as an implicit replacement for the requested
authentication name. Native endpoints, trust objects, provider status values, callbacks, and
Security.framework records do not cross the standard-library boundary.

## Ownership and I/O

`TlsStream` uniquely owns one authenticated provider connection and implements the ordinary
`BlockingReader` and `BlockingWriter` contracts. Reads return decrypted application bytes and zero
only after clean peer completion. Writes accept the complete plaintext view or report a failure after any prefix
already accepted by the provider remains observable.

Read and write timeout configuration has the same fixed monotonic-deadline meaning as `TcpStream`.
Timeouts do not restart after provider progress. Directional shutdown updates the common provider
stream state. Explicit `close` is terminal and idempotent. Close and destruction transfer an open
stream once to private cleanup; that transfer returns before the worker observes the final provider
state, crosses the serial callback-queue barrier, and releases native resources.

Asynchronous establishment, reads, and writes use the same ordered provider event descriptor as
their synchronous counterparts. They suspend the executor instead of blocking its thread. Dropping
an unfinished computation cancels its registered wait, while dropping or closing the resulting
stream transfers ownership through the same nonwaiting cleanup boundary.

## Failures

Resolution and transport failures retain the stable `std.net.*` vocabulary. TLS provider failures
use `std.net.tls_failed`; native error domains and codes remain private. Provider endpoint
selection does not restart the timeout or change the authentication name.
