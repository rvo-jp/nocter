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
