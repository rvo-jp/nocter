# Backend V0

This document describes the current native backend implementation boundary.
The public ABI rules live in [ABI and Layout](../../spec/09-abi-layout.md).

## Target

The v0 native backend targets `arm64-darwin` and writes Mach-O executables
directly. It does not invoke LLVM, `clang`, `as`, `ld`, Xcode Command Line
Tools, or an external linker.

Reserved future target names may be recognized by the CLI, but unsupported
targets must diagnose explicitly before backend emission.

## Pipeline Position

```text
typed compile unit
  -> buildability preflight
  -> IR lowering
  -> ABI classification and validation
  -> ARM64 code generation
  -> Mach-O image
```

The backend consumes IR and ABI facts. It should validate inconsistent IR, but
ordinary user source outside the v0 runtime subset should be rejected earlier by
buildability diagnostics.

## ABI Summary

Nocter ABI v0 is register-first:

- one ABI word is 8 bytes
- scalar `i32`, `u8`, and `bool` occupy one ABI word but use 32-bit or narrower
  value operations as appropriate
- `usize` and raw pointers occupy one ABI word
- public raw pointer conversions lower raw pointers as address-carrying `usize`
  ABI words; pointer dereference is not part of v0 backend support
- ABI layout recognizes additional integer widths for aggregate storage, but
  `i8`, `i16`, `i64`, `isize`, `u16`, `u32`, and `u64` currently accept only
  literal initialization and replacement inside supported aggregates;
  standalone runtime scalar values remain limited to the documented buildable
  subset
- `&str`, `&[T]`, and `&+[T]` occupy two ABI words: pointer then length
- direct aggregates are aggregate values of 16 bytes or less
- indirect aggregates are aggregate values larger than 16 bytes
- direct aggregate returns use `x0` and optionally `x1`
- indirect aggregate returns use caller-provided return storage
- normal-call argument words after the first eight use the caller stack argument
  area
- tail calls remain narrower than normal calls and must not branch with borrow
  arguments that point into a released caller frame

ABI classification belongs in `abi`. Lowering and backend validation should
reuse that classification rather than duplicating layout rules.

## Frame And Call Model

Frameless functions are still valid for simple leaf paths. Functions that need
normal calls, stack-passed arguments, saved return addresses, aggregate slots,
or parameter spill addresses use a fixed-size 16-byte-aligned stack frame.

The current frame model supports:

- saved `x30` for normal calls
- conservative scalar spill/reload around calls
- parameter ABI-word spills when later code needs stable parameter values or
  borrow addresses
- aggregate slots for local aggregate values and staged call results
- caller stack argument space for normal calls with more than eight ABI words
- hidden return storage handling for indirect aggregate calls and returns

The implementation intentionally favors correctness and clear validation over
fine-grained liveness optimization.

## Runtime-Shipped Codegen

The backend currently supports the documented subset of:

- entry wrappers for `i32`, `usize`, `void`, `i32!`, `usize!`, and `void!`
- scalar and view locals, parameters, returns, calls, checked i32 unary
  negation, arithmetic, numeric compound assignments, comparisons, bool
  operations, runtime traps, computed identity conversions for `i32`, `u8`, and
  `usize`, and `u8` widening to `i32`/`usize`
- static string literals and `&str` ABI passing
- slice length, selected indexing, bounds-checked stores, and numeric
  scalar/view slice element compound assignments
- same-file and loaded imported calls in the current scalar/view/aggregate
  subset
- concrete generic specializations whose lowered bodies stay inside the runtime
  subset
- fallible and optional propagation, force unwrap, `catch`, and direct-call
  `otherwise` in supported scalar/view value, binding, assignment, and return
  paths, supported aggregate/fixed-array member root, binding, argument,
  aggregate-field initializer, assignment, and return paths
- terminal and selected non-terminal control flow with cleanup
- payloadless enum tag comparisons, `if is`, and `match`
- direct and indirect aggregate parameters, arguments, returns, call-result
  slots, copies, field loads/stores, explicit moves, replacement assignment, and
  scope cleanup
- owned std values such as `String`, `Vec<T>` in shipped element paths, `File`,
  allocator buffers, and their drop members
- target-gated std primitives for traps, syscalls, I/O, allocation, process
  context, and process termination

## Aggregate ABI Status

The aggregate path has been promoted beyond scalar-only codegen. Current support
includes:

- direct aggregate values up to 16 bytes, including partial final ABI words
- indirect aggregate values larger than 16 bytes through slot pointers or hidden
  return storage
- payload-carrying enum ABI layout as a tag byte plus aligned payload union
- payload-carrying enum construction, local slots, returns, and value arguments
  for the current runtime-supported payload subset, including direct enum
  constructors in value-argument position
- tag-only payload-carrying enum `if is Enum.variant(_)` statements over
  existing enum bindings and parameters, plus supported
  plain/forced/propagated/caught direct-call, constructor, and move-local
  pattern targets, in the current runtime-supported payload subset
- payload-carrying enum `if is Enum.variant(binding)` statements and value
  expressions over existing enum bindings and parameters, plus supported
  plain/forced/propagated/caught direct-call, constructor, and move-local
  pattern targets, when the bound payload ABI is `i32`, `u8`, `usize`, `bool`,
  `&str`, a slice view, a copy aggregate value, or an owned aggregate value with
  runtime-supported recursive drop glue
- tag-only payload-carrying enum `match` statements over existing enum bindings
  and parameters, plus supported plain/forced/propagated/caught direct-call,
  constructor, and move-local pattern targets, in the current runtime-supported
  payload subset, including wildcard-only, nonexhaustive no-wildcard, and
  exhaustive no-wildcard statement forms
- payload-carrying enum `match` statement and value-expression arm bindings over
  existing enum bindings and parameters, plus supported
  plain/forced/propagated/caught direct-call, constructor, and move-local
  pattern targets, when the bound payload ABI is `i32`, `u8`, `usize`, `bool`,
  `&str`, a slice view, a copy aggregate value, or an owned aggregate value with
  runtime-supported recursive drop glue
- active payload cleanup for payload-carrying enum values whose droppable
  variants have runtime-supported recursive aggregate drop trees, covering
  scope-end cleanup, parameter cleanup, discarded call results, call-result
  bindings, whole-local replacement, and multi-field payload cleanup in reverse
  aggregate field order
- aggregate call arguments crossing register and stack argument boundaries
- aggregate returns from literals, local slots, member copies, calls, explicit
  moves, and terminal branches in the supported subset
- copy aggregate fields loaded from non-copy owners when the field type is
  `copy struct`
- literal initialization and replacement of storage-only integer aggregate
  fields, including signed minimum values
- drop-aware whole-binding and field replacement in supported paths
- optional and fallible aggregate call results in supported positions
- copy aggregate storage and slice indexing through current `Vec<T>` and
  slice-view paths

Remaining aggregate backend work is concentrated around payload-carrying enum
move-only aggregate payload bindings with unsupported recursive drop trees,
broader payload enum pattern target expressions, broader field-level live-state tracking,
non-copy aggregate collection elements, arrays, broad control-flow joins, and
unsupported expression shapes.

## Safety Checks

Runtime checks are part of v0 behavior, not debug-only instrumentation.

The backend emits traps for supported cases such as:

- integer overflow where v0 arithmetic requires trapping
- division or remainder by zero
- invalid signed division overflow
- shift counts outside the type width
- out-of-bounds slice and string indexing
- forced unwrap of absent optional or failed fallible values
- explicit unreachable paths

Future optimizations may remove a check only when the compiler proves the trap
condition impossible.

## Backend Non-Goals For v0

- external linker integration
- LLVM or assembly output as the normal backend path
- C ABI compatibility
- dynamic linking
- debug info
- register allocation optimization
- broad tail-call lowering
- Linux, Windows, wasm, or x86_64 output
- stable public binary ABI

## Tests

Backend changes should use the narrowest useful tests:

- backend unit tests for instruction encodings, frame layout, call emission, and
  Mach-O invariants
- IR lowering tests for expected instruction shapes
- CLI build/run tests for user-visible runtime behavior
- distributed-home tests for std-backed runtime behavior

ABI behavior changes must also update
[ABI and Layout](../../spec/09-abi-layout.md) and
[v0-closure.md](v0-closure.md).
