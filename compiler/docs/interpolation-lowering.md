# Interpolation Lowering Direction

This document records the implementation direction for bare string
interpolation. The source-language rules live in
[Strings, Arrays, Views, and Pointers](../../spec/07-strings-arrays-views-pointers.md#string-interpolation).

## Current Boundary

Interpolated strings are parsed and typechecked as `String!`, but bare
interpolation is not runtime-shipped. Buildability must reject source such as:

```nct
let message = "count: ${count}"
```

The reason is allocator ownership. A bare interpolation expression has no
explicit allocator source, and v0 should not silently choose a global allocator.

## Supported Manual Shape

Code that wants an owned formatted string should currently allocate explicitly:

```nct
use std/fmt.append_i32
use std/fmt.append_str
use std/mem.page_allocator
use std/string.String

func message(count: i32): String! {
    var allocator = page_allocator()
    var out = String.with_capacity(&+allocator, 32)?
    append_str(&+out, "count: ")?
    append_i32(&+out, count)?
    return move out
}
```

This keeps allocation, failure, and ownership visible in source.

## Promotion Requirement

Bare interpolation can be promoted only after the language chooses one of:

- an explicit allocator operand in the interpolation syntax
- a scoped allocator context with clear lifetime and failure behavior
- a standard-library formatting builder that user source invokes explicitly

Promotion must update:

- `../../spec/07-strings-arrays-views-pointers.md`
- `../../spec/11-stdlib-primitives-os.md`
- `implementation-status.md`
- `v0-closure.md`
- parser/typecheck/buildability/lowering/runtime tests

Until then, buildability rejection is the correct behavior.
