# Interpolation Lowering Direction

This note records the implementation direction for lowering interpolated strings without adding compiler magic.

Normative language rules remain in `../../spec/`. This file describes the compiler implementation plan.

## Current Constraint

An interpolated string expression such as `"hello ${name}"?` has type `String!`, but the source form does not carry an allocator operand.
The specification also forbids silently choosing a process-global allocator.

Therefore the compiler must keep bare interpolation lowering disabled until the language or standard-library surface provides an explicit allocator source.
The backend should first make the explicit standard-library construction path buildable:

```nct
var out = string.with_capacity(&+allocator, capacity)?
fmt.append_str(&+out, "hello ")?
fmt.append_i32(&+out, count)?
fmt.append_bool(&+out, ready)?
return move out
```

This keeps allocation, mutation, formatting, and failure visible in ordinary Nocter code.

## Lowering Shape

Once an explicit allocator source exists, interpolation lowering should be equivalent to this sequence:

1. Evaluate the allocator expression exactly once.
2. Compute or conservatively choose an initial capacity.
3. Call `std/string.with_capacity(&+allocator, capacity)?`.
4. For each literal text segment, call `std/fmt.append_str(&+out, segment)?`.
5. For each interpolation expression, evaluate it exactly once in source order.
6. Dispatch to the supported append function for the expression type:
   - `str` -> `std/fmt.append_str`
   - `String` -> `std/fmt.append_string`
   - `i32` -> `std/fmt.append_i32`
   - `bool` -> `std/fmt.append_bool`
7. Return the owned `String`.

Every fallible call in the sequence propagates the built-in `error` payload. The compiler must not synthesize domain-specific string or formatting errors.

## Backend Prerequisites

Already buildable in the narrow scalar subset:

- loaded imported scalar calls
- scalar `i32`/`usize`/`bool` parameters and call arguments
- static string literals and `str` parameters as `str` call arguments
- direct `str` returns from static string literals, `str` parameters, `str` locals, and tail calls
- `str` normal-call result staging for annotated `str` locals and nested `str` call arguments
- scalar call-result staging and scalar tail-call staging

Still required before explicit string construction can build:

- aggregate storage for `String`, `Allocator`, and `RawBuffer`
- stack-backed `var` bindings and reassignment for mutable owned values
- borrow argument lowering for `&T` and `&+T`
- fallible propagation for non-entry functions and ordinary calls returning `T!`
- explicit move/return handling for owned aggregate results

## Open Language Decision

Bare interpolation syntax still needs an allocator source. Viable options must keep the allocator explicit at the source level, for example an allocator-bearing expression form or a standard-library formatting API that takes the allocator as an ordinary argument.
Until that decision is made, lowering should continue to reject bare interpolation with `E8008`.
