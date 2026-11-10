use super::*;
use nocter_model::{ValueClosure, ValueFunctionOrigin};

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
                    element: Box::new(BuiltinType::I32.into()),
                    length: 4,
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
    let field = header.records().get(record).unwrap().fields()[0];
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
    let field = late.records().get(record).unwrap().fields()[0];
    let header_array = late.fields().get(field).unwrap().data();
    assert_eq!(
        inner, header_array,
        "the late header must reuse the raw slot"
    );
    let canonical = late
        .canonical_body_types(BTreeMap::from([(first, &a), (second, &b)]))
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
        owner: second,
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
        .canonical_body_types(BTreeMap::from([(first, &earlier), (second, &body)]))
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
    let function = header
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Declaration(first),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    for element in [
        header.types.store().builtin(BuiltinType::Never),
        header.types.store().builtin(BuiltinType::F64),
        function,
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
