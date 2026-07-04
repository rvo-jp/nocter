# Memory, Regions, and Allocators

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

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
let text = try file.read_to_string(allocator)
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
    let source = try read_file(scratch.allocator(), "main.nct")
    let tokens = try lex(scratch.allocator(), source.view())
}
```

## Allocators

Adopted: allocator APIs live in the standard library module `std.mem`.

The initial file for this module is:

```text
.nocter-arm64-macos/std/mem.nct
```

`Allocator`, `AllocError`, `Layout`, and `RawBuffer` are ordinary standard-library names. The compiler must not special-case those identifiers.

The special language behavior is limited to:

- the `region name using allocator_expr { ... }` syntax
- provenance tracking for allocator values derived from a region handle
- escape checking for values, borrows, and views backed by region allocation

Initial public surface:

```nct
pub enum AllocError {
    out_of_memory
    invalid_layout
}

pub copy struct Layout {
    pub size: usize
    pub align: usize
}

pub struct RawBuffer {
    pub ptr: *u8
    pub len: usize
    pub align: usize
}

pub struct Allocator {
    ...
}

pub func page_allocator(): Allocator
pub func alloc(allocator: &+Allocator, size: usize, align: usize): RawBuffer!AllocError
pub func free(allocator: &+Allocator, buffer: RawBuffer): void
```

In primitive set v0, allocator operations are not compiler primitives. These public functions should be implemented in the standard library, eventually through target-overlay syscall wrappers and ordinary Nocter code. Names such as `raw_alloc` and `raw_free` are not part of the compiler's closed primitive set.

Rules:

- Allocation is explicit.
- Allocation APIs are fallible and return `T!AllocError` where allocation can fail.
- Out-of-memory is reported as `AllocError.out_of_memory`.
- Invalid size / alignment requests are reported as `AllocError.invalid_layout`.
- Allocation mutates allocator state, so allocation APIs take `&+Allocator`.
- `RawBuffer` is a low-level standard-library type for untyped bytes.
- General application code should prefer owning wrappers such as `String` and `Buffer<T>`.
- `RawBuffer` values allocated from a region allocator are region-owned and cannot escape that region.
- `Allocator` values derived from a region handle cannot escape that region.
- `free` consumes the `RawBuffer`; after `mem.free(&+allocator, move buffer)`, the original binding is no longer valid.

Example:

```nct
import std.mem as mem
import std.mem.{Allocator, AllocError, RawBuffer}

func make_bytes(allocator: &+Allocator, size: usize): RawBuffer!AllocError {
    let buffer = try mem.alloc(allocator, size, 16)
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
    let source = try read_file(scratch.allocator(), "main.nct")
    let tokens = try lex(scratch.allocator(), source.view())
    let ast = try parse(scratch.allocator(), tokens.view())

    use(ast)
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
- The compiler tracks values allocated through a region allocator as region-owned.
- Owned values backed by region allocation cannot be moved out of the region.
- Borrows of region-owned values cannot escape the region.
- Borrow-like views into region-owned storage cannot escape the region.
- `StringView`, `View<T>`, `WriteView<T>`, raw pointers, or user structs that carry region-derived storage cannot escape the region.
- A copy value may leave the region only if its type and value do not carry region-derived storage.
- Pure copy values such as integers, booleans, and copy structs containing only region-independent fields may leave the region.
- Returning a region-owned value from inside the region is a compile error.
- Storing a region-derived view into an outer variable is a compile error.
- Assigning a region-owned value into a place that outlives the region is a compile error.
- At normal region exit, local owned values still live inside the region body are dropped in reverse declaration order before the region releases its backing memory.
- `return`, `fail`, `break`, and `continue` that leave a region run the same region cleanup before transferring control.
- Calling a `never` function inside a region does not imply stack unwinding or region cleanup. Cleanup must be explicit before such a call if it matters.
- The exact allocator implementation may be arena-like, bump-like, or another standard-library strategy. The language feature is the scoped region and escape check, not a specific allocator algorithm.

Invalid:

```nct
func load_path(allocator: &+Allocator): StringView!IOError {
    region scratch using allocator {
        let text = try read_file(scratch.allocator(), "main.nct")
        return text.view() // error: view points into scratch
    }
}
```

Invalid:

```nct
func load_text(allocator: &+Allocator): String!IOError {
    region scratch using allocator {
        let text = try read_file(scratch.allocator(), "main.nct")
        return move text // error: String is backed by scratch
    }
}
```

Valid:

```nct
func count_words(allocator: &+Allocator, path: StringView): WordStats!IOError {
    region scratch using allocator {
        let text = try read_file(scratch.allocator(), path)
        let stats = scan_words(text.view())
        return stats
    }
}
```

`WordStats` can leave the region if it is a copy value containing only counters and no region-derived storage.
