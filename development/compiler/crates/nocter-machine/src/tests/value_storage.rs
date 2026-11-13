//! Actual private source -> concrete runtime shapes -> the shared Machine layout authority.
mod executable;
use std::{fmt::Write as _, sync::Arc};

use nocter_checking::value_access::accepted::SourceUnit;
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
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
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
fn contextual_runtime_shapes_keep_independent_layouts_and_value_paths() {
    use nocter_model::{AccessFieldId, ArenaBuilder, ValueField, ValueProjection};
    use nocter_runtime_contract::{
        RuntimeDataId, RuntimePrimitive, RuntimeTypeRepresentation, RuntimeTypeRepresentationTable,
        RuntimeTypeTableBuilder, RuntimeValueFieldRepresentation,
    };
    let semantic = nocter_model::TypeStore::new();
    let error = RuntimeDataId::Source(semantic.builtin(BuiltinType::Error));
    let integer = RuntimeDataId::Source(semantic.builtin(BuiltinType::I32));
    let array = RuntimeDataId::Instance(0);
    let environment = RuntimeDataId::Instance(1);
    let empty = RuntimeDataId::Instance(2);
    let captured = RuntimeDataId::Instance(3);
    let field = ArenaBuilder::<AccessFieldId, ()>::new().insert(());
    let mut types = RuntimeTypeTableBuilder::new();
    types
        .insert(error, RuntimeType::Primitive(RuntimePrimitive::Error))
        .unwrap();
    types
        .insert(
            integer,
            RuntimeType::Primitive(RuntimePrimitive::Signed(32)),
        )
        .unwrap();
    types
        .insert(
            array,
            RuntimeType::FixedArray {
                element: error,
                length: 2,
            },
        )
        .unwrap();
    types.insert(environment, RuntimeType::Aggregate).unwrap();
    types
        .insert(empty, RuntimeType::StaticCallable { environment: None })
        .unwrap();
    types
        .insert(
            captured,
            RuntimeType::StaticCallable {
                environment: Some(environment),
            },
        )
        .unwrap();
    let runtime = RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::new(std::collections::BTreeMap::from([(
            environment,
            RuntimeTypeRepresentation::ValueAggregate {
                fields: vec![RuntimeValueFieldRepresentation::new(field.into(), array)].into(),
            },
        )])),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap();
    let layouts = MachineLayoutPlan::from_runtime(&runtime, [empty, captured, integer]).unwrap();
    assert_eq!(layouts.get(empty).unwrap().size(), 0);
    assert_eq!(
        layouts.get(captured).unwrap().size(),
        layouts.get(environment).unwrap().size()
    );
    assert!(layouts.get(captured).unwrap().size() > 0);
    assert_eq!(
        layouts
            .value_field(environment, ValueField::Record(field))
            .unwrap()
            .ty(),
        array
    );
    let path = [
        ValueProjection::Record(field),
        ValueProjection::Array(1_u64),
    ];
    assert_eq!(
        runtime.value_path(environment, &path, |index, length| *index < length),
        Some(error)
    );
    let past_end = [
        ValueProjection::Record(field),
        ValueProjection::Array(2_u64),
    ];
    assert!(
        runtime
            .value_path(environment, &past_end, |index, length| *index < length)
            .is_none()
    );
    assert_contextual_value_abi(&layouts, empty, captured, integer);
}

fn assert_contextual_value_abi(
    layouts: &MachineLayoutPlan,
    empty: nocter_runtime_contract::RuntimeDataId,
    captured: nocter_runtime_contract::RuntimeDataId,
    integer: nocter_runtime_contract::RuntimeDataId,
) {
    use nocter_model::{AccessLocalId, ArenaBuilder};
    use nocter_runtime_contract::RuntimeValueInput;
    let mut locals = ArenaBuilder::<AccessLocalId, ()>::new();
    let signature = RuntimeValueSignature::new(
        None,
        [
            RuntimeValueInput {
                local: locals.insert(()),
                data: empty,
                operation: ValueInputOperation::Take,
            },
            RuntimeValueInput {
                local: locals.insert(()),
                data: captured,
                operation: ValueInputOperation::Take,
            },
        ],
        integer,
    );
    let abi = MachineValueCallableAbi::build(signature, layouts).unwrap();
    assert_eq!(abi.transport().arguments()[0].ty(), empty);
    assert!(abi.transport().arguments()[0].location().is_none());
    assert_eq!(abi.transport().arguments()[1].ty(), captured);
    assert!(abi.transport().arguments()[1].location().is_some());
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
        .map(|&field| {
            layouts
                .value_field(
                    nocter_runtime_contract::RuntimeDataId::Source(data),
                    field.into(),
                )
                .unwrap()
                .offset()
        })
        .collect();
    assert_eq!(offsets, [0, 1, 4, 8, 8, 8]);
    assert_ne!(fields[3], fields[4]);
    assert_ne!(
        layouts
            .value_field(
                nocter_runtime_contract::RuntimeDataId::Source(data),
                fields[3].into()
            )
            .unwrap()
            .ty(),
        layouts
            .value_field(
                nocter_runtime_contract::RuntimeDataId::Source(data),
                fields[4].into()
            )
            .unwrap()
            .ty()
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
    assert_eq!(
        environment
            .types()
            .get(nocter_runtime_contract::RuntimeDataId::Source(unit)),
        Some(&RuntimeType::Unit)
    );
    assert_eq!(layouts.get(unit).unwrap().kind(), &MachineLayoutKind::Unit);
    // A field from another record never acquires meaning just because its offset would fit.
    assert!(
        layouts
            .value_field(
                nocter_runtime_contract::RuntimeDataId::Source(empty),
                fields[0].into()
            )
            .is_none()
    );
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
        let target = call.target().instance().unwrap();
        let signature = storage::signature(&instances, target).unwrap();
        let abi = MachineValueCallableAbi::build(signature, &layouts).unwrap();
        let (input, transport) = abi.inputs().next().unwrap();
        assert_eq!(
            input.local,
            instances
                .body(target)
                .unwrap()
                .typed()
                .body()
                .inputs()
                .explicit()[0]
                .local
        );
        assert_eq!(input.operation, call.inputs()[0].operation);
        assert_eq!(
            transport.ty(),
            nocter_runtime_contract::RuntimeDataId::Source(data)
        );
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
                assert!(
                    environment
                        .types()
                        .get(nocter_runtime_contract::RuntimeDataId::Source(data))
                        .is_none()
                );
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
                let target = call.target().instance().unwrap();
                let signature = storage::signature(&instances, target).unwrap();
                let body = instances.body(target).unwrap().typed().body();
                for (index, (&binding, input)) in
                    call.inputs().iter().zip(signature.explicit()).enumerate()
                {
                    assert_eq!(input.local, body.inputs().explicit()[index].local);
                    assert_eq!(input.operation, binding.operation);
                    assert_eq!(
                        input.data,
                        nocter_runtime_contract::RuntimeDataId::Source(binding.payload.data())
                    );
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
                        assert_eq!(
                            environment.data,
                            nocter_runtime_contract::RuntimeDataId::Source(payload.data())
                        );
                    }
                }
                assert_eq!(
                    signature.result(),
                    nocter_runtime_contract::RuntimeDataId::Source(call.result().data())
                );
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

#[test]
fn structural_tuple_layout_uses_logical_positions_and_shared_member_padding() {
    let instances = close(&ValueSourceFixture::new(
        "func run(take value: (bool, void, i32, void)): i32 { value.2 }",
    ));
    let environment = environment(&instances);
    let layouts = layouts(&environment).unwrap();
    let data = storage::signature(&instances, instances.root())
        .unwrap()
        .explicit()[0]
        .data;
    let fields = (0..4)
        .map(nocter_model::ValueField::Tuple)
        .collect::<Vec<_>>();
    let offsets = fields
        .iter()
        .map(|&field| layouts.value_field(data, field).unwrap().offset())
        .collect::<Vec<_>>();
    assert_eq!(offsets, [0, 1, 4, 8]);
    assert_eq!(
        (
            layouts.get(data).unwrap().size(),
            layouts.get(data).unwrap().alignment()
        ),
        (8, 4)
    );
    assert!(
        layouts
            .value_field(data, nocter_model::ValueField::Tuple(4))
            .is_none()
    );
}

#[test]
fn empty_value_arrays_have_geometry_without_materializing_their_element() {
    let instances = close(&ValueSourceFixture::new(
        "struct Node { next: [Node; 0] }\nfunc run(): [Node; 0] { [] }",
    ));
    let runtime = environment(&instances);
    let (data, element) = runtime
        .types()
        .iter()
        .find_map(|(data, kind)| match kind {
            RuntimeType::EmptyArray { element } => Some((data, *element)),
            _ => None,
        })
        .unwrap();
    assert!(runtime.types().get(element).is_none());
    let plan = layouts(&runtime).unwrap();
    let layout = plan.get(data).unwrap();
    assert_eq!((layout.size(), layout.alignment()), (0, 1));
    assert!(
        matches!(layout.kind(), MachineLayoutKind::FixedArray { element: selected, length: 0, stride: 0 } if *selected == element)
    );
    assert!(plan.get(element).is_none());
    assert!(
        plan.value_field(data, nocter_model::ValueField::Array(0))
            .is_none()
    );
}

#[test]
fn empty_value_array_geometry_preserves_the_released_fixed_array_policy() {
    use nocter_runtime_contract::{
        RuntimePrimitive, RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
    };
    let semantic = nocter_model::TypeStore::new();
    let element = semantic.builtin(BuiltinType::I32);
    let empty = semantic.builtin(BuiltinType::Bool);
    let fixed = semantic.builtin(BuiltinType::Usize);
    let mut types = RuntimeTypeTableBuilder::new();
    types
        .insert(
            element,
            RuntimeType::Primitive(RuntimePrimitive::Signed(32)),
        )
        .unwrap();
    types
        .insert(
            empty,
            RuntimeType::EmptyArray {
                element: nocter_runtime_contract::RuntimeDataId::Source(element),
            },
        )
        .unwrap();
    types
        .insert(
            fixed,
            RuntimeType::FixedArray {
                element: nocter_runtime_contract::RuntimeDataId::Source(element),
                length: 0,
            },
        )
        .unwrap();
    let runtime = RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::default(),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap();
    let plan = layouts(&runtime).unwrap();
    assert_eq!(
        (
            plan.get(empty).unwrap().size(),
            plan.get(empty).unwrap().alignment()
        ),
        (0, 1)
    );
    assert_eq!(
        (
            plan.get(fixed).unwrap().size(),
            plan.get(fixed).unwrap().alignment()
        ),
        (0, 4)
    );
}

#[test]
fn iterative_layout_dependencies_reject_positive_cycles_and_preserve_empty_cuts() {
    use nocter_runtime_contract::{RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder};
    let semantic = nocter_model::TypeStore::new();
    let first = semantic.builtin(BuiltinType::I32);
    let next = semantic.builtin(BuiltinType::Bool);
    for cut in [false, true] {
        let mut types = RuntimeTypeTableBuilder::new();
        types
            .insert(
                first,
                RuntimeType::Optional(nocter_runtime_contract::RuntimeDataId::Source(next)),
            )
            .unwrap();
        types
            .insert(
                next,
                if cut {
                    RuntimeType::EmptyArray {
                        element: nocter_runtime_contract::RuntimeDataId::Source(first),
                    }
                } else {
                    RuntimeType::Optional(nocter_runtime_contract::RuntimeDataId::Source(first))
                },
            )
            .unwrap();
        let runtime = RuntimeEnvironment::new(
            types.finish().unwrap(),
            RuntimeTypeRepresentationTable::default(),
            RuntimeAbiIdentity::Arm64DarwinV1,
        )
        .unwrap();
        let result = MachineLayoutPlan::from_runtime(
            &runtime,
            [nocter_runtime_contract::RuntimeDataId::Source(first)],
        );
        if cut {
            assert_eq!(result.unwrap().get(first).unwrap().size(), 1);
        } else {
            assert!(
                matches!(result, Err(MachineLayoutError::RecursiveValue(id)) if id == nocter_runtime_contract::RuntimeDataId::Source(first))
            );
        }
    }
}
