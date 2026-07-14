# Interpolation Lowering Direction

This note records the implementation direction for lowering interpolated strings without adding compiler magic.

Normative language rules remain in `../../spec/`. This file describes the compiler implementation plan.

## Current Constraint

An interpolated string expression such as `"hello ${name}"?` has type `String!`, but the source form does not carry an allocator operand.
The specification also forbids silently choosing a process-global allocator.

Therefore the compiler must keep bare interpolation lowering disabled until the language or standard-library surface provides an explicit allocator source.
The backend can now build the explicit standard-library construction shape through the current stub `std/string` and `std/fmt` APIs:

```nct
var out = string.with_capacity(&+allocator, capacity)?
fmt.append_str(&+out, "hello ")?
fmt.append_i32(&+out, count)?
fmt.append_bool(&+out, ready)?
return move out
```

This keeps allocation, mutation, formatting, and failure visible in ordinary Nocter code.
The current standard-library bodies still report allocation as unsupported, so this path is buildable but not allocation-backed runtime string construction yet.

## Lowering Shape

Once an explicit allocator source exists, interpolation lowering should be equivalent to this sequence:

1. Evaluate the allocator expression exactly once.
2. Compute or conservatively choose an initial capacity.
3. Call `std/string.with_capacity(&+allocator, capacity)?`.
4. For each literal text segment, call `std/fmt.append_str(&+out, segment)?`.
5. For each interpolation expression, evaluate it exactly once in source order.
6. Dispatch to the supported append function for the expression type:
   - `&str` -> `std/fmt.append_str`
   - `String` -> `std/fmt.append_string`
   - `i32` -> `std/fmt.append_i32`
   - `bool` -> `std/fmt.append_bool`
7. Return the owned `String`.

Every fallible call in the sequence propagates the built-in `error` payload. The compiler must not synthesize domain-specific string or formatting errors.

## Backend Prerequisites

Already buildable in the narrow scalar subset:

- loaded imported scalar calls
- scalar `i32`/`usize`/`bool` parameters and call arguments
- static string literals and `&str` parameters as `&str` call arguments
- direct `&str` returns from static string literals, `&str` parameters, `&str` locals, and tail calls
- `&str` normal-call result staging for annotated `&str` locals and nested `&str` call arguments
- scalar call-result staging and scalar tail-call staging
- fallible propagation for non-entry functions and ordinary calls returning `T!` in the current scalar/view/void call subset
- stack-backed scalar/view `var` bindings and simple whole-binding `=` assignment
- local scalar borrow argument lowering for `&T` and `&+T` normal-call parameters
- ABI-indirect aggregate call-result `let`/`var` slots, including propagated fallible aggregate calls
- direct aggregate normal-call result `let`/`var` slots for 16-byte-or-smaller values such as `std/mem.Allocator`
- fallible direct aggregate call-result `let`/`var` slots for 16-byte-or-smaller values such as `std/mem.Allocator`
- aggregate struct-literal local slots with 8-byte integer fields or `std/ptr.from_addr` pointer fields
- aggregate slot reassignment from supported struct literals, normal or propagated fallible aggregate call results, and matching copy struct local aggregate slots
- local aggregate slot borrow argument lowering for `&T` and `&+T` normal-call parameters
- return-by-name from reserved aggregate slots
- `return move name` from reserved aggregate slots, with straight-line, conditional, switch, loop, and reachability ownership-state checking
- explicit `drop name` lowering for reserved aggregate locals whose type declares a drop member
- straight-line scope-end drop insertion for aggregate locals and by-value aggregate parameters whose type declares a drop member
- distributed `std/mem.page_allocator`, `std/string.with_capacity`, `std/fmt.append_str`, and `return move out` in the explicit construction shape build to Mach-O with the current stub standard-library bodies

Still required before allocation-backed string construction can run:

- propagation-failure plus branch/loop/catch/tail-call scope-end drop insertion, replacement drop lowering, and ownership-fact export from type checking to lowering
- target-backed allocation and mutation in `std/mem`, `std/string`, and `std/fmt`

## Open Language Decision

Bare interpolation syntax still needs an allocator source. Viable options must keep the allocator explicit at the source level, for example an allocator-bearing expression form or a standard-library formatting API that takes the allocator as an ordinary argument.
Until that decision is made, lowering should continue to reject bare interpolation with `E8008`.
