//! Real private source through the common Machine, ARM64, image writer and native process.
use super::{TempPackage, execute_native_status};
use nocter_checking::value_access::accepted::Unit;
use nocter_declaration_lowering::access_declarations;
use nocter_runtime_contract::RuntimeAbiIdentity;
use nocter_target_program::value_access::{InstanceProgram, lifecycle};
use nocter_test_support::ValueSourceFixture;
use std::sync::Arc;

fn machine(source: &str) -> nocter_machine::MachineProgram {
    let fixture = ValueSourceFixture::new(source);
    let headers = access_declarations::Query::prepare(fixture.syntax()).unwrap();
    let unit = Arc::new(Unit::prepare(&headers.bind(fixture.syntax()).unwrap()).unwrap());
    let root = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
    let lifecycle = lifecycle::Program::prepare(instances).unwrap();
    let mir =
        nocter_mir::value_access::Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1)
            .unwrap();
    nocter_machine::value_access::lower(&mir).unwrap()
}
fn compile(source: &str) -> crate::NativeImage {
    let machine = machine(source);
    let arm = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    crate::NativeImage::from_macho(nocter_macho::MachOImage::build(&arm).unwrap())
}

#[test]
fn private_scalar_computation_and_scoped_input_calls_execute_natively() {
    for (name, source, expected) in [
        ("constant", "func run(): i32 { 42 }", 42),
        (
            "snapshot",
            "func next(edit value: i32): i32 { value += 1\nvalue }\nfunc run(): i32 { var value = 4\nvalue + next(value) }",
            9,
        ),
        (
            "const-runtime-state",
            "const func increment(edit value: i32): void { value += 1 }\nfunc run(): i32 { var left = 20\nvar right = 20\nincrement(left)\nincrement(right)\nleft + right }",
            42,
        ),
        (
            "required-const-value",
            "const func increment(edit value: i32): void { value += 1 }\nconst func calculate(): i32 { var left = 20\nvar right = 20\nincrement(left)\nincrement(right)\nleft + right }\nconst ANSWER: i32 = calculate()\nfunc run(): i32 { ANSWER }",
            42,
        ),
        (
            "constant-before-flow",
            "const func decide(): bool { false }\nconst FLAG: bool = decide()\nconst func calculate(): i32 { let value = 42\nif FLAG { drop value }\nvalue }\nconst ANSWER: i32 = calculate()\nfunc run(): i32 { let value = ANSWER\nif FLAG { drop value }\nvalue }",
            42,
        ),
        (
            "constant-before-completion",
            "const FLAG: bool = true\nfunc run(): i32 { while FLAG { return 42 } }",
            42,
        ),
        (
            "boolean-proof-effects",
            "func touch(edit count: i32): bool { count += 1\nfalse }\nfunc run(): i32 { var count = 40\nif touch(count) || true { count += 1 }\nlet _ = !true && touch(count)\ncount }",
            42,
        ),
        (
            "boolean-proof-completion",
            "func run(): i32 { let _ = true && if true { return 42 } else { false } }",
            42,
        ),
        (
            "unit-comparison-effects",
            "func first(edit count: i32): void { count = count * 10 + 1 }\nfunc second(edit count: i32): void { count = count * 10 + 2 }\nfunc run(): i32 { var count = 0\nif first(count) == second(count) { count } else { 0 } }",
            12,
        ),
        (
            "unit-comparison-inequality",
            "func run(): i32 { let value = 42\nif () != () { drop value }\nvalue }",
            42,
        ),
        (
            "const-in-capture",
            "const OFFSET: i32 = 40\nfunc run(): i32 { let value = 1\nlet action = (value;): i32 { OFFSET + value + 1 }\naction() }",
            42,
        ),
        (
            "remainder-result-domain",
            "func remainder(value: i32, divisor: i32): i32 { value % divisor }\nfunc run(): i32 { remainder(-2147483648, -1) }",
            0,
        ),
        (
            "negative",
            "func negate(value: i32): i32 { -value }\nfunc run(): i32 { negate(-7) }",
            7,
        ),
        ("unit", "func run(): void {}", 0),
        (
            "interrupted",
            "func forward(take action: func(i32): i32): func(i32): i32 { action^ }\nfunc run(): i32 { let absent = forward(if true { return 1 } else { return 2 })\nlet adapted: once(i32): i32 = absent\nadapted(1) }",
            1,
        ),
    ] {
        let package = TempPackage::new();
        let image = compile(source);
        execute_native_status(&image, &package.0, name, expected);
    }
}

#[test]
fn private_indirect_aggregate_inputs_and_results_execute_natively() {
    let source = "struct Wide { a: i32\nb: i32\nc: i32\nd: i32\ne: i32 }\nfunc forward(take value: Wide): Wide { value^ }\nfunc run(): i32 { let value = Wide { a: 1, b: 2, c: 3, d: 4, e: 42 }\nlet next = forward(value^)\nnext.e }";
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "wide", 42);
}

#[test]
fn private_callbacks_and_mutable_captures_use_the_same_native_call_path() {
    for (name, source, expected) in [
        (
            "forward",
            "func increment(value: i32): i32 { value + 1 }\nfunc apply(action: func(i32): i32): i32 { action(1) }\nfunc forward(action: func(i32): i32): i32 { apply(action) }\nfunc run(): i32 { forward(increment) }",
            2,
        ),
        (
            "adapter",
            "func increment(value: i32): i32 { value + 1 }\nfunc apply(take action: once(i32): i32): i32 { action^(1) }\nfunc forward(take action: func(i32): i32): i32 { apply(action^) }\nfunc run(): i32 { forward(increment) }",
            2,
        ),
        (
            "capture",
            "func run(): i32 { let count = 40\nvar action = (count;): i32 { count += 1\ncount }\nlet first = action()\nlet second = action()\nfirst + second }",
            83,
        ),
        (
            "capture-once",
            "func apply(take action: once(): i32): i32 { action^() }\nfunc run(): i32 { let count = 41\nlet action = (count;): i32 { count + 1 }\napply(action) }",
            42,
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, expected);
    }
}

#[test]
fn private_loops_short_circuits_and_partial_values_execute_natively() {
    for (name, source, expected) in [
        (
            "loop",
            "func run(): i32 { var value = 0\nwhile value < 42 { value += 1 }\nvalue }",
            42,
        ),
        (
            "short",
            "func touch(edit value: i32): bool { value += 1\ntrue }\nfunc run(): i32 { var value = 41\nlet first = false && touch(value)\nlet second = true || touch(value)\nif first || !second { return 1 }\nvalue }",
            41,
        ),
        (
            "partial",
            "struct Pair { first: i32\nsecond: i32 }\nfunc choose(): bool { true }\nfunc run(): i32 { let pair = Pair { first: 1, second: 42 }\nif choose() { let moved = pair.first^ }\npair.second }",
            42,
        ),
        (
            "temporary",
            "func touch(edit value: i32): void { value += 1 }\nfunc run(): i32 { touch(1 + 2)\n42 }",
            42,
        ),
        (
            "retired",
            "func observe(value: i32): i32 { value + 1 }\nfunc run(): i32 { let value = 41\nobserve(value^) }",
            42,
        ),
        (
            "aliases",
            "func run(): i32 { var count = 1\nread source = count\nlet snapshot = source\nedit target = count\ntarget += 2\nsnapshot + target }",
            4,
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, expected);
    }
}

#[test]
fn private_address_channels_cross_register_and_stack_argument_boundaries() {
    let source = "func sum(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, i: i32, j: i32): i32 { a + b + c + d + e + f + g + h + i + j }\nfunc run(): i32 { sum(1, 2, 3, 4, 5, 6, 7, 8, 9, 10) }";
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "ten-inputs", 55);
}

#[test]
fn private_zero_byte_values_keep_addressable_scoped_channels() {
    let source = "struct Empty {}\nfunc inspect(input: Empty): i32 { 42 }\nfunc run(): i32 { let value = Empty {}\ninspect(value) }";
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "zero", 42);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn private_required_arithmetic_checks_reach_native_traps() {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;

    for (name, source) in [
        (
            "overflow",
            "func add(a: i32, b: i32): i32 { a + b }\nfunc run(): i32 { let unused = add(2147483647, 1)\n0 }",
        ),
        (
            "boolean-proof-trap",
            "func fail(): bool { let _ = 1 / 0\nfalse }\nfunc run(): i32 { if fail() || true { return 42 } else { return 0 } }",
        ),
        (
            "unit-comparison-trap",
            "func fail(): void { let _ = 1 / 0 }\nfunc run(): i32 { if fail() == () { return 42 } else { return 0 } }",
        ),
        (
            "remainder-zero",
            "func remainder(value: i32, divisor: i32): i32 { value % divisor }\nfunc run(): i32 { let unused = remainder(42, 0)\n0 }",
        ),
        (
            "divide-zero",
            "func divide(a: i32, b: i32): i32 { a / b }\nfunc run(): i32 { let unused = divide(42, 0)\n0 }",
        ),
    ] {
        let package = TempPackage::new();
        let executable = package.0.join(name);
        std::fs::write(&executable, compile(source).bytes()).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let status = Command::new(&executable)
            .current_dir(&package.0)
            .status()
            .unwrap();
        // Darwin reports the compiler-emitted `brk` instruction as SIGTRAP.
        assert_eq!(
            status.signal(),
            Some(5),
            "native image ended with {status:?}"
        );
    }
}

#[test]
fn private_record_constants_are_serialized_and_each_use_receives_an_independent_value() {
    let source = "struct Data { flag: bool\nvalue: i32\nunit: void }\nconst func initialize(): Data { var value = 40\nvalue += 2\nData { value: value, unit: (), flag: true } }\nconst DATA: Data = initialize()\nfunc run(): i32 { var first = DATA\nlet second = DATA\nfirst.value = 0\nif second.flag { first.value + second.value } else { 1 } }";
    let program = machine(source);
    assert_eq!(program.data().len(), 1);
    let (_, data) = program.data().iter().next().unwrap();
    assert_eq!(data.bytes(), &[1, 0, 0, 0, 42, 0, 0, 0]);
    assert_eq!(data.alignment(), 4);
    assert!(data.relocations().is_empty());
    // Only the ordinary entry and process wrapper exist: the initializer is compile-time only.
    assert_eq!(program.functions().len(), 2);
    let arm = nocter_arm64::Arm64Program::lower_machine(&program).unwrap();
    let image = crate::NativeImage::from_macho(nocter_macho::MachOImage::build(&arm).unwrap());
    let package = TempPackage::new();
    execute_native_status(&image, &package.0, "record-constant", 42);
    for (name, source, expected) in [
        (
            "wide-constant",
            "struct Wide { a: i32\nb: i32\nc: i32\nd: i32\ne: i32 }\nconst DATA: Wide = Wide { a: 1, b: 2, c: 3, d: 4, e: 42 }\nfunc forward(take value: Wide): Wide { value^ }\nfunc run(): i32 { let value = forward(DATA)\nvalue.e }",
            42,
        ),
        (
            "empty-constant",
            "struct Empty {}\nconst DATA: Empty = Empty {}\nfunc inspect(value: Empty): i32 { 42 }\nfunc run(): i32 { inspect(DATA) }",
            42,
        ),
        (
            "nested-constant",
            "struct Inner { value: i32 }\nstruct Outer { value: Inner\nflag: bool }\nconst DATA: Outer = Outer { value: Inner { value: -42 }, flag: true }\nfunc run(): i32 { let value = DATA.value\n-value.value }",
            42,
        ),
    ] {
        execute_native_status(&compile(source), &package.0, name, expected);
    }
}
