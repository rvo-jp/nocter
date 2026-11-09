# Prelude Value Construction

## some

The adopted v2.0.0 synthetic prelude exposes this ordinary function contract from its own
`std/prelude` declaration surface:

```text
func some<T>(take value: T): T?
```

It constructs exactly one present layer around its received value. Supply uses ordinary
Copy-or-move: a Copy source remains available, an eligible non-Copy source transfers, and a
read/edit alias without Copy cannot supply ownership. Existing optional/fallible tags inside T
are preserved, never flattened. Ordinary result injection in a generic body suffices to implement
the operation; no identifier-based compiler intrinsic, extra allocator request, wrapper heap
object or new source keyword is implied.

Context or complete explicit arguments determine T when its value is `none` or otherwise
insufficient for inference. The [outcome contract](../language/outcomes.md) owns injection and
nesting, while the [generic contract](../language/generics-and-constants.md) owns argument inference.
No const/evaluator or target ABI capability is implicitly promised by this signature.

Synthetic fallback uses the actual selected declaration identity. An authored local name can
shadow it under the prelude name-resolution policy; calling that other `some` does not retain
these semantics by spelling. The contextual `some` in an opaque result type is independent.
This is an adopted unreleased API change. The published `std/prelude` source and export set are
unchanged until coherent library/toolchain migration.
