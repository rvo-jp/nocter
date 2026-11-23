//! Machine cleanup reads one tag and visits the selected case's owned fields.
use super::*;
use crate::value_access::executable::{data::Data, transport::Prepared};
use crate::{MachineBody, MachineLayoutPlan, MachineSwitchValue, MachineValueDefinition};
use nocter_model::{AccessFieldId, ArenaBuilder, VariantId};
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimeEnvironment, RuntimePrimitive, RuntimeType,
    RuntimeTypeRepresentation, RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
    RuntimeValueVariantRepresentation,
};
use std::collections::{BTreeMap, BTreeSet};

fn environment([data, tag, error, callback]: [RuntimeDataId; 4]) -> RuntimeEnvironment {
    let mut types = RuntimeTypeTableBuilder::new();
    for (id, kind) in [
        (data, RuntimeType::Aggregate),
        (tag, RuntimeType::Primitive(RuntimePrimitive::Unsigned(16))),
        (error, RuntimeType::Primitive(RuntimePrimitive::Error)),
        (
            callback,
            RuntimeType::StaticCallable {
                environment: Some(error),
            },
        ),
    ] {
        types.insert(id, kind).unwrap();
    }
    let mut variants = ArenaBuilder::<VariantId, ()>::new();
    let mut fields = ArenaBuilder::<AccessFieldId, ()>::new();
    let first = [(fields.insert(()), error)];
    let last = [(fields.insert(()), error), (fields.insert(()), callback)];
    let cases = (0..257)
        .map(|case| {
            let layout = match case {
                0 => first.as_slice(),
                256 => last.as_slice(),
                _ => &[],
            };
            RuntimeValueVariantRepresentation::new(variants.insert(()), layout)
        })
        .collect();
    RuntimeEnvironment::new(
        types.finish().unwrap(),
        RuntimeTypeRepresentationTable::new(
            [(
                data,
                RuntimeTypeRepresentation::ValueEnum { variants: cases },
            )]
            .into(),
        ),
        RuntimeAbiIdentity::Arm64DarwinV1,
    )
    .unwrap()
}

fn cleanup_calls(body: &MachineBody, block: MachineBlockId) -> Vec<(MachineFunctionId, u64)> {
    body.block(block)
        .unwrap()
        .operations()
        .iter()
        .filter_map(|&id| {
            let MachineOperationKind::Call(call) = body.operation(id).unwrap().kind() else {
                return None;
            };
            let MachineCallTarget::Direct(target) = call.target() else {
                panic!("direct cleanup helper")
            };
            let [argument] = call.arguments() else {
                panic!("one cleanup address")
            };
            let MachineValueDefinition::Operation(address) =
                body.value(*argument).unwrap().definition()
            else {
                panic!("selected field address")
            };
            let MachineOperationKind::AddressOf { source } =
                body.operation(address).unwrap().kind()
            else {
                panic!("address operation")
            };
            let Some(MachineAddressStep::Offset(offset)) =
                body.address(*source).unwrap().steps().last()
            else {
                panic!("relative field offset")
            };
            Some((*target, *offset))
        })
        .collect()
}

#[test]
fn cleanup_uses_the_full_tag_width_and_visits_fields_in_reverse_order() {
    let [data, tag, error, callback] = [0, 1, 2, 3].map(RuntimeDataId::Instance);
    let runtime = environment([data, tag, error, callback]);
    let layouts = MachineLayoutPlan::from_runtime(&runtime, [data, tag]).unwrap();
    let mut needed = BTreeMap::new();
    assert!(super::super::super::cleanup::required(&runtime, data, &mut needed).unwrap());
    assert_eq!(
        needed,
        [(data, true), (error, true), (callback, true)].into()
    );
    let error_helper = MachineFunctionId::new(1);
    let closure_helper = MachineFunctionId::new(2);
    let constants = Data::default();
    let context = Context {
        runtime: &runtime,
        layouts: &layouts,
        data: &constants,
        functions: BTreeMap::new(),
        abis: BTreeMap::new(),
        cleanup: [(error, error_helper), (callback, closure_helper)].into(),
        transports: Prepared {
            helpers: BTreeMap::new(),
            copies: BTreeSet::new(),
        },
        primitives: BTreeMap::new(),
    };
    let function = build(&context, data, MachineFunctionId::new(0)).unwrap();
    let body = function.body();
    let MachineTerminator::SwitchValue {
        subject,
        cases,
        fallback,
    } = body.block(body.entry()).unwrap().terminator()
    else {
        panic!("tag dispatch");
    };
    assert_eq!(body.value(*subject).unwrap().ty(), tag);
    let mut loads = body
        .operations()
        .filter_map(|(_, operation)| match operation.kind() {
            MachineOperationKind::Load { source } => Some(*source),
            _ => None,
        });
    let source = loads.next().expect("tag load");
    assert!(loads.next().is_none());
    assert_eq!(body.address(source).unwrap().stored_size(), Some(2));
    for (tag, expected) in [
        (0, vec![(error_helper, 8)]),
        (1, vec![]),
        (256, vec![(closure_helper, 16), (error_helper, 8)]),
    ] {
        let case = cases
            .iter()
            .find(|case| case.value() == MachineSwitchValue::Integer(tag))
            .unwrap();
        assert_eq!(cleanup_calls(body, case.target().block()), expected);
    }
    assert!(cleanup_calls(body, fallback.block()).is_empty());
}
