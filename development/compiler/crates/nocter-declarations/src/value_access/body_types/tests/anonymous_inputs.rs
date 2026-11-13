use super::*;
use nocter_model::{ValueClosure, ValueCode, ValueFunctionOrigin, ValueParameter};

#[test]
fn anonymous_formal_signatures_are_lexical_snapshot_facts_without_payload_or_header_slots() {
    let (header, [root, other, _]) = application_headers();
    let mut body = header.body_types(root).unwrap();
    let owner = body.reserve_closure().unwrap();
    let old = body.program().clone();
    let integer = header.data_type(BuiltinType::I32).unwrap();
    let void = header.data_type(BuiltinType::Void).unwrap();
    let contract = FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: vec![FunctionInput {
            data: integer,
            operation: nocter_model::ValueInputOperation::Read,
        }]
        .into(),
        result: integer,
    };
    let first = body.closure_parameter(owner, 0, contract.clone()).unwrap();
    let second = body.closure_parameter(owner, 1, contract.clone()).unwrap();
    let formal = ValueParameter::Closure { owner, position: 0 };
    assert!(old.parameter_contract(formal).is_none());
    assert!(header.parameter_contract(formal).is_none());
    assert!(body.program().closure(owner).is_none());
    assert_eq!(body.program().parameter_contract(formal), Some(&contract));
    assert_eq!(
        body.program().parameter_owner(formal),
        Some(ValueCode::Closure(owner))
    );
    assert_eq!(body.program().data_copyable(first), Some(false));
    assert_eq!(
        body.program().payload_readiness(first),
        PayloadReadiness::Unsupported
    );
    assert_ne!(first, second);
    assert_eq!(
        body.closure_parameter(owner, 0, contract.clone()),
        Err(BodyTypeError::InvalidExtension)
    );
    let foreign = ValueClosure {
        owner: other.into(),
        slot: owner.slot,
    };
    assert_eq!(
        body.closure_parameter(foreign, 0, contract.clone()),
        Err(BodyTypeError::InvalidExtension)
    );
    assert!(
        header
            .canonical_body_types(BTreeMap::from([(root.into(), body.program())]))
            .is_err()
    );
    let inputs = vec![first, second]
        .into_iter()
        .map(|data| FunctionInput {
            data,
            operation: nocter_model::ValueInputOperation::Read,
        })
        .collect();
    body.closure(
        owner,
        void,
        FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs,
            result: integer,
        },
    )
    .unwrap();
    assert_eq!(
        body.closure_parameter(owner, 2, contract.clone()),
        Err(BodyTypeError::InvalidExtension)
    );
    let body = body.finish();
    let canonical = header
        .canonical_body_types(BTreeMap::from([(root.into(), &body)]))
        .unwrap();
    let first = canonical.project(root, first).unwrap();
    let canonical = canonical.finish();
    let resolved = canonical
        .resolve_body_types(&BTreeMap::new())
        .unwrap()
        .finish();
    assert_eq!(resolved.parameter_contract(formal), Some(&contract));
    assert_eq!(
        resolved.function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(formal),
            invocation: ValueInvocation::Read
        }),
        Some(first)
    );
    assert!(resolved.preserves_headers(&header));
    assert_eq!(
        resolved.parameters().iter().count(),
        header.parameters().iter().count()
    );
}
