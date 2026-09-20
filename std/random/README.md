# Cryptographic Randomness

The compiler-checked [`std/random` contract](index.nct) exposes bytes supplied by the
operating-system cryptographic entropy source. `fill` accepts caller-owned mutable storage,
allocates nothing, and completes every byte before returning success. It accepts an empty
destination without contacting the operating system.

On the supported Darwin target, the bounded kernel entropy operation is a nonblocking callable in
Nocter's effect model: it does not wait for caller-controlled external progress. Random operations
therefore retain `noalloc` without carrying `blocking`. This is independent of ordinary execution
latency and does not weaken failure reporting.

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

`below_u64(upper_exclusive)` and `below_usize(upper_exclusive)` use rejection sampling. A zero
bound returns `std.random.empty_range` before consuming entropy. Remainder reduction occurs only
after excluding the biased prefix of the full-width domain.

`shuffle(values)` applies Fisher-Yates to a mutable slice. It does not allocate, does not require
copyable elements, and consumes no entropy for an empty or singleton slice. If entropy becomes
unavailable after permutation begins, the function returns `std.random.unavailable`; the slice
remains fully initialized but may already be partially permuted.

These functions provide unpredictable bytes, values, and permutations. They do not provide
deterministic random streams, password hashing, encryption, signatures, or a general cryptographic
protocol API.

## Responsibility Boundaries

The selected target owns only the native entropy operation and ABI. `std/internal/entropy` owns
request limits and retry. `std/random` owns public failure, cleanup, scalar assembly, sampling, and
permutation.
`std/bytes` remains the sole byte-order implementation, and `std/hash` consumes the same internal
entropy source without exposing its hash seed.
