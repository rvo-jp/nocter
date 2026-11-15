//! Empty representation domains are metadata, distinct from unknown code and received values.
use super::*;
use crate::value_access::{DataInstanceKey, InstanceSchema};
use nocter_model::ValueField;

#[test]
fn domain_instantiation_shares_the_canonical_fold_without_turning_missing_evidence_into_emptiness()
{
    let (headers, [run, _, plain, wrong, factory]) = headers();
    let parameter = headers.codes().get(run).unwrap().parameters()[0].into();
    let formal = headers
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(parameter),
            invocation: nocter_model::ValueInvocation::Read,
        })
        .unwrap();
    let pending = headers.codes().get(factory).unwrap().result();
    let mut body = headers.body_types(run).unwrap();
    let optional = body.optional(pending).unwrap();
    let array = body.array(formal, u64::MAX).unwrap();
    let empty = body.array(pending, 0).unwrap();
    let root = body.tuple(&[optional, array, empty, formal]).unwrap();
    let program = body.finish();
    let callback = program.concrete_witness(plain.into()).unwrap();
    let visits = std::cell::RefCell::new(std::collections::BTreeSet::new());
    let result = program
        .instantiate_callable_domains(root, |data, witness| {
            assert!(
                visits.borrow_mut().insert(data),
                "shared schema leaves resolve once"
            );
            match witness.origin {
                ValueFunctionOrigin::Parameter(selected) if selected == parameter => {
                    Ok(Some(callback.clone()))
                }
                ValueFunctionOrigin::Result(_) => Ok(None),
                _ => Err(CodeBindingError::InvalidWitness),
            }
        })
        .unwrap();
    assert_eq!(visits.into_inner().len(), 2);
    assert_eq!(result.data(), root);
    assert!(!result.is_uninhabited());
    assert!(
        result
            .component(&program, ValueField::Tuple(0))
            .unwrap()
            .component(&program, ValueField::Variant(true))
            .unwrap()
            .is_uninhabited()
    );
    let positive = result
        .component(&program, ValueField::Tuple(1))
        .unwrap()
        .component(&program, ValueField::Array(u64::MAX - 1))
        .unwrap();
    assert!(
        matches!(positive.schema(&program), Ok(InstanceSchema::Function(actual)) if actual == callback)
    );
    assert!(
        program
            .instantiate_callable_domains(root, |_, _| Err(CodeBindingError::InvalidWitness))
            .is_err()
    );
    let wrong = program.concrete_witness(wrong.into()).unwrap();
    assert!(
        program
            .instantiate_callable_domains(formal, |_, _| Ok(Some(wrong.clone())))
            .is_err()
    );
    let retry = program
        .instantiate_callable_domains(root, |data, _| {
            if data == pending {
                Ok(None)
            } else {
                Ok(Some(callback.clone()))
            }
        })
        .unwrap();
    assert_eq!(retry, result);
    assert!(program.instantiate_closed_data(root).is_err());
}

#[test]
fn empty_domains_preserve_types_without_providing_callable_code_or_a_positive_component() {
    let (program, [_, _, plain, _, factory]) = headers();
    let pending = program.codes().get(factory).unwrap().result();
    let empty = program.instantiate_uninhabited_data(pending).unwrap();
    assert_eq!(empty.data(), pending);
    assert!(empty.is_uninhabited());
    assert!(!empty.is_context_free());
    assert!(matches!(
        empty.schema(&program),
        Ok(InstanceSchema::Uninhabited)
    ));
    assert!(empty.function_representation(&program).unwrap().is_none());
    assert!(empty.environment(&program).unwrap().is_none());
    assert!(program.instantiate_closed_data(pending).is_err());
    assert!(empty.component(&program, ValueField::Tuple(0)).is_err());
    assert_eq!(
        empty,
        program.instantiate_uninhabited_data(pending).unwrap()
    );
    assert!(empty.has_same_representation(&program, &empty));
    let positive = program
        .instantiate_closed_data(program.function_data(plain.into()).unwrap())
        .unwrap();
    assert!(!empty.has_same_representation(&program, &positive));
    assert_ne!(empty, positive);
    let empty_known = program
        .instantiate_uninhabited_data(positive.data())
        .unwrap();
    assert_ne!(empty_known, positive);
    assert!(!empty_known.has_same_representation(&program, &positive));
    assert_eq!(
        std::collections::BTreeSet::from([empty_known.clone(), positive.clone()]).len(),
        2
    );
    assert!(positive < empty_known);
    assert!(format!("{empty_known:?}").contains("Uninhabited"));
}

#[test]
fn products_propagate_empty_domains_but_outcomes_and_zero_arrays_retain_inhabited_alternatives() {
    let (headers, [run, _, _, _, factory]) = headers();
    let pending = headers.codes().get(factory).unwrap().result();
    let integer = headers.data_type(BuiltinType::I32).unwrap();
    let error = headers.data_type(BuiltinType::Error).unwrap();
    let mut body = headers.body_types(run).unwrap();
    let optional = body.optional(pending).unwrap();
    let fallible = body.fallible(pending).unwrap();
    let pair = body.tuple(&[pending, integer]).unwrap();
    let array = body.array(pending, u64::MAX).unwrap();
    let zero = body.array(pending, 0).unwrap();
    let program = body.finish();
    let empty = program.instantiate_uninhabited_data(pending).unwrap();
    let assemble = |data, fields: Vec<(ValueField, DataInstanceKey)>| {
        program.instantiate_components(data, fields).unwrap()
    };
    let absent = assemble(optional, vec![(ValueField::Variant(true), empty.clone())]);
    assert!(!absent.is_uninhabited());
    assert!(
        absent
            .component(&program, ValueField::Variant(true))
            .unwrap()
            .is_uninhabited()
    );
    assert!(!absent.is_context_free());
    let failure = assemble(
        fallible,
        vec![
            (ValueField::Variant(true), empty.clone()),
            (
                ValueField::Variant(false),
                program.instantiate_closed_data(error).unwrap(),
            ),
        ],
    );
    assert!(!failure.is_uninhabited());
    let no_failure = assemble(
        fallible,
        vec![
            (ValueField::Variant(true), empty.clone()),
            (
                ValueField::Variant(false),
                program.instantiate_uninhabited_data(error).unwrap(),
            ),
        ],
    );
    assert!(no_failure.is_uninhabited());
    let product = assemble(
        pair,
        vec![
            (ValueField::Tuple(0), empty.clone()),
            (
                ValueField::Tuple(1),
                program.instantiate_closed_data(integer).unwrap(),
            ),
        ],
    );
    assert!(product.is_uninhabited());
    assert!(product.component(&program, ValueField::Tuple(1)).is_err());
    assert!(assemble(array, vec![(ValueField::Array(0), empty)]).is_uninhabited());
    assert!(!assemble(zero, vec![]).is_uninhabited());
    assert!(
        program.instantiate_components(optional, []).is_err(),
        "unknown payload metadata is not an empty-domain proof"
    );
}

#[test]
fn empty_domains_cannot_supply_an_actual_environment_or_received_input() {
    let fixture = fixture();
    let program = &fixture.program;
    let data = program.function_data(fixture.captured).unwrap();
    let ValueFunctionOrigin::Closure(captured) = fixture.captured.origin else {
        unreachable!()
    };
    let environment = program.closure(captured).unwrap().environment();
    let empty = program.instantiate_uninhabited_data(environment).unwrap();
    assert!(
        program
            .bind_callable_data(fixture.captured, &empty)
            .is_err()
    );
    let selected = program.concrete_witness(fixture.receiver).unwrap();
    let ValueFunctionOrigin::Closure(receiver) = fixture.receiver.origin else {
        unreachable!()
    };
    let direct = nocter_model::ValueParameter::Closure {
        owner: receiver,
        position: 1,
    };
    let callback = program.concrete_witness(fixture.callbacks[0]).unwrap();
    let empty_pair = program.instantiate_uninhabited_data(fixture.pair).unwrap();
    assert_eq!(
        program.bind_instance_with_inputs(&selected, [(direct, callback)], [(0, empty_pair)]),
        Err(crate::value_access::InputBindingError::Schema(0))
    );
    assert!(
        program
            .instantiate_uninhabited_data(data)
            .unwrap()
            .function_representation(program)
            .unwrap()
            .is_none()
    );
}
