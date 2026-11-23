use super::*;
use nocter_model::{GenericApplication, UsizeTerm, ValueClosure, ValueFunctionOrigin};
use std::collections::BTreeSet;

fn generic_array_signature() -> (Program, AccessCodeId, UsizeTerm) {
    use crate::GenericParameterDomain::{Type, UsizeConstant};
    use nocter_model::ValueInputOperation;
    use std::sync::Arc;

    let symbols = SymbolTable::from_spellings(["decode", "T", "N", "parts"]);
    let name = |text| symbols.get(text).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let code = reservations.reserve(name("decode"), 1);
    reservations
        .reserve_callable_generics(code, vec![(name("T"), Type), (name("N"), UsizeConstant)])
        .unwrap();
    let mut headers = reservations.finish().unwrap().into_publication();
    let slots = headers.snapshot();
    let &[element, count] = slots.generic_parameter_ids(code).unwrap() else {
        panic!("expected two ordered binders");
    };
    let length = UsizeTerm::Parameter(count);
    let parameter = Arc::new(DataReference::GenericParameter(element));
    headers
        .define_generic(
            code,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                binding: ParameterBinding::Name(name("parts")),
                data: DataReference::Array {
                    element: parameter.clone(),
                    length,
                }
                .into(),
                operation: ValueInputOperation::Take,
            }],
            DataReference::Fallible(parameter),
        )
        .unwrap();
    (headers.finish().unwrap(), code, length)
}

#[test]
fn applied_signature_reuses_concrete_body_types_and_keeps_input_operations() {
    use nocter_model::{GenericValue, ValueInputOperation};
    let (header, code, length) = generic_array_signature();
    let mut body = header.body_types(code).unwrap();
    let integer = header.data_type(BuiltinType::I32).unwrap();
    let result = body.fallible(integer).unwrap();
    let count_four = GenericValue::Usize(4.into());
    let before = body.program().types.store().cursor();
    for values in [
        vec![integer.into()],
        vec![count_four, integer.into()],
        vec![integer.into(), length.into()],
        vec![result.into(), count_four],
    ] {
        assert_eq!(
            body.apply_callable_signature(code, &GenericApplication::new(values)),
            Err(BodyTypeError::InvalidApplication)
        );
        assert_eq!(body.program().types.store().cursor(), before);
    }
    let applied = body
        .apply_callable_signature(code, &GenericApplication::new([integer.into(), count_four]))
        .unwrap();
    assert_eq!(applied.result, result);
    assert_eq!(applied.inputs[0].operation, ValueInputOperation::Take);
    assert_eq!(applied.inputs[0].data, body.array(integer, 4).unwrap());
    let body = body.finish();
    assert!(
        body.function_contract(FunctionWitness::from(code))
            .is_none()
    );
    let canonical = header
        .canonical_body_types(BTreeMap::from([(code.into(), &body)]))
        .unwrap();
    let data = canonical.project(code, applied.inputs[0].data).unwrap();
    let canonical = canonical.finish();
    assert_eq!(array(&canonical, data).length(), 4);
}

#[test]
fn exact_frames_project_body_types_with_the_selected_length() {
    let (header, owner, length) = generic_array_signature();
    let integer = header.data_type(BuiltinType::I32).unwrap();
    let generic_input = header.codes().get(owner).unwrap().parameters()[0];
    let generic_input = header.parameters().get(generic_input).unwrap().data();
    let application =
        |count| GenericApplication::new([integer.into(), UsizeTerm::Value(count).into()]);
    let mut body = header.body_types(owner).unwrap();
    let local = body.array(integer, length).unwrap();
    let body = body.finish();
    let canonical = header
        .canonical_body_types(BTreeMap::from([(owner.into(), &body)]))
        .unwrap();
    let local = canonical.project(owner, local).unwrap();
    let program = canonical.finish();
    assert_eq!(
        program.bind_generic_call(owner, &application(4)),
        Err(CodeBindingError::InvalidGenericApplication(owner))
    );
    let program = program
        .project_generic_frames(BTreeMap::from([
            ((owner, application(4)), BTreeSet::from([local])),
            ((owner, application(7)), BTreeSet::from([local])),
        ]))
        .unwrap();
    let first = program.bind_generic_call(owner, &application(4)).unwrap();
    let second = program.bind_generic_call(owner, &application(7)).unwrap();
    assert_eq!(first.code(), second.code());
    assert_ne!(first, second);
    for (frame, count) in [(first, 4), (second, 7)] {
        let data = program.instantiate_data(local, &frame).unwrap().data();
        assert_eq!(program.frame_type(&frame, generic_input), Ok(data));
        assert_eq!(array(&program, data).length(), count);
    }
    assert!(program.instantiate_closed_data(local).is_err());
}

#[test]
fn symbolic_array_suffix_replays_only_its_own_length_parameter() {
    use nocter_model::{TypeKind, UsizeTerm};
    let symbols = SymbolTable::from_spellings(["first", "second", "N"]);
    let name = |text| symbols.get(text).unwrap();
    let mut reservations = Reservations::new(symbols.clone());
    let first = reservations.reserve(name("first"), 0);
    let second = reservations.reserve(name("second"), 0);
    for code in [first, second] {
        reservations
            .reserve_callable_generics(
                code,
                vec![(name("N"), crate::GenericParameterDomain::UsizeConstant)],
            )
            .unwrap();
    }
    let mut headers = reservations.finish().unwrap().into_publication();
    for code in [first, second] {
        headers
            .define_generic(
                code,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                [],
                BuiltinType::Void,
            )
            .unwrap();
    }
    let program = headers.finish().unwrap();
    let length = |code| UsizeTerm::Parameter(program.generic_parameter_ids(code).unwrap()[0]);
    let data = program.data_type(BuiltinType::I32).unwrap();
    let mut body = program.body_types(first).unwrap();
    assert_eq!(
        body.array(data, length(second)),
        Err(BodyTypeError::InvalidExtension)
    );
    let array = body.array(data, length(first)).unwrap();
    let body = body.finish();
    let canonical = program
        .canonical_body_types(BTreeMap::from([(first.into(), &body)]))
        .unwrap();
    let array = canonical.project(first, array).unwrap();
    let canonical = canonical.finish();
    assert_eq!(
        canonical.type_kind(array),
        Some(&TypeKind::FixedArray {
            element: data,
            length: length(first)
        })
    );
    assert!(canonical.data_schema(array).is_none());
}

fn headers() -> (HeaderPublication, AccessCodeId, AccessCodeId, NominalTypeId) {
    let symbols = SymbolTable::from_spellings(["Data", "values", "first", "second"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Data").unwrap(), 1);
    let first = reservations.reserve(symbols.get("first").unwrap(), 0);
    let second = reservations.reserve(symbols.get("second").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            record,
            false,
            [FieldDefinition {
                name: symbols.get("values").unwrap(),
                data: DataReference::Array {
                    element: std::sync::Arc::new(BuiltinType::I32.into()),
                    length: nocter_model::UsizeTerm::Value(4),
                },
            }],
        )
        .unwrap();
    for code in [first, second] {
        builder
            .define(
                code,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                [],
                BuiltinType::Void,
            )
            .unwrap();
    }
    (builder.into_publication(), first, second, record)
}

fn array(program: &Program, data: TypeId) -> ArraySchema {
    let Some(PayloadSchema::Array(array)) = program.data_schema(data) else {
        panic!("fixed-array descriptor");
    };
    array
}

#[test]
fn body_arrays_share_header_identity_and_keep_cardinality_out_of_metadata() {
    let (headers, first, _, record) = headers();
    let header = headers.finish().unwrap();
    let integer = header.data_type(BuiltinType::I32).unwrap();
    let error = header.data_type(BuiltinType::Error).unwrap();
    let field = header.record(record).unwrap().fields()[0];
    let mut body = header.body_types(first).unwrap();
    let same = body.array(integer, 4).unwrap();
    assert_eq!(same, header.fields().get(field).unwrap().data());
    assert_eq!(same, body.array(integer, 4).unwrap());
    assert_ne!(same, body.array(integer, 3).unwrap());
    assert_ne!(same, body.tuple(&[integer; 4]).unwrap());
    let empty = body.array(error, 0).unwrap();
    let positive = body.array(error, 1).unwrap();
    let huge = body.array(empty, u64::MAX).unwrap();
    let body = body.finish();
    assert_eq!(body.data_copyable(empty), Some(true));
    assert_eq!(body.data_copyable(positive), Some(false));
    assert_eq!(body.data_copyable(huge), Some(true));
    assert_eq!(array(&body, huge).element(), empty);
    assert_eq!(array(&body, huge).length(), u64::MAX);
    assert_eq!(array(&body, empty).element(), error);
    assert!(
        body.component(huge, nocter_model::ValueField::Tuple(0))
            .is_none()
    );
    assert!(header.data_schema(huge).is_none());
}

#[test]
fn array_replay_remaps_nested_elements_after_late_headers_and_sibling_suffixes() {
    let (mut headers, first, second, record) = headers();
    headers.publish(first).unwrap();
    headers.publish(second).unwrap();
    let early = headers.snapshot();
    let boolean = early.data_type(BuiltinType::Bool).unwrap();
    let integer = early.data_type(BuiltinType::I32).unwrap();
    let mut a = early.body_types(first).unwrap();
    let inner = a.array(boolean, 3).unwrap();
    let tuple = a.tuple(&[inner, integer]).unwrap();
    let outer = a.array(tuple, u64::MAX).unwrap();
    let a = a.finish();
    let mut b = early.body_types(second).unwrap();
    let different = b.array(integer, 3).unwrap();
    let equal = b.array(boolean, 3).unwrap();
    let b = b.finish();
    assert_eq!(inner, different, "sibling raw positions must collide");
    let late = headers.finish().unwrap();
    let field = late.record(record).unwrap().fields()[0];
    let header_array = late.fields().get(field).unwrap().data();
    assert_eq!(
        inner, header_array,
        "the late header must reuse the raw slot"
    );
    let canonical = late
        .canonical_body_types(BTreeMap::from([(first.into(), &a), (second.into(), &b)]))
        .unwrap();
    let projected_inner = canonical.project(first, inner).unwrap();
    let projected_tuple = canonical.project(first, tuple).unwrap();
    let projected_outer = canonical.project(first, outer).unwrap();
    let projected_different = canonical.project(second, different).unwrap();
    assert_eq!(projected_inner, canonical.project(second, equal).unwrap());
    assert_ne!(projected_inner, projected_different);
    assert_ne!(projected_inner, header_array);
    let canonical = canonical.finish();
    assert_eq!(array(&canonical, projected_inner).element(), boolean);
    assert_eq!(array(&canonical, projected_different).element(), integer);
    assert_eq!(array(&canonical, projected_outer).length(), u64::MAX);
    assert_eq!(
        array(&canonical, projected_outer).element(),
        projected_tuple
    );
    let Some(PayloadSchema::Aggregate(schema)) = canonical.data_schema(projected_tuple) else {
        panic!("tuple containing a replayed array");
    };
    assert_eq!(
        schema.components().map(|c| c.data).collect::<Vec<_>>(),
        [projected_inner, integer]
    );
    assert_eq!(array(&canonical, header_array).length(), 4);
    assert_eq!(array(&canonical, header_array).element(), integer);
}

#[test]
fn array_closure_slots_replay_without_confusing_capture_copy_with_call_contracts() {
    let (headers, first, second, _) = headers();
    let header = headers.finish().unwrap();
    let mut body = header.body_types(second).unwrap();
    let error = header.data_type(BuiltinType::Error).unwrap();
    let integer = header.data_type(BuiltinType::I32).unwrap();
    let environment = body.array(error, 0).unwrap();
    let input = body.array(error, 1).unwrap();
    let tuple = body.tuple(&[environment, integer]).unwrap();
    let result = body.array(tuple, u64::MAX).unwrap();
    let id = ValueClosure {
        owner: second.into(),
        slot: 0,
    };
    let data = body
        .closure(
            id,
            environment,
            FunctionConstraint {
                invocation: ValueInvocation::Take,
                inputs: Box::new([FunctionInput {
                    operation: ValueInputOperation::Edit,
                    data: input,
                }]),
                result,
            },
        )
        .unwrap();
    let body = body.finish();
    let mut earlier = header.body_types(first).unwrap();
    earlier.array(integer, 5).unwrap();
    let earlier = earlier.finish();
    let canonical = header
        .canonical_body_types(BTreeMap::from([
            (first.into(), &earlier),
            (second.into(), &body),
        ]))
        .unwrap();
    let projected = |data| canonical.project(second, data).unwrap();
    assert_ne!(
        projected(environment),
        environment,
        "exercise a real suffix remap"
    );
    let (data, environment, input, result) = (
        projected(data),
        projected(environment),
        projected(input),
        projected(result),
    );
    let canonical = canonical.finish();
    let closure = canonical.closure(id).unwrap();
    assert_eq!(closure.environment(), environment);
    assert_eq!(closure.contract().inputs[0].data, input);
    assert_eq!(closure.contract().result, result);
    assert_eq!(canonical.data_copyable(data), Some(true));
    assert_eq!(canonical.data_copyable(input), Some(false));
    assert_eq!(
        canonical.function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Closure(id),
            invocation: ValueInvocation::Take
        }),
        Some(data)
    );
}

#[test]
fn invalid_array_elements_reject_even_at_zero_without_changing_the_type_branch() {
    let (headers, first, _, _) = headers();
    let header = headers.finish().unwrap();
    let mut body = header.body_types(first).unwrap();
    let mut untouched = body.clone();
    for element in [
        header.types.store().builtin(BuiltinType::Never),
        header.types.store().builtin(BuiltinType::F64),
    ] {
        for length in [0, 1, u64::MAX] {
            assert_eq!(
                body.array(element, length),
                Err(BodyTypeError::InvalidExtension)
            );
        }
    }
    let integer = header.data_type(BuiltinType::I32).unwrap();
    assert_eq!(body.array(integer, 2), untouched.array(integer, 2));
}
