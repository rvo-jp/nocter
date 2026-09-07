# URL

The compiler-checked [`std/url` contract](index.nct) is the sole authority for exact public
declarations. `std/url` owns immutable absolute HTTP-family URLs. `Url.parse` accepts complete RFC
3986 hierarchical URLs with `http` or `https` schemes and retains normalized logical components
rather than the caller's source text.

The initial URL boundary accepts numeric IPv4 hosts, bracketed numeric IPv6 hosts, and ASCII
registered names. User information and raw non-ASCII registered names are rejected. International
host names require a future explicit IDNA contract instead of locale-dependent conversion.

Canonical text uses lowercase schemes and registered names, canonical numeric addresses, uppercase
percent-escape digits, removed dot segments, `/` for an omitted path, and no explicit scheme-default
port. Repeated non-dot path segments and present-empty queries or fragments remain distinguishable.
`request_target` returns the path and optional query and never includes a fragment.
`authority` returns the canonical host and any non-default port for an HTTP `Host` field; IPv6
addresses retain the required brackets.

`resolve` applies RFC 3986 relative-reference resolution to the retained components. A reference
with its own authority replaces the host, port, path, and query; an empty-path reference inherits
the base query only when it has no authority and no query of its own. A complete `http` or `https`
URL replaces the base. The returned value is independently owned and does not borrow the base URL.

`parse` follows the ordinary abort-on-allocation-exhaustion policy. `try_parse`, `try_to_string`,
`try_authority`, and `try_request_target` use caller-provided recoverable allocation; `try_resolve`
does the same for relative resolution. Syntax failures use stable `std.url.*` codes and are never
relabeled as allocation failures. Raw non-ASCII URL components must be percent-encoded.

Equality, hashing, formatting, projections, and request-target generation consume the same retained
component representation. Consumers must not parse canonical text again to recover URL meaning.
