# Native Testing

## CI Result Contract

`nocter test --format json` emits one `nocter.tests` version-1 envelope. It separates package-level
diagnostics from ordered process runs and records each target, optional native declaration name,
outcome, exit code or signal, stdout, stderr, and diagnostics. The summary contains passed and
failed counts. Success exits with status 0; any compile, runner, assertion, error, or process
failure exits with status 1; command-line misuse exits with status 2; serialization/internal
failure exits with status 3. JSON output does not use terminal color or progress text.

This chapter specifies the native source-test contract adopted in Nocter v0.5.0 Phase 2.

## Declarations

```nct
use std/testing.{assert, assert_eq_usize}
use std/vec.Vec

test vec_pushes_in_order {
    var values = Vec []
    values.push(1)
    assert(values[0] == 1)?
    assert_eq_usize(values.len(), 1)?
}
```

`test name { ... }` is a top-level declaration. Its result contract is always `void!`. It has no
visibility modifier, parameters, generic parameters, explicit return type, or callable value
identity. Test names are unique within their module. A test cannot be imported or called by source
code.

Falling through the body or executing `return` succeeds. Returning an `error`, propagating one with
`?`, trapping, or aborting fails that test run. A test declaration is omitted from ordinary
executable reachability and is selected only by the compiler-owned test plan.

## Module Visibility

A test uses ordinary module visibility; it receives no friend or privileged-import capability.

- A test declared in the module under test may use that module's private declarations.
- A test declared in another module crosses an import boundary and may use only `pub` API.
- The rule is identical inside and outside the same package.

This permits white-box tests beside an implementation and black-box tests in separate modules
without adding a second privacy system.

## Targets and Discovery

Package test targets remain explicit:

```nct
#test: {
    name: "vec-unit",
    entry: "./src/vec",
}

#test: {
    name: "vec-api",
    entry: "./tests/vec",
}
```

Only test declarations directly contained in the target's entry module are selected. Imported
modules are not scanned recursively, so a package never runs dependency or helper-module tests by
accident. Targets run in manifest declaration order; cases within a target run in source
declaration order.

Each accepted declaration is compiled with an explicit compiler-owned entry identity and launched
in its own temporary process. The compiler does not rewrite source, synthesize an AST `main`, or
select a declaration by backend name lookup. A signal or failure cannot prevent later runs. A
target-wide parse, resolution, or type error is reported with a null case identity.

## Assertions

`std/testing` exports ordinary fallible functions:

```nct
pub func assert(condition: bool): void!
pub func assert_eq_bool(actual: bool, expected: bool): void!
pub func assert_eq_i32(actual: i32, expected: i32): void!
pub func assert_eq_usize(actual: usize, expected: usize): void!
pub func assert_eq_u8(actual: u8, expected: u8): void!
pub func assert_eq_str(actual: &str, expected: &str): void!
```

Failures use `std.testing.assertion_failed` or `std.testing.not_equal`. Messages are static error
payload data and do not allocate, so reporting an assertion failure cannot itself become an
allocation failure. Assertions use normal `error` propagation and are not compiler intrinsics.
