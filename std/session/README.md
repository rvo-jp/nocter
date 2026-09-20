# Opaque Session Identifiers

The compiler-checked [`std/session` contract](index.nct) owns one opaque 256-bit identifier. An
issued `SessionId` receives exactly 32 bytes from `std/random`; its canonical transport spelling is
URL-safe unpadded Base64 from `std/base64`. Parsing accepts only that 43-character canonical form.

`SessionId` stores bytes rather than retaining a second text representation. Equality, ordering,
and hashing consume those same bytes. The type intentionally does not implement `Format`: a
generic formatter or structured logger cannot reveal a bearer credential accidentally. `reveal`
is the explicit boundary for a cookie, protocol field, or storage key that actually needs text.
Equality examines all 32 bytes before returning; ordering remains ordinary lexicographic ordering.

An identifier is an unpredictable lookup key, not a signed claim. Session data, expiration,
revocation, rotation, and concurrent storage belong to the application. HTTP cookie policy belongs
to `std/http`; this module neither reads headers nor keeps process-global session state.
