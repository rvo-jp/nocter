//! Only positive environment payload edges can close a result-induced containment cycle.
use super::*;
use nocter_model::{AccessExpressionId, ArenaBuilder, ValueCode, ValueFunctionOrigin as Origin};

fn contract(program: &Program) -> FunctionConstraint {
    FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: program.data_type(BuiltinType::Void).unwrap(),
    }
}

#[test]
fn result_induced_self_containment_rejects_atomically_but_empty_edges_break_it() {
    for count in [None, Some(0), Some(1), Some(u64::MAX)] {
        let (header, [root, factory, _]) = application_headers();
        let mut body = header.body_types(root).unwrap();
        let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
        let pending = body
            .application(ValueCode::Declaration(root), call, factory)
            .unwrap();
        let Some(PayloadSchema::Function(source)) = body.program().data_schema(pending) else {
            panic!("application")
        };
        let environment = count.map_or(pending, |count| body.array(pending, count).unwrap());
        let id = body.reserve_closure().unwrap();
        let data = body.closure(id, environment, contract(&header)).unwrap();
        let Some(PayloadSchema::Function(target)) = body.program().data_schema(data) else {
            panic!("capture")
        };
        let body = body.finish();
        let canonical = header
            .canonical_body_types(BTreeMap::from([(root.into(), &body)]))
            .unwrap();
        let environment = canonical.project(root, environment).unwrap();
        let data = canonical.project(root, data).unwrap();
        let program = canonical.finish();
        let resolved = program.resolve_body_types(&BTreeMap::from([(source, target)]));
        if count == Some(0) {
            let resolved = resolved.unwrap();
            let data = resolved.program().function_data(target).unwrap();
            assert_eq!(resolved.program().data_copyable(data), Some(true));
            assert!(resolved.program().owned_payload_supported(data));
            let Some(PayloadSchema::Array(array)) = resolved
                .program()
                .data_schema(resolved.program().closure(id).unwrap().environment())
            else {
                panic!("empty environment")
            };
            assert_eq!(array.element(), data);
        } else {
            assert!(
                matches!(resolved, Err(BodyTypeError::RecursiveEnvironment(found)) if found == id)
            );
            let named = FunctionWitness {
                origin: Origin::Declaration(root),
                invocation: ValueInvocation::Read,
            };
            let retried = program
                .resolve_body_types(&BTreeMap::from([(source, named)]))
                .unwrap();
            assert_eq!(
                retried
                    .program()
                    .data_copyable(retried.program().function_data(target).unwrap()),
                Some(true)
            );
        }
        assert_eq!(program.closure(id).unwrap().environment(), environment);
        assert_eq!(program.function_data(target), Some(data));
    }
}

#[test]
fn mutual_environment_containment_uses_the_same_positive_edge_policy() {
    for second_count in [0, 1] {
        let (header, [root, first_factory, second_factory]) = application_headers();
        let mut body = header.body_types(root).unwrap();
        let mut calls = ArenaBuilder::<AccessExpressionId, ()>::new();
        let first = body
            .application(
                ValueCode::Declaration(root),
                calls.insert(()),
                first_factory,
            )
            .unwrap();
        let second = body
            .application(
                ValueCode::Declaration(root),
                calls.insert(()),
                second_factory,
            )
            .unwrap();
        let Some(PayloadSchema::Function(first_source)) = body.program().data_schema(first) else {
            panic!("application")
        };
        let Some(PayloadSchema::Function(second_source)) = body.program().data_schema(second)
        else {
            panic!("application")
        };
        let first_id = body.reserve_closure().unwrap();
        let first = body.closure(first_id, first, contract(&header)).unwrap();
        let second_id = body.reserve_closure().unwrap();
        let second_environment = body.array(second, second_count).unwrap();
        let second = body
            .closure(second_id, second_environment, contract(&header))
            .unwrap();
        let Some(PayloadSchema::Function(first_target)) = body.program().data_schema(first) else {
            panic!("capture")
        };
        let Some(PayloadSchema::Function(second_target)) = body.program().data_schema(second)
        else {
            panic!("capture")
        };
        let body = body.finish();
        let program = header
            .canonical_body_types(BTreeMap::from([(root.into(), &body)]))
            .unwrap()
            .finish();
        let resolved = program.resolve_body_types(&BTreeMap::from([
            (first_source, second_target),
            (second_source, first_target),
        ]));
        if second_count == 0 {
            let resolved = resolved.unwrap();
            assert_eq!(
                resolved
                    .program()
                    .data_copyable(resolved.program().function_data(first_target).unwrap()),
                Some(true)
            );
        } else {
            assert!(matches!(
                resolved,
                Err(BodyTypeError::RecursiveEnvironment(_))
            ));
        }
    }
}

#[test]
fn deep_shared_environment_closure_uses_query_frames_without_expanding_payload_paths() {
    let (header, [root, _, _]) = application_headers();
    let mut body = header.body_types(root).unwrap();
    let ids = (0..1024)
        .map(|_| body.reserve_closure().unwrap())
        .collect::<Vec<_>>();
    let mut data = header.data_type(BuiltinType::Void).unwrap();
    for id in ids.into_iter().rev() {
        let shared = body.tuple(&[data, data]).unwrap();
        data = body.closure(id, shared, contract(&header)).unwrap();
    }
    let body = body.finish();
    let canonical = header
        .canonical_body_types(BTreeMap::from([(root.into(), &body)]))
        .unwrap();
    let data = canonical.project(root, data).unwrap();
    let resolved = canonical
        .finish()
        .resolve_body_types(&BTreeMap::new())
        .unwrap();
    assert!(matches!(
        resolved.program().data_schema(data),
        Some(PayloadSchema::Function(FunctionWitness {
            origin: Origin::Closure(_),
            ..
        }))
    ));
    assert_eq!(resolved.program().data_copyable(data), Some(true));
}
