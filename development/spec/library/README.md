# Development Library Contracts

These chapters own adopted v2.0.0 changes to observable library behavior. They do not replace the
released declarations or assigned behavior guides under [`std/`](../../../std/README.md).
Unspecified API migration remains open; these contracts do not claim compiler/library admission.

- [Recoverable Errors](error.md): owned failure-graph snapshot and cleanup behavior.
- [Allocation and Failure](memory.md): physical backing versus exposed allocator obligations.
- [Vectors](vectors.md): capacity, allocator affinity and sequence-transformation obligations.
- [Prelude Value Construction](prelude.md): the ordinary some function for explicit optional layers.

The [development specification](../README.md) defines version scope and promotion policy.
