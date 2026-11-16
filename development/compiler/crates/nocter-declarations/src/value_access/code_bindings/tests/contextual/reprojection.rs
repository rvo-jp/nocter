//! Existing instance keys retain separate occurrences across equivalent declaration descriptors.
use super::*;
use crate::value_access::DataInstanceKey;
use nocter_model::{TypeId, ValueField};

#[test]
fn reprojection_preserves_distinct_contexts_shared_graphs_and_empty_domains() {
    let (headers, [creator, edit, plain, wrong, _]) = headers();
    let formal = headers.codes().get(creator).unwrap().parameters()[0].into();
    let formal_data = headers
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(formal),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let edit_data = headers
        .parameters()
        .get(headers.codes().get(edit).unwrap().parameters()[0])
        .unwrap()
        .data();
    let void = headers.data_type(BuiltinType::Void).unwrap();
    let mut body = headers.body_types(creator).unwrap();
    let contract = FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: void,
    };
    let captured = body.reserve_closure().unwrap();
    let data = body
        .closure(captured, formal_data, contract.clone())
        .unwrap();
    let other = body.reserve_closure().unwrap();
    body.closure(other, void, contract).unwrap();
    let callbacks = [
        FunctionWitness::from(plain),
        FunctionWitness {
            origin: ValueFunctionOrigin::Closure(other),
            invocation: ValueInvocation::Read,
        },
    ];
    let optional = body.optional(data).unwrap();
    let target_optional = body.optional(formal_data).unwrap();
    let source_pair = body.tuple(&[data, data]).unwrap();
    let target_pair = body.tuple(&[formal_data, formal_data]).unwrap();
    let mut layers = vec![(source_pair, target_pair)];
    for _ in 0..48 {
        let &(source, target) = layers.last().unwrap();
        layers.push((
            body.tuple(&[source, source]).unwrap(),
            body.tuple(&[target, target]).unwrap(),
        ));
    }
    let &(source, target) = layers.last().unwrap();
    let source_array = body.array(source, u64::MAX).unwrap();
    let target_array = body.array(target, u64::MAX).unwrap();
    let program = body.finish();
    let keys = callbacks.map(|callback| {
        let frame = program
            .bind_code(creator.into(), [(formal, callback)])
            .unwrap();
        program.instantiate_data(data, &frame).unwrap()
    });
    assert_ne!(keys[0], keys[1]);
    let value = shared_value(&program, &layers, &keys, source_array);
    let projected = value.reproject(&program, target_array).unwrap();
    assert_eq!(projected.data(), target_array);
    assert_eq!(value.data(), source_array);
    assert!(value.has_same_representation(&program, &projected));
    let mut pair = projected
        .component(&program, ValueField::Array(u64::MAX - 1))
        .unwrap();
    for _ in 0..48 {
        pair = pair.component(&program, ValueField::Tuple(0)).unwrap();
    }
    assert_eq!(pair.data(), target_pair);
    for (i, original) in keys.iter().enumerate() {
        let actual = pair.component(&program, ValueField::Tuple(i)).unwrap();
        assert_eq!(actual.data(), formal_data);
        assert!(actual.has_same_representation(&program, original));
    }
    let empty = program.instantiate_uninhabited_data(data).unwrap();
    let absent = program
        .instantiate_components(optional, [(ValueField::Variant(true), empty)])
        .unwrap();
    let converted = absent.reproject(&program, target_optional).unwrap();
    assert!(
        converted
            .component(&program, ValueField::Variant(true))
            .unwrap()
            .is_uninhabited()
    );
    assert!(converted.has_same_representation(&program, &absent));
    assert!(keys[0].reproject(&program, edit_data).is_err());
    assert!(
        keys[0]
            .reproject(&program, program.function_data(wrong.into()).unwrap())
            .is_err()
    );
    assert!(value.reproject(&program, target_optional).is_err());
    assert_eq!(value.reproject(&program, target_array).unwrap(), projected);
}

fn shared_value(
    program: &Program,
    layers: &[(TypeId, TypeId)],
    keys: &[DataInstanceKey; 2],
    source_array: TypeId,
) -> DataInstanceKey {
    let mut value = program
        .instantiate_components(
            layers[0].0,
            keys.iter()
                .cloned()
                .enumerate()
                .map(|(i, key)| (ValueField::Tuple(i), key)),
        )
        .unwrap();
    for &(source, _) in &layers[1..] {
        value = program
            .instantiate_components(
                source,
                [
                    (ValueField::Tuple(0), value.clone()),
                    (ValueField::Tuple(1), value),
                ],
            )
            .unwrap();
    }
    program
        .instantiate_components(source_array, [(ValueField::Array(0), value)])
        .unwrap()
}

#[test]
fn positive_domain_reprojection_preserves_every_context_and_exact_result_family() {
    use crate::value_access::InstanceSchema;
    let fixture = fixture();
    let program = &fixture.program;
    let data = program.function_data(fixture.captured).unwrap();
    let keys = fixture.callbacks.map(|callback| {
        let frame = program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap();
        program.instantiate_data(data, &frame).unwrap()
    });
    let domain = program
        .join_data_domains(&keys[0], &keys[1], || Ok::<_, ()>(()))
        .unwrap();
    let formal = program
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(fixture.formal),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let projected = domain.reproject(program, formal).unwrap();
    assert_eq!(projected.data(), formal);
    assert!(domain.has_same_representation(program, &projected));
    let InstanceSchema::FunctionAlternatives(original) = domain.schema(program).unwrap() else {
        panic!("source domain")
    };
    let InstanceSchema::FunctionAlternatives(received) = projected.schema(program).unwrap() else {
        panic!("received domain")
    };
    assert_eq!(original, received);
    assert_eq!(projected.reproject(program, data).unwrap(), domain);
    let factory = program
        .resolve_code(program.symbols().get("factory").unwrap())
        .unwrap();
    let result = program.codes().get(factory).unwrap().result();
    assert!(domain.reproject(program, result).is_err());
    let returned = domain
        .reproject_result(
            program,
            factory,
            &nocter_model::ValuePath::default(),
            result,
        )
        .unwrap();
    assert!(returned.has_same_representation(program, &domain));
    assert!(
        domain
            .reproject_result(
                program,
                fixture.creator,
                &nocter_model::ValuePath::default(),
                result
            )
            .is_err()
    );
    let concrete = program.function_data(fixture.callbacks[0]).unwrap();
    assert!(projected.reproject(program, concrete).is_err());
    let edit = program
        .resolve_code(program.symbols().get("edit").unwrap())
        .unwrap();
    let edit_data = program
        .parameters()
        .get(program.codes().get(edit).unwrap().parameters()[0])
        .unwrap()
        .data();
    assert!(projected.reproject(program, edit_data).is_err());
}
