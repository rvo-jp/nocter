# Cryptographic Randomness

`std/random` exposes bytes supplied by the operating-system cryptographic entropy source. `fill`
accepts caller-owned mutable storage, allocates nothing, and completes every byte before returning
success. It accepts an empty destination without contacting the operating system.

Native request-size limits are not part of the public contract. The package-internal entropy
adapter divides a large destination into bounded requests and retries interrupted operations. The
public module never exposes native error numbers.

If any native request fails, `fill` clears the complete destination before returning
`std.random.unavailable`. A caller therefore cannot accidentally treat a successfully generated
prefix as a complete secret. Clearing is an observable safety property, not a guarantee against
all compiler, processor, or operating-system copies of prior memory contents.

`next_u64` fills eight bytes and interprets them through the standard library's canonical
little-endian scalar codec. Every possible bit pattern is accepted, so its output is uniform over
the complete `u64` domain. `next_usize` uses the complete native `usize` width on the supported
64-bit target. Neither operation retains hidden generator state.

These functions provide unpredictable bytes and values. They do not provide deterministic random
streams, bounded distributions, shuffling, password hashing, encryption, signatures, or a general
cryptographic protocol API.

## Responsibility Boundaries

The selected target owns only the native entropy operation and ABI. `std/internal/entropy` owns
request limits and retry. `std/random` owns public failure, cleanup, and scalar assembly.
`std/bytes` remains the sole byte-order implementation, and `std/hash` consumes the same internal
entropy source without exposing its hash seed.
