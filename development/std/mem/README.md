# Allocation and Failure

Exact allocator, allocation-context, layout, and raw-buffer declarations are owned by the
compiler-checked [`std/mem` contract](index.nct).

Normal `String`, `Vec`, path, split, buffered-I/O, and formatting construction uses the current
allocation context and aborts if allocation cannot continue. `String` conversion and numeric
formatting also expose explicit `try_*` operations for a recoverable `TryAllocator`. Repeated
`String` and `Vec` growth reserves geometrically so one-at-a-time append has amortized constant
growth cost; capacity may exceed the minimum requested amount. Checked capacity arithmetic rejects
an unrepresentable size before allocation and never wraps. Filesystem, I/O, invalid UTF-8, invalid
paths, and empty split separators remain recoverable `T!` failures.
