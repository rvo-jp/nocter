//! Actual private source -> concrete runtime shapes -> the shared Machine layout authority.
mod activation;
use std::{fmt::Write as _, sync::Arc};

use nocter_checking::value_access::accepted::Unit;
use nocter_declaration_lowering::access_declarations;
use nocter_model::{BuiltinType, TypeId, ValueInputOperation};
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimeEnvironment, RuntimeType, RuntimeValueSignature,
};
use nocter_target_program::value_access::{Environment, InstanceProgram, storage};
use nocter_test_support::ValueSourceFixture;

use crate::{
    MachineAbiError, MachineArgumentLocation, MachineLayoutError, MachineLayoutKind,
    MachineLayoutPlan, MachineResultAbi, MachineResultLocation, MachineValueCallableAbi,
    MachineValueClass,
};

fn close(fixture: &ValueSourceFixture) -> InstanceProgram {
    let headers = access_declarations::Query::prepare(fixture.syntax()).unwrap();
    let unit = Arc::new(Unit::prepare(&headers.bind(fixture.syntax()).unwrap()).unwrap());
    let root = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    InstanceProgram::prepare(unit, root).unwrap()
}

fn environment(instances: &InstanceProgram) -> RuntimeEnvironment {
    storage::prepare(instances, RuntimeAbiIdentity::Arm64DarwinV1).unwrap()
}

fn layouts(environment: &RuntimeEnvironment) -> Result<MachineLayoutPlan, MachineLayoutError> {
    MachineLayoutPlan::from_runtime(environment, environment.types().iter().map(|(ty, _)| ty))
}

fn record(instances: &InstanceProgram, name: &str) -> TypeId {
    let program = instances.unit().program();
    program
        .records()
        .iter()
        .find(|(_, record)| program.symbols().spelling(record.name()) == Some(name))
        .unwrap()
        .1
        .data()
}

#[test]
fn records_preserve_alignment_padding_and_zero_byte_member_identity() {
    let instances = close(&ValueSourceFixture::new(
        "struct Empty {}\nstruct Data { flag: bool\nfirst: void\nnumber: i32\nlast: void\nempty: Empty\nfailure: error }\nfunc run(take input: Data): void {}",
    ));
    let environment = environment(&instances);
    let layouts = layouts(&environment).unwrap();
    let program = instances.unit().program();
    let data = record(&instances, "Data");
    let fields = program
        .records()
        .iter()
        .find(|(_, r)| r.data() == data)
        .unwrap()
        .1
        .fields();
    let offsets: Vec<_> = fields
        .iter()
        .map(|&field| layouts.value_field(data, field).unwrap().offset())
        .collect();
    assert_eq!(offsets, [0, 1, 4, 8, 8, 8]);
    assert_ne!(fields[3], fields[4]);
    assert_ne!(
        layouts.value_field(data, fields[3]).unwrap().ty(),
        layouts.value_field(data, fields[4]).unwrap().ty()
    );
    assert_eq!(
        (
            layouts.get(data).unwrap().size(),
            layouts.get(data).unwrap().alignment()
        ),
        (16, 8)
    );
    let empty = record(&instances, "Empty");
    assert_eq!(
        (
            layouts.get(empty).unwrap().size(),
            layouts.get(empty).unwrap().alignment()
        ),
        (0, 1)
    );
    assert_eq!(layouts.class(empty), Some(MachineValueClass::Zero));
    let unit = program.data_type(BuiltinType::Void).unwrap();
    assert_eq!(environment.types().get(unit), Some(&RuntimeType::Unit));
    assert_eq!(layouts.get(unit).unwrap().kind(), &MachineLayoutKind::Unit);
    // A field from another record never acquires meaning just because its offset would fit.
    assert!(layouts.value_field(empty, fields[0]).is_none());
}

#[test]
fn selected_named_functions_are_zero_byte_tokens_and_captures_are_inline_payloads() {
    let instances = close(&ValueSourceFixture::new(
        "func plain(): void {}\nfunc apply(take action: once(): void): void { action^() }\nfunc run(take input: error): void {\nlet named = plain\nlet captured = (input^;): void {}\napply(named^)\napply(captured^)\n}",
    ));
    let environment = environment(&instances);
    let layouts = layouts(&environment).unwrap();
    let error = environment.abi().schema().error();
    let mut tokens = 0;
    let mut captures = 0;
    for (ty, kind) in environment.types().iter() {
        if let RuntimeType::StaticCallable { environment } = kind {
            let layout = layouts.get(ty).unwrap();
            assert_eq!(
                layout.kind(),
                &MachineLayoutKind::StaticCallable {
                    environment: *environment
                }
            );
            if let Some(environment) = environment {
                captures += 1;
                assert_eq!(layout.size(), error.size());
                assert_eq!(layout.alignment(), error.alignment());
                assert_eq!(
                    layouts.get(*environment).unwrap().kind(),
                    &MachineLayoutKind::ErrorHandle
                );
            } else {
                tokens += 1;
                assert_eq!(layout.size(), 0);
                assert_eq!(layouts.class(ty), Some(MachineValueClass::Zero));
            }
        }
    }
    assert!(tokens > 0 && captures > 0);
}

#[test]
fn access_operations_do_not_change_data_layout_or_create_borrow_types() {
    let instances = close(&ValueSourceFixture::new(
        "struct Data { value: i32 }\nfunc read_input(input: Data): void {}\nfunc edit_input(edit input: Data): void {}\nfunc take_input(take input: Data): void {}\nfunc run(take input: Data): void {\nread_input(input)\nedit_input(input)\ntake_input(input^)\n}",
    ));
    let environment = environment(&instances);
    let layouts = layouts(&environment).unwrap();
    let data = record(&instances, "Data");
    assert_eq!(layouts.get(data).unwrap().size(), 4);
    assert!(
        environment
            .types()
            .iter()
            .all(|(_, kind)| !matches!(kind, RuntimeType::Borrow { .. } | RuntimeType::Pointer(_)))
    );
    let root = instances.instances().get(instances.root()).unwrap();
    assert_eq!(root.calls().len(), 3);
    for call in root.calls().values() {
        assert_eq!(call.inputs()[0].payload.data(), data);
        let signature = storage::signature(&instances, call.target()).unwrap();
        let abi = MachineValueCallableAbi::build(signature, &layouts).unwrap();
        let (input, transport) = abi.inputs().next().unwrap();
        assert_eq!(input.local, call.inputs()[0].input.local);
        assert_eq!(input.operation, call.inputs()[0].input.operation);
        assert_eq!(transport.ty(), data);
        assert!(
            matches!(abi.transport().result(), MachineResultAbi::Value(result) if result.class() == MachineValueClass::Zero)
        );
    }
}

#[test]
fn interrupted_unproduced_callable_shapes_do_not_request_symbolic_storage() {
    let instances = close(&ValueSourceFixture::new(
        "func forward(take action: func(i32): i32): func(i32): i32 { action^ }\nfunc run(): i32 {\nlet absent = forward(if true { return 1 } else { return 2 })\nlet adapted: once(i32): i32 = absent\nadapted(1)\n}",
    ));
    assert!(
        instances
            .instances()
            .iter()
            .any(|(_, instance)| instance.data().values().any(|e| e.payload().is_none()))
    );
    let environment = environment(&instances);
    let layouts = layouts(&environment).unwrap();
    let signature = storage::signature(&instances, instances.root()).unwrap();
    let abi = MachineValueCallableAbi::build(signature, &layouts).unwrap();
    assert!(
        matches!(abi.transport().result(), MachineResultAbi::Value(result)
        if result.class() == MachineValueClass::Direct { words: 1 })
    );
    for (_, instance) in instances.instances().iter() {
        for (&data, evidence) in instance.data() {
            if evidence.payload().is_none() {
                assert!(environment.types().get(data).is_none());
                assert!(layouts.get(data).is_none());
            }
        }
    }
    assert!(
        environment
            .types()
            .iter()
            .all(|(_, kind)| !matches!(kind, RuntimeType::Callable | RuntimeType::ErasedCallable))
    );
}

#[test]
fn dependent_and_captured_source_witnesses_reach_closed_layouts() {
    for case in [
        "a82-callback-forward-read.nct",
        "a84-callback-forward-adapter.nct",
        "a99-callable-result-forward.nct",
        "a107-callable-result-callback-input.nct",
        "a124-dependent-result-relay.nct",
        "a125-dependent-result-recursion.nct",
        "a126-dependent-result-permutation.nct",
        "a136-dependent-result-mut.nct",
        "a138-owned-capture-read.nct",
        "a140-owned-capture-take.nct",
        "a141-owned-capture-partial.nct",
        "a147-owned-capture-input-operations.nct",
    ] {
        let instances = close(&ValueSourceFixture::corpus(case));
        let environment = environment(&instances);
        let layouts = layouts(&environment).unwrap_or_else(|failure| panic!("{case}: {failure:?}"));
        for (ty, _) in environment.types().iter() {
            assert!(layouts.get(ty).is_some());
        }
        // Storage must refer to the already closed concrete identities, not caller formals.
        for (_, instance) in instances.instances().iter() {
            for evidence in instance.data().values() {
                if let Some(payload) = evidence.payload() {
                    assert!(layouts.get(payload.data()).is_some());
                }
            }
            for call in instance.calls().values() {
                let signature = storage::signature(&instances, call.target()).unwrap();
                for (&binding, input) in call.inputs().iter().zip(signature.explicit()) {
                    assert_eq!(input.local, binding.input.local);
                    assert_eq!(input.operation, binding.input.operation);
                    assert_eq!(input.data, binding.payload.data());
                }
                assert_eq!(call.inputs().len(), signature.explicit().len());
                match call.environment() {
                    Environment::None | Environment::CodeToken { .. } => {
                        assert!(signature.environment().is_none());
                    }
                    Environment::Capture { input, payload, .. } => {
                        let environment = signature.environment().unwrap();
                        assert_eq!(environment.local, input.local);
                        assert_eq!(environment.operation, input.operation);
                        assert_eq!(environment.data, payload.data());
                    }
                }
                assert_eq!(signature.result(), call.result().data());
                MachineValueCallableAbi::build(signature, &layouts).unwrap();
            }
        }
    }
}

#[test]
fn zero_byte_inputs_keep_access_channels_and_wide_values_use_indirect_transport() {
    let instances = close(&ValueSourceFixture::new(
        "struct Wide { first: error\nsecond: error\nthird: error }\nfunc run(first: void, edit second: void, take third: void, view: Wide, edit update: Wide, take owned: Wide, a: i32, b: i32, c: i32, d: i32, e: i32, f: i32): Wide { owned^ }",
    ));
    let environment = environment(&instances);
    let layouts = layouts(&environment).unwrap();
    let signature = storage::signature(&instances, instances.root()).unwrap();
    let first = signature.explicit()[0];
    let abi = MachineValueCallableAbi::build(signature, &layouts).unwrap();
    let arguments: Vec<_> = abi.inputs().collect();
    for (index, register) in [
        (0, 0),
        (1, 1),
        (3, 2),
        (4, 3),
        (5, 4),
        (6, 5),
        (7, 6),
        (8, 7),
    ] {
        assert!(
            matches!(arguments[index].1.location(), Some(MachineArgumentLocation::Registers(span)) if span.first() == register && span.words() == 1)
        );
    }
    assert_eq!(arguments[0].0.operation, ValueInputOperation::Read);
    assert_eq!(arguments[1].0.operation, ValueInputOperation::Edit);
    assert_eq!(arguments[2].0.operation, ValueInputOperation::Take);
    assert_eq!(arguments[0].0.data, arguments[2].0.data);
    assert_eq!(arguments[2].1.class(), MachineValueClass::Zero);
    assert!(arguments[2].1.location().is_none());
    assert_eq!(
        arguments[3].1.class(),
        MachineValueClass::Direct { words: 1 }
    );
    assert_eq!(
        arguments[4].1.class(),
        MachineValueClass::Direct { words: 1 }
    );
    assert_eq!(arguments[5].1.class(), MachineValueClass::Indirect);
    for (index, offset) in [(9, 0), (10, 8), (11, 16)] {
        assert!(
            matches!(arguments[index].1.location(), Some(MachineArgumentLocation::Stack(slot)) if slot.offset() == offset && slot.size() == 8 && slot.alignment() == 8)
        );
    }
    assert_eq!(abi.transport().stack_argument_size(), 32);
    assert!(
        matches!(abi.transport().result(), MachineResultAbi::Value(result)
        if result.class() == MachineValueClass::Indirect && result.location() == MachineResultLocation::CallerStorage { pointer_register: 8 })
    );

    let duplicate = RuntimeValueSignature::new(Some(first), vec![first], abi.signature().result());
    assert_eq!(
        MachineValueCallableAbi::build(duplicate, &layouts).unwrap_err(),
        MachineAbiError::DuplicateValueInput(first.local)
    );
}

#[test]
fn shared_record_schemas_reject_size_overflow_without_expanding_payload_trees() {
    let mut source = String::from("struct Leaf { value: i32 }\n");
    let mut previous = String::from("Leaf");
    for depth in 0..64 {
        let next = format!("Layer{depth}");
        writeln!(
            source,
            "struct {next} {{ first: {previous}\nsecond: {previous} }}"
        )
        .unwrap();
        previous = next;
    }
    write!(source, "func run(take input: {previous}): void {{}}").unwrap();
    let instances = close(&ValueSourceFixture::new(&source));
    let environment = environment(&instances);
    assert_eq!(environment.types().iter().len(), 67);
    assert!(matches!(
        layouts(&environment),
        Err(MachineLayoutError::LayoutOverflow(_))
    ));
}
