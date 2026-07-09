# Backend V0

The current native backend targets `arm64-darwin` and writes Mach-O executables directly.
It does not invoke LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or an external linker.

## Pipeline

`nocter build` runs the native backend path end to end:

```text
SourceMap
    -> lexer/parser
    -> import resolution
    -> type checking
    -> IR lowering
    -> ARM64 Darwin machine code
    -> Mach-O executable image
    -> executable file
```

## Register Convention

The backend v0 uses a deliberately small register-only convention while the IR has no stack frame, spill slots, or ABI-complete call lowering.

- scalar `i32` and `bool` values are represented in 32-bit ARM64 `w` registers
- `bool` is encoded as `0` for false and `1` for true
- lowered function arguments are passed in `w0` through `w7`
- lowered function return values are produced in `w0`
- scalar local bindings use `w9` through `w15` across both `i32` and `bool`
- `w16` and `w17` are backend scratch registers and may be clobbered by code generation

Tail calls are lowered by loading the callee arguments into `w0` through `w7` and branching directly to the target function.
Non-tail calls are intentionally not buildable yet.
A normal call would return to the caller after clobbering argument, return, and scratch registers; with the current register-only local model, that can destroy live values in later expressions.

Before adding non-tail calls, add stack slots, spill/reload support, and a clear caller/callee preservation rule.

## Normal Call Lowering Design

Normal calls are the next backend boundary to design before implementation.
They must not be added by treating `bl` as a drop-in replacement for the existing tail-call `b`.
On ARM64, `bl` writes the caller return address into `x30`.
If a Nocter function executes `bl` and later executes `ret`, the original `x30` value must be restored first.
The current register-only local model also keeps scalar locals in caller-clobbered registers, so a callee may overwrite values that remain live after the call.

The first implementation should keep the user-visible subset small:

- same-file, non-generic calls only
- `i32` arguments and `i32` return values only
- calls used in lowerable `i32` expressions and `let` initializers
- up to 8 arguments, passed in `w0` through `w7`
- no imported calls, aggregate arguments, aggregate returns, strings, optionals, ownership/drop lowering, or calls in general control-flow conditions

### Frame Shape

Functions that contain no normal call can continue using the existing frameless code path.
Functions that contain at least one normal call need a stack frame.

The v0 frame should be fixed-size and 16-byte aligned:

```text
high addresses

sp + frame_size - 8    saved x30
...                    reserved padding for 16-byte alignment
sp + spill_offset(n)   scalar spill slots for caller-live locals
sp                     bottom of frame after prologue

low addresses
```

The initial implementation does not need a frame pointer.
Using `sp`-relative addressing keeps the prologue small and avoids introducing `x29` until diagnostics, debugging metadata, variable-sized frames, or external ABI interop require it.
If a frame pointer is later added, it should be a separate mechanical change.

The prologue for a framed function is:

```text
sub sp, sp, #frame_size
str x30, [sp, #saved_x30_offset]
```

The epilogue before every non-tail `ret` is:

```text
ldr x30, [sp, #saved_x30_offset]
add sp, sp, #frame_size
ret
```

Tail calls from framed functions must deallocate the frame and restore `x30` before branching to the callee:

```text
ldr x30, [sp, #saved_x30_offset]
add sp, sp, #frame_size
b callee
```

The process entry stub can remain special-purpose: it calls the selected entry with `bl`, then exits by syscall instead of returning to another Nocter caller.

### Spill And Reload Rule

For v0, keep local allocation simple and conservative.
The existing `w9` through `w15` scalar locals may remain as the fast local representation, but every normal call site must spill all currently defined scalar locals before the call and reload them after the call.

This is intentionally broader than liveness requires.
It is correct for the current straight-line and terminal-if subset, avoids building a dataflow liveness pass too early, and gives later work a stable place to replace conservative spilling with live-range-aware spilling.

Call-site lowering should use this shape:

```text
str w9,  [sp, #slot_for_local_0]
str w10, [sp, #slot_for_local_1]
...
mov/evaluate argument 0 into w0
mov/evaluate argument 1 into w1
...
bl callee
mov w16, w0                 ; optional if the result must survive reloads
ldr w9,  [sp, #slot_for_local_0]
ldr w10, [sp, #slot_for_local_1]
...
mov destination, w16/w0
```

Use a temporary result register when the call result is not written directly to its final destination after reload.
For example, `let x = callee()` can reload locals and then write `w0` into `x` if the destination local was not among the spilled pre-call locals.
For `callee() + local`, preserve the call result in a scratch or spill slot before reloading locals, then perform the addition.

The first implementation should not try to support nested call arguments.
Reject or avoid lowering `outer(inner())` until call arguments can consume staged call results.

### Argument Movement

Tail calls currently reject reordered parameter arguments because sequential moves into `w0` through `w7` can clobber later source parameters.
Normal calls use argument staging slots:

- evaluate each lowerable `i32` argument into a stack slot or scratch register
- after all arguments are staged, load `w0` through `w7` from those staged values
- issue `bl`

This makes source normal calls such as `swap(b, a)` safe when `a` and `b` already live in argument registers.
The same relaxation does not apply to tail calls yet; tail calls still use direct sequential argument moves and must continue rejecting reordered parameter arguments until they also gain staging or parallel move support.

### IR Shape

The IR should distinguish tail calls from normal calls.
Do not overload `Instruction::TailCall`.

A minimal extension is:

```text
CallI32 {
    destination: I32Location,
    function: String,
    arguments: Vec<I32Value>,
}
```

This covers `let x = callee()` and `return callee() + 1` after the expression lowerer has a destination for intermediate results.
The source expression lowerer stages normal-call results in temporary scalar locals for `i32` additions.
Multiple normal calls in an addition are evaluated left to right and receive distinct temporary locals.

Bool-returning normal calls should wait until the i32 path is stable.
Imported calls should also wait because they need symbol/linkage policy, not just call sequence support.

### Encoder Work Required First

The ARM64 encoder has focused helpers and unit tests for the first frame/spill instructions:

- `sub sp, sp, #imm`
- `add sp, sp, #imm`
- `str x30, [sp, #imm]`
- `ldr x30, [sp, #imm]`
- `str wN, [sp, #imm]` for scalar local spills
- `ldr wN, [sp, #imm]` for scalar local reloads

Keep immediates constrained to the encodable forms used by v0 frames.
If a frame grows beyond those immediate ranges, report a backend diagnostic instead of introducing unplanned large-immediate materialization.

### Implementation Order

Implement normal calls in this order:

1. Done: add ARM64 stack/load/store encoder helpers with unit tests.
2. Done: add a backend frame planner that computes fixed frame size, saved `x30` offset, and scalar spill slot offsets.
3. Done: emit framed-function prologue/epilogue and route `Return` and `TailCall` emission through frame-aware helpers so framed functions restore `x30` and deallocate their frame before exiting.
4. Done: add the IR normal-call instruction without lowering source calls to it yet; add codegen unit tests for hand-built IR functions that perform normal calls.
5. Done: lower the smallest source subset: same-file `i32` normal call in a `let` initializer or simple i32 return addition, with CLI build/run coverage.
6. Done: add normal-call argument staging slots and allow reordered parameter arguments for source normal calls.
7. Done: make one-call `i32` addition result staging explicit, including `let` initializers and nested additions that contain a single normal call.
8. Done: add multiple temporary allocation and left-to-right evaluation for `i32` additions with multiple normal calls.
9. Expand to nested call arguments only after call arguments can consume staged call results.

### Non-Goals For This Phase

Do not combine normal-call work with:

- stack-backed `var` and reassignment
- general loops or non-terminal control flow
- imported calls or external linking
- aggregate layout or aggregate calling convention
- ownership/drop lowering
- ABI-complete Darwin interop

Those features all need the same frame foundation, but each adds a separate semantic or ABI decision.
