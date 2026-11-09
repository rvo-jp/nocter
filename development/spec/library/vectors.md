# Vectors

## Storage Optimization

Under the future [storage rules](../language/memory-and-regions.md#storage-optimization),
a vector may be scalarized or its backing reused only when the selected API's logical sequence,
capacity, initialized prefix, cleanup, failure atomicity, and allocator affinity remain valid.
This does not change the current declarations or published guide.

Construction-time allocator selection remains meaningful even if no current bytes are materialized.
A zero-capacity or optimized-away buffer cannot acquire an unrelated allocator on later growth.
Reservation must satisfy its capacity guarantee before exposing it, or have a checked virtual plan
that needs no allocation on the guaranteed no-growth paths. Deferring a necessary request into
such a path is not allocation elimination. Recoverable requests retain their failure and state
obligations even when the resulting bytes would otherwise be dead.

Eliminating an intermediate sequence must preserve element ownership, callback order, first
failure, and resource destruction. A read-only callback is not necessarily pure or total. Streaming
fusion that changes these observations needs a stronger proof or must retain the original schedule;
no source keyword or special interpretation of a method's spelling grants that proof.
