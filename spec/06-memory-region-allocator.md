# Memory, Regions, and Allocators

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Memory Management

Nocter does not use GC.

The memory model is based on:

- ownership
- moves
- borrowing
- automatic destruction at scope end
- explicit allocators
- region allocation for bounded temporary memory
- compile-time escape checking for borrows, views, and region-backed values

Owned values are destroyed when their scope ends unless ownership is moved.

```nct
let text = file.read_to_string(allocator)?
return move text
```

After `move`, the original binding cannot be used.

```nct
let a = Buffer.alloc(1024)
let b = move a
// a is no longer valid here
```

Temporary allocation uses regions when the lifetime is bounded by a lexical block.

```nct
region scratch using allocator {
    let source = read_file(scratch.allocator(), "main.nct")?
    let tokens = lex(scratch.allocator(), source.view())?
}
```

## Allocators

Adopted: allocator APIs live in the standard library module path `std/mem`.

The initial file for this module is:

```text
~/.nocter/std/mem.nct
```

`Allocator`, `Layout`, and `RawBuffer` are ordinary standard-library names. The compiler must not special-case those identifiers.

The special language behavior is limited to:

- the `region name using allocator_expr { ... }` syntax
- provenance tracking for allocator values derived from a region handle
- escape checking for values, borrows, and views backed by region allocation

Initial public surface:

```nct
pub copy struct Layout {
    pub(nocter) size: usize
    pub(nocter) align: usize
}

pub struct RawBuffer {
    pub(nocter) ptr: *u8
    pub(nocter) len: usize
    pub(nocter) align: usize
    ...
}

pub struct Allocator {
    ...
}

pub func page_allocator(): Allocator
pub func Layout.new(size: usize, align: usize): Layout!
pub func layout(size: usize, align: usize): Layout!
pub func layout_size(value: &Layout): usize
pub func layout_align(value: &Layout): usize
pub func alloc(allocator: &+Allocator, size: usize, align: usize): RawBuffer!
pub func alloc_layout(allocator: &+Allocator, layout: Layout): RawBuffer!
pub func grow(allocator: &+Allocator, buffer: &+RawBuffer, new_size: usize): void!
pub func free(allocator: &+Allocator, buffer: RawBuffer): void
pub func out_of_memory(): error
pub func invalid_argument(): error
```

In v0.2.0, allocator operations are not compiler primitives. These public functions are implemented in the standard library through target-gated syscall wrappers and ordinary Nocter code. Names such as `raw_alloc` and `raw_free` are not part of the compiler's closed primitive set.

Rules:

- Allocation is explicit.
- Allocation APIs are fallible and return `T!` where allocation can fail.
- The failure payload is always the built-in `error`; `std/mem` must not define `AllocError`, `Result<T, E>`, or another domain-specific fallible payload.
- Out-of-memory is reported as `"std.mem.out_of_memory"`.
- Invalid size / alignment requests are reported as `"std.mem.invalid_argument"`.
- Allocation mutates allocator state, so allocation APIs take `&+Allocator`.
- Alignment must be a non-zero power of two supported by the target allocator.
- A zero-size request produces a canonical empty buffer and performs no OS allocation.
- `grow` is failure-atomic: allocation failure leaves pointer, length, alignment, provenance, and bytes unchanged.
- `RawBuffer` is a low-level standard-library type for owned untyped bytes.
- `RawBuffer`'s representation fields are `pub(nocter)`. User code cannot
  construct a `RawBuffer` literal or inspect its pointer, length, and alignment
  fields directly; it must use `std/mem` functions and methods such as `bytes`,
  `bytes_mut`, `prefix`, `prefix_mut`, and `free`.
- `RawBuffer` retains the private provenance required to release through its creating allocator and drops its allocation automatically if it is not explicitly consumed by `free`.
- General application code should prefer owning wrappers such as `String` and `Buffer<T>`.
- `RawBuffer` values allocated from a region allocator are region-owned and cannot escape that region.
- `Allocator` values derived from a region handle cannot escape that region.
- `free` consumes the `RawBuffer`; after `mem.free(&+allocator, move buffer)`, the original binding is no longer valid.

Example:

```nct
use std/mem.{Allocator, RawBuffer}

use std/mem as mem

func make_bytes(allocator: &+Allocator, size: usize): RawBuffer! {
    let buffer = mem.alloc(allocator, size, 16)?
    return move buffer
}
```

## Regions

Adopted: `region` is a language construct, not only a standard-library arena API.

Syntax:

```text
region name using allocator_expr {
    statements
}
```

The `name` after `region` is an ordinary block-local binding name for the region handle. It has no special meaning by spelling. `scratch`, `temp`, `work`, and `arena` are all ordinary names.

Example:

```nct
region scratch using allocator {
    let source = read_file(scratch.allocator(), "main.nct")?
    let tokens = lex(scratch.allocator(), source.view())?
    let ast = parse(scratch.allocator(), tokens.view())?

    consume(ast)
}
```

Rules:

- `region name using allocator_expr { ... }` creates a lexical region scope.
- `allocator_expr` is evaluated before entering the region body.
- The region body is a block and creates a normal scope.
- The region handle binding exists only inside the region body.
- The region handle binding cannot be reassigned by user code.
- The region handle cannot be explicitly dropped or moved out of the region.
- The region handle provides access to a region allocator through ordinary standard-library APIs such as `name.allocator()`.
- The compiler tracks allocator values derived from a region handle by provenance, not by special-casing the method name `allocator`.
- Region-derived values use the `region` provenance source kind described in [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md#borrow-like-provenance).
- The compiler tracks values allocated through a region allocator as region-owned.
- Owned values backed by region allocation cannot be moved out of the region.
- Borrows of region-owned values cannot escape the region.
- Borrow-like views into region-owned storage cannot escape the region.
- `&str`, `&[T]`, `&+[T]`, raw pointers, or user structs that carry region-derived storage cannot escape the region.
- A copy value may leave the region only if its type and value do not carry region-derived storage.
- Pure copy values such as integers, booleans, and copy structs containing only region-independent fields may leave the region.
- Returning a region-owned value from inside the region is a compile error.
- Storing a region-derived view into an outer variable is a compile error.
- Assigning a region-owned value into a place that outlives the region is a compile error.
- At normal region exit, local owned values still live inside the region body are dropped in reverse declaration order before the region releases its backing memory.
- `return`, `break`, and `continue` that leave a region run the same region cleanup before transferring control.
- Calling a `never` function inside a region does not imply stack unwinding or region cleanup. Cleanup must be explicit before such a call if it matters.
- The exact allocator implementation may be arena-like, bump-like, or another standard-library strategy. The language feature is the scoped region and escape check, not a specific allocator algorithm.

Invalid:

```nct
func load_path(allocator: &+Allocator): &str! {
    region scratch using allocator {
        let text = read_file(scratch.allocator(), "main.nct")?
        return text.view() // error: view points into scratch
    }
}
```

Invalid:

```nct
func load_text(allocator: &+Allocator): String! {
    region scratch using allocator {
        let text = read_file(scratch.allocator(), "main.nct")?
        return move text // error: String is backed by scratch
    }
}
```

Valid:

```nct
func count_words(allocator: &+Allocator, path: &str): WordStats! {
    region scratch using allocator {
        let text = read_file(scratch.allocator(), path)?
        let stats = scan_words(text.view())
        return stats
    }
}
```

`WordStats` can leave the region if it is a copy value containing only counters and no region-derived storage.
