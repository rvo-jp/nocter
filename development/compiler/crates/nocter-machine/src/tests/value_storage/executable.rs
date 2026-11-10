use super::*;
use crate::{
    MachineFunctionKind, MachineOperationKind, MachineProgram, MachineValueCarrier,
    MachineValueRepresentation,
};
use nocter_target_program::value_access::lifecycle;

pub(super) fn compile(source: &str) -> MachineProgram {
    lower(source).unwrap()
}

fn lower(source: &str) -> Result<MachineProgram, crate::value_access::ExecutableFailure> {
    let instances = Arc::new(close(&ValueSourceFixture::new(source)));
    let lifecycle = lifecycle::Program::prepare(instances).unwrap();
    let mir =
        nocter_mir::value_access::Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1)
            .unwrap();
    crate::value_access::lower(&mir)
}

#[test]
fn process_roots_reject_inputs_and_non_exit_results() {
    for source in [
        "func run(input: i32): i32 { input }",
        "func run(): bool { true }",
    ] {
        assert!(matches!(
            lower(source),
            Err(crate::value_access::ExecutableFailure::InvalidRoot)
        ));
    }
}

#[test]
fn unused_runtime_types_do_not_require_cleanup_layouts() {
    let program = compile("struct Unused { failure: error }\nfunc run(): i32 { 42 }");
    assert_eq!(program.functions().len(), 2);
    assert!(
        !program
            .functions()
            .flat_map(|(_, f)| f.body().operations())
            .any(|(_, op)| matches!(op.kind(), MachineOperationKind::ReleaseError { .. }))
    );
}

#[test]
fn scoped_inputs_use_pointer_carriers_without_borrow_data_types() {
    let program = compile(
        "func increment(edit value: i32): void { value += 1 }\nfunc run(): i32 { var value = 41\nincrement(value)\nvalue }",
    );
    let callee = program.functions().find(|(_, f)| matches!(f.kind(), MachineFunctionKind::Callable(abi) if abi.arguments().len() == 1)).unwrap().1;
    let parameter = callee.body().stack(callee.body().parameters()[0]).unwrap();
    assert_eq!(parameter.carrier(), MachineValueCarrier::Address);
    assert_eq!(parameter.size(), program.layouts().target().pointer_size());
    assert_eq!(program.layouts().get(parameter.ty()).unwrap().size(), 4);
    assert!(
        program
            .functions()
            .flat_map(|(_, f)| f.body().values())
            .any(|(_, value)| value.carrier() == MachineValueCarrier::Address
                && matches!(
                    value.representation(),
                    MachineValueRepresentation::Stored { size: 8, .. }
                ))
    );
    assert!(program.functions().all(|(_, f)| {
        !f.body()
            .blocks()
            .any(|(_, block)| matches!(block.terminator(), crate::MachineTerminator::Unreachable))
    }));
}

#[test]
fn wide_owned_inputs_and_results_use_the_common_indirect_abi() {
    let program = compile(
        "struct Wide { a: i32\nb: i32\nc: i32\nd: i32\ne: i32 }\nfunc forward(take value: Wide): Wide { value^ }\nfunc run(): i32 { let value = Wide { a: 1, b: 2, c: 3, d: 4, e: 42 }\nlet next = forward(value^)\nnext.e }",
    );
    assert!(program.functions().any(|(_, f)| matches!(f.kind(), MachineFunctionKind::Callable(abi) if abi.arguments().first().is_some_and(|a| a.class() == MachineValueClass::Indirect) && matches!(abi.result(), crate::MachineResultAbi::Value(r) if r.class() == MachineValueClass::Indirect))));
}

#[test]
fn record_cleanup_helpers_share_schemas_instead_of_expanding_nested_field_paths() {
    let mut source = String::from("struct Leaf { value: error }\n");
    let mut previous = String::from("Leaf");
    for depth in 0..12 {
        let next = format!("Layer{depth}");
        writeln!(
            source,
            "struct {next} {{ first: {previous}\nsecond: {previous} }}"
        )
        .unwrap();
        previous = next;
    }
    write!(
        source,
        "func make(): {previous} {{ loop {{}} }}\nfunc run(): void {{ let value = make() }}"
    )
    .unwrap();
    let program = compile(&source);
    // Source make/run, a process root, and one helper per record/error shape.
    assert_eq!(program.functions().len(), 17);
    assert_eq!(
        program
            .functions()
            .flat_map(|(_, f)| f.body().operations())
            .filter(|(_, op)| matches!(op.kind(), MachineOperationKind::ReleaseError { .. }))
            .count(),
        1
    );
}

fn shared_constant(depth: usize, empty: bool) -> String {
    use std::fmt::Write as _;
    let mut source = if empty {
        "copy struct S0 {}\n".to_owned()
    } else {
        "copy struct S0 { value: i32 }\n".to_owned()
    };
    for index in 1..=depth {
        writeln!(
            source,
            "copy struct S{index} {{ left: S{}\nright: S{} }}",
            index - 1,
            index - 1
        )
        .unwrap();
    }
    writeln!(source, "const func build(): S{depth} {{").unwrap();
    source.push_str(if empty {
        "let v0 = S0 {}\n"
    } else {
        "let v0 = S0 { value: 1 }\n"
    });
    for index in 1..=depth {
        writeln!(
            source,
            "let v{index} = S{index} {{ left: v{}, right: v{} }}",
            index - 1,
            index - 1
        )
        .unwrap();
    }
    writeln!(
        source,
        "v{depth}\n}}\nconst DATA: S{depth} = build()\nfunc run(): i32 {{ let data = DATA\n42 }}"
    )
    .unwrap();
    source
}
#[test]
fn shared_constant_output_is_bounded_before_expanding_physical_data() {
    assert!(matches!(
        lower(&shared_constant(23, false)),
        Err(crate::value_access::ExecutableFailure::DataLimit(_))
    ));
}
#[test]
fn exponentially_shared_zero_size_data_needs_no_expanded_serialization_or_storage() {
    let program = compile(&shared_constant(64, true));
    assert!(program.data().is_empty());
}

#[test]
fn repeated_array_constants_respect_physical_backing_limits_before_expansion() {
    assert!(matches!(
        lower("const DATA: [i32; 5000000] = [42; 5000000]\nfunc run(): i32 { DATA[0] }"),
        Err(crate::value_access::ExecutableFailure::DataLimit(_))
    ));
    let program = compile(
        "const DATA: [void; 18446744073709551615] = [(); 18446744073709551615]\nfunc run(): i32 { let _ = DATA[18446744073709551614]\n42 }",
    );
    assert!(program.data().is_empty());
}

#[test]
fn native_repetition_has_a_finite_cfg_independent_of_cardinality() {
    let mut counts = Vec::new();
    for length in [2, 100_000] {
        let program = compile(&format!(
            "func run(): i32 {{ let values = [42; {length}]\nvalues[0] }}"
        ));
        counts.push(
            program
                .functions()
                .flat_map(|(_, body)| body.body().operations())
                .count(),
        );
        assert!(
            program
                .functions()
                .flat_map(|(_, f)| f.body().operations())
                .all(|(_, operation)| !matches!(
                    operation.kind(),
                    MachineOperationKind::Aggregate(_)
                ))
        );
    }
    assert_eq!(counts[0], counts[1]);
    assert!(counts[0] < 100);
}
