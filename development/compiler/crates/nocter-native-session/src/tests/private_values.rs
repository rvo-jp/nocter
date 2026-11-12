//! Real private source through the common Machine, ARM64, image writer and native process.
use super::{TempPackage, execute_native_signal, execute_native_status};
use nocter_checking::value_access::accepted::SourceUnit;
use nocter_runtime_contract::RuntimeAbiIdentity;
use nocter_target_program::value_access::{InstanceProgram, lifecycle};
use nocter_test_support::ValueSourceFixture;
use std::sync::Arc;

fn machine(source: &str) -> nocter_machine::MachineProgram {
    let fixture = ValueSourceFixture::new(source);
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let root = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
    let lifecycle = lifecycle::Program::prepare(instances).unwrap();
    let mir =
        nocter_mir::value_access::Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1)
            .unwrap();
    nocter_machine::value_access::lower(&mir)
        .unwrap_or_else(|failure| panic!("machine lowering failed for {source:?}: {failure:?}"))
}

#[test]
fn private_structural_result_values_close_payloads_before_native_execution() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/structural-result-values.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "structural-result-values", 42);
}

#[test]
fn private_result_owned_captures_close_nested_environments_and_source_operations() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/result-owned-captures.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "result-owned-captures", 42);
}
fn compile(source: &str) -> crate::NativeImage {
    let machine = machine(source);
    let arm = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    crate::NativeImage::from_macho(nocter_macho::MachOImage::build(&arm).unwrap())
}

#[test]
fn private_callable_value_captures_preserve_views_state_and_recursive_cleanup() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/callable-value-captures.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "callable-value-captures", 42);
}

#[test]
fn private_nested_captures_keep_distinct_activations_and_invocation_demands() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/nested-closed-captures.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "nested-closed-captures", 42);
}

#[test]
fn private_closure_factory_applications_keep_their_lexical_code_and_environment() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/closure-result-applications.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "closure-result-applications",
        42,
    );
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
            "early-value-late-headers",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/native/value-access/early-value-late-headers.nct"
            )),
            42,
        ),
        (
            "late-header-replay",
            "func run(): i32 { let pair = (40, true)\nlet action = (pair;): i32 { if pair.1 { pair.0 + 2 } else { 0 } }\naction() }\nfunc z_unused(action: func(): i32): i32 { action() }",
            42,
        ),
        (
            "late-record-replay",
            "struct Late { pair: (bool, bool) }\nfunc run(): i32 { let pair = (40, true)\nlet action = (pair;): i32 { if pair.1 { pair.0 + 2 } else { 0 } }\naction() }\nfunc z_unused(): i32 { let data: Late = Late { pair: (true, false) }\nif data.pair.0 { 1 } else { 0 } }",
            42,
        ),
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
        (
            "alias-replacement",
            "func run(): i32 { var count = 0\nedit target = count\ntarget = 42\ncount }",
            42,
        ),
        (
            "alias-record-replacement",
            "struct Pair { left: i32\nright: i32 }\nfunc run(): i32 { var pair = Pair { left: 1, right: 2 }\nedit target = pair\ntarget = Pair { left: 20, right: 22 }\npair.left + pair.right }",
            42,
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
    for (name, source) in [
        (
            "usize-overflow",
            "func run(): i32 { let value: usize = 18446744073709551615\nlet unused = value + 1\n0 }",
        ),
        (
            "usize-underflow",
            "func run(): i32 { let value: usize = 0\nlet unused = value - 1\n0 }",
        ),
        (
            "usize-multiply-overflow",
            "func run(): i32 { let value: usize = 18446744073709551615\nlet unused = value * 2\n0 }",
        ),
        (
            "usize-negate",
            "func run(): i32 { let value: usize = 1\nlet unused = -value\n0 }",
        ),
        (
            "usize-source-order",
            "func run(): i32 { let value: usize = 0\nlet unused = (18446744073709551615 + 1) + if true { return 42 } else { value }\n0 }",
        ),
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
        // Darwin reports the compiler-emitted `brk` instruction as SIGTRAP.
        execute_native_signal(&compile(source), &package.0, name, 5);
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
        (
            "selected-constant-evaluation",
            "struct Inner { value: i32 }\nstruct Outer { ignored: (bool, i32)\nselected: Inner }\nconst DATA: Outer = Outer { ignored: (false, 7), selected: Inner { value: 40 } }\nconst func change(take value: Inner): Inner { value.value += 2\nvalue^ }\nconst ANSWER: Inner = change(DATA.selected)\nfunc run(): i32 { if DATA.selected.value == 40 { ANSWER.value } else { 0 } }",
            42,
        ),
    ] {
        execute_native_status(&compile(source), &package.0, name, expected);
    }
}

#[test]
fn private_immutable_statics_share_places_without_initializer_calls_or_hidden_inputs() {
    let source = "const func initialize(): i32 { 40 + 2 }\nstatic FIRST: i32 = initialize()\nstatic SECOND: i32 = 42\nfunc first(): i32 { read value = FIRST\nvalue }\nfunc second(): i32 { FIRST + SECOND }\nfunc run(): i32 { first() + second() }";
    let program = machine(source);
    assert_eq!(program.data().len(), 2);
    assert!(
        program
            .data()
            .iter()
            .all(|(_, data)| data.bytes() == [42, 0, 0, 0])
    );
    assert_eq!(program.functions().len(), 4);
    assert!(
        program
            .functions()
            .all(|(_, function)| match function.kind() {
                nocter_machine::MachineFunctionKind::Callable(abi) => abi.arguments().is_empty(),
                _ => true,
            })
    );
    let arm = nocter_arm64::Arm64Program::lower_machine(&program).unwrap();
    let image = crate::NativeImage::from_macho(nocter_macho::MachOImage::build(&arm).unwrap());
    let package = TempPackage::new();
    execute_native_status(&image, &package.0, "static-identity", 126);
    for (name, source) in [
        (
            "static-noncopy",
            "struct Pair { left: i32\nright: i32 }\nstatic DATA: Pair = Pair { left: 40, right: 2 }\nfunc inspect(value: Pair): i32 { value.left + value.right }\nfunc run(): i32 { read value = DATA\ninspect(value) }",
        ),
        (
            "static-copy",
            "copy struct Pair { value: i32 }\nstatic DATA: Pair = Pair { value: 42 }\nfunc run(): i32 { var value = DATA\nvalue.value = 0\nDATA.value }",
        ),
        (
            "static-empty",
            "struct Empty {}\nstatic FIRST: Empty = Empty {}\nstatic SECOND: Empty = Empty {}\nfunc inspect(first: Empty, second: Empty): i32 { 42 }\nfunc run(): i32 { inspect(FIRST, SECOND) }",
        ),
        (
            "static-capture",
            "static BASE: i32 = 40\nfunc run(): i32 { let value = 2\nlet action = (value;): i32 { BASE + value }\naction() }",
        ),
    ] {
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}

#[test]
fn private_structural_tuples_share_native_calls_constants_and_static_places() {
    for (name, source) in [
        (
            "tuple-interrupted-component",
            "func run(): i32 { let absent: (i32, bool) = (if true { return 42 } else { return 43 }, false) }",
        ),
        (
            "tuple-callback-contract",
            "func adjust(take pair: (i32, i32)): (i32, i32) { pair.0 += 1\npair^ }\nfunc relay(take action: func(take (i32, i32)): (i32, i32), take input: (i32, i32)): (i32, i32) { action(input^) }\nfunc run(): i32 { let pair = (20, 21)\nlet next = relay(adjust, pair^)\nnext.0 + next.1 }",
        ),
        (
            "tuple-inferred-snapshot",
            "func increment(edit value: i32): i32 { value += 1\nvalue }\nfunc run(): i32 { var value = 20\nlet pair = (value, increment(value))\npair.0 + pair.1 + 1 }",
        ),
        (
            "tuple-record-composition",
            "copy struct Wrapped { pair: (i32, (bool, i32)) }\nfunc run(): i32 { var value = Wrapped { pair: (20, (true, 22)) }\nlet snapshot = value\nvalue.pair.0 = 0\nif snapshot.pair.1.0 { snapshot.pair.0 + snapshot.pair.1.1 } else { 0 } }",
        ),
        (
            "tuple-edit",
            "func increment(edit value: i32): void { value += 1 }\nfunc run(): i32 { var pair = (20, 21)\nincrement(pair.0)\npair.0 + pair.1 }",
        ),
        (
            "tuple-mixed-layout",
            "func build(): (bool, void, i32, void) { (true, (), 42, ()) }\nfunc run(): i32 { let value = build()\nif value.0 { value.2 } else { 0 } }",
        ),
        (
            "tuple-record-transfer",
            "struct Count { value: i32 }\nfunc add(take pair: (Count, Count)): i32 { pair.0.value + pair.1.value }\nfunc run(): i32 { let pair = (Count { value: 20 }, Count { value: 22 })\nadd(pair^) }",
        ),
        (
            "tuple-indirect-result",
            "func build(): (i32, i32, i32, i32, i32, i32) { (20, 1, 2, 3, 4, 22) }\nfunc run(): i32 { let values = build()\nvalues.0 + values.5 }",
        ),
        (
            "tuple-frozen-data",
            "struct Count { value: i32 }\nconst DATA: (Count, (bool, i32)) = (Count { value: 40 }, (true, 2))\nstatic BASE: (Count, i32) = (Count { value: 40 }, 2)\nfunc run(): i32 { var own = DATA\nown.0.value = 0\nread base = BASE.0\nif DATA.1.0 { base.value + BASE.1 + own.0.value } else { 0 } }",
        ),
        (
            "tuple-capture-local-types",
            "func other(): i32 { let unrelated = (true, ())\n0 }\nfunc run(): i32 { let captured = (true, 40)\nlet action = (captured;): i32 { let nested = (captured, (1, 1))\nif nested.0.0 { nested.0.1 + nested.1.0 + nested.1.1 } else { 0 } }\naction() }",
        ),
        (
            "tuple-zero-byte-channels",
            "func assign(edit left: void, edit right: void): i32 { left = ()\nright = ()\n42 }\nfunc run(): i32 { var pair = ((), ())\nassign(pair.0, pair.1) }",
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}

#[test]
fn private_size_integers_preserve_unsigned_arithmetic_literals_and_data() {
    for (name, source) in [
        (
            "usize-high-domain",
            "func run(): i32 { let maximum: usize = 18446744073709551615\nlet high: usize = 9223372036854775808\nif maximum > high && high > 0 && maximum / high == 1 && maximum % high == 9223372036854775807 { 42 } else { 0 } }",
        ),
        (
            "usize-literal-context",
            "func receive(value: usize): usize { value }\nfunc run(): i32 { let value: usize = 40\nif 1 + (1 + value) == receive(40 + 2) && (1 + 1) + value == 42 { 42 } else { 0 } }",
        ),
        (
            "usize-input-snapshots",
            "func increment(edit value: usize): usize { value += 1\nvalue }\nfunc run(): i32 { var value: usize = 20\nlet total = value + increment(value) + 1\nif total == 42 { 42 } else { 0 } }",
        ),
        (
            "usize-static-data",
            "copy struct Size { value: usize }\nstatic MAX: Size = Size { value: 18446744073709551615 }\nconst HIGH: (usize, usize) = (9223372036854775808, 9223372036854775807)\nfunc run(): i32 { if MAX.value == HIGH.0 + HIGH.1 { 42 } else { 0 } }",
        ),
        (
            "usize-const-loop",
            "const func width(): usize { var value: usize = 1\nvar count: usize = 0\nwhile count < 6 { value *= 2\ncount += 1 }\nvalue }\nconst WIDTH: usize = width()\nfunc run(): i32 { if WIDTH == 64 { 42 } else { 0 } }",
        ),
        (
            "usize-zero-negation",
            "func run(): i32 { let value: usize = 0\nif -value == 0 { 42 } else { 0 } }",
        ),
    ] {
        let package = TempPackage::new();
        execute_native_status(&compile(source), &package.0, name, 42);
    }
}

mod arrays;

#[test]
fn private_const_named_values_use_the_same_runtime_body_and_embedded_answer() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/const-named-values.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "const-named-values", 42);
}

#[test]
fn private_const_captured_values_share_runtime_views_and_environment_semantics() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/const-captured-values.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "const-captured-values", 42);
}

#[test]
fn private_owned_environments_execute_empty_and_multiple_captures() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/owned-environments.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "owned-environments", 42);
}

#[test]
fn private_contextual_closures_keep_selected_contracts_in_runtime_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-closures.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "contextual-closures", 42);
}

#[test]
fn private_direct_indexed_callables_keep_retirement_indices_and_snapshots() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/direct-indexed-callables.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "direct-indexed-callables", 42);
}

#[test]
fn private_adapted_callable_element_transfers_share_prefix_and_cancellation_phases() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/adapted-element-transfers.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "adapted-element-transfers",
        42,
    );
}

#[test]
fn private_integer_catalog_preserves_boundaries_narrow_storage_and_abi() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/integer-values.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "integer-values", 42);
}

#[test]
fn private_integer_catalog_failures_preserve_native_traps_and_evaluation_order() {
    let package = TempPackage::new();
    for (name, source) in nocter_test_support::integer_failure_sources() {
        execute_native_signal(&compile(&source), &package.0, &name, 5);
    }
}

#[test]
fn private_optional_values_preserve_layers_source_supply_storage_and_abi() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-values.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "optional-values", 42);
}
#[test]
fn private_optional_absence_assertions_trap_before_payload_reads_and_without_unwind() {
    let package = TempPackage::new();
    for (name, source) in [
        (
            "absent",
            "func run(): i32 { let value: i32? = none\nvalue! }",
        ),
        (
            "forwarded-absent",
            "func same(value: i32?): i32? { value }\nfunc run(): i32 { same(none)! }",
        ),
        (
            "nested-absent",
            "func lift(value: i32?): i32?? { value }\nfunc run(): i32 { (lift(none)!)! }",
        ),
        (
            "error-absent",
            "func run(): i32 { let value: error? = none\nlet unused = value^!\n42 }",
        ),
        (
            "absent-before-exit",
            "func receive(value: i32, ignored: i32): i32 { value }\nfunc run(): i32 { let value: i32? = none\nreceive(value!, if true { return 42 } else { 0 }) }",
        ),
        (
            "unit-absent",
            "func run(): i32 { let value: void? = none\nlet _ = value!\n42 }",
        ),
        (
            "scoped-nested-propagated-none",
            "func absent(): i32?? { let inner: i32? = none\nlet value: i32?? = inner\nread payload = (value?)?\npayload }\nfunc run(): i32 { let inner = absent()!\n42 }",
        ),
        (
            "scoped-unit-propagated-none",
            "func absent(): void? { var value: i32? = none\nedit payload = value?\npayload = 42 }\nfunc run(): i32 { let _ = absent()!\n42 }",
        ),
        (
            "scoped-absence-before-later-exit",
            "func observe(value: i32, other: i32): i32 { value }\nfunc absent(): i32? { let value: i32? = none\nobserve(value?, if true { return 42 } else { 0 }) }\nfunc run(): i32 { absent()! }",
        ),
        (
            "scoped-empty-optional-array",
            "func observe(value: error): void {}\nfunc run(): i32 { let values: [error?; 0] = []\nobserve(values[0]!)\n42 }",
        ),
        (
            "scoped-bounds-before-exit",
            "func observe(value: i32, other: i32): i32 { value }\nfunc run(): i32 { let values: [i32?; 1] = [none]\nobserve(values[1]!, if true { return 42 } else { 0 }) }",
        ),
    ] {
        execute_native_signal(&compile(source), &package.0, name, 5);
    }
}

#[test]
fn private_deep_optional_absence_has_finite_layout_and_no_inactive_payload_cleanup() {
    let source = nocter_test_support::deep_optional_source(4_096);
    let package = TempPackage::new();
    execute_native_status(&compile(&source), &package.0, "deep-optional", 42);
}

#[test]
fn private_optional_propagation_preserves_carriers_layers_and_call_cancellation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-propagation.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "optional-propagation", 42);
}

#[test]
fn private_optional_places_edit_existing_payloads_with_stable_tags_and_saved_indices() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-places.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(&compile(source), &package.0, "optional-places", 42);
}

#[test]
fn private_optional_place_propagation_keeps_sources_tags_and_cancellation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/optional-place-propagation.nct"
    ));
    let package = TempPackage::new();
    execute_native_status(
        &compile(source),
        &package.0,
        "optional-place-propagation",
        42,
    );
}
