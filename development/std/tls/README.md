# Authenticated TLS

The compiler-checked [`std/tls` contract](index.nct) is the sole authority for exact public
declarations. This guide defines the authentication, ownership, progress, timeout, and failure
behavior shared by those declarations.

## Client Authentication

`TlsStream.connect` resolves one non-empty ASCII host through `std/net`, tries the resulting
numeric addresses in system order, and authenticates the requested host against the operating
system trust store. The host text remains the authentication identity even when connection falls
back between IPv6 and IPv4 candidates. TLS 1.2 is the minimum accepted protocol version; the
operating-system provider may negotiate a newer version.

`TrustAnchor.from_der` copies one non-empty DER certificate. The custom-trust connection
operations add that certificate to the operating-system roots; they do not replace system trust or
disable hostname authentication. Certificate parsing and path validation occur during connection,
so malformed non-empty DER is reported as `std.net.tls_failed`, not during byte ownership
construction. The asynchronous operations copy the anchor into the deferred computation and do not
borrow the caller's `TrustAnchor` after returning.

`tls.connect_async` and `tls.connect_async_with_timeout` perform resolution before returning a
deferred computation. Resolution failure therefore belongs to the outer result; authentication or
transport failure after the computation starts belongs to the awaited result. The timeout variant
starts one monotonic deadline before resolution and preserves it across every address candidate.

The `std/http` client selects this authenticated transport for `https` URLs. HTTP request and
response parsing remain transport-independent; selecting TLS does not introduce a second HTTP
codec or response-body cursor.

The numeric connection address is never used as an implicit replacement for the requested
authentication name. Native endpoints, trust objects, provider status values, callbacks, and
Security.framework records do not cross the standard-library boundary.

## Ownership and I/O

`TlsStream` uniquely owns one authenticated provider connection and implements the ordinary
`Reader` and `Writer` contracts. Reads return decrypted application bytes and zero only after clean
peer completion. Writes accept the complete plaintext view or report a failure after any prefix
already accepted by the provider remains observable.

Read and write timeout configuration has the same fixed monotonic-deadline meaning as `TcpStream`.
Timeouts do not restart after provider progress. Directional shutdown updates the common provider
stream state. Explicit `close` is terminal and idempotent. Destruction cancels and releases an open
stream once; release occurs only after the final provider state and serial callback-queue barrier
have both been observed.

Asynchronous establishment, reads, and writes use the same ordered provider event descriptor as
their synchronous counterparts. They suspend the executor instead of blocking its thread. Dropping
an unfinished computation cancels its registered wait, while dropping or closing the resulting
stream follows the same terminal provider-state and callback-queue barrier as synchronous use.

## Failures

Resolution and transport failures retain the stable `std.net.*` vocabulary. TLS provider failures
use `std.net.tls_failed`; native error domains and codes remain private. Failure of one resolved
candidate does not restart the timeout or change the authentication name, and the final error is
contextualized after every candidate fails.
