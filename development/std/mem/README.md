# Allocation and Failure

Exact allocator, allocation-context, layout, and raw-buffer declarations are owned by the
compiler-checked [`std/mem` contract](index.nct).

`Layout` validates size and alignment before a backend call. A zero-sized allocation produces a
canonical empty buffer without invoking the operating system. `RawBuffer` retains the actual
allocated layout, backend identity, and storage origin; user code cannot construct or mutate its
representation.

Aborting and recoverable operations are adapters over one allocation implementation, not separate
allocator engines. Normal operations select the current `Allocator`; corresponding `try_*`
operations explicitly select a `TryAllocator`. This distinction never changes unrelated I/O,
parsing, or domain failures into allocation failures.

`Allocator` operations either succeed or terminate immediately. Allocation termination returns no
recoverable value, performs no further allocation, does not unwind scopes or run pending drops, and
must not expose a partially updated buffer or collection. It uses a stable target-independent reason
and abnormal process status.

`TryAllocator` operations return the built-in error payload and are failure-atomic. Stable memory
codes include `std.mem.out_of_memory`, `std.mem.invalid_argument`, and
`std.mem.capacity_overflow`. Out-of-memory reporting uses a prebuilt static error and cannot
recursively allocate; failure while constructing another validation error terminates. Fixed arenas,
budgets, speculative large operations, compilers, and servers can use this recoverable policy even
when exhaustion of the process allocator would be fatal.

Normal `String`, `Vec`, path, split, buffered-I/O, and formatting construction uses the current
allocation context and aborts if allocation cannot continue. `String` conversion and numeric
formatting also expose explicit `try_*` operations for a recoverable `TryAllocator`. Repeated
`String` and `Vec` growth reserves geometrically so one-at-a-time append has amortized constant
growth cost; capacity may exceed the minimum requested amount. Checked capacity arithmetic rejects
an unrepresentable size before allocation and never wraps. Filesystem, I/O, invalid UTF-8, invalid
paths, and empty split separators remain recoverable `T!` failures.
