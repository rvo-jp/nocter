use super::*;
use nocter_model::{
    AccessExpressionId, ArenaBuilder, ValueApplication, ValueCode, ValueFunctionOrigin,
};

#[test]
fn result_ordinal_gaps_cannot_change_an_existing_calls_template_set() {
    use nocter_model::{BuiltinType, CompileTimeGuarantee, SymbolTable, ValueAppliedResult};
    let symbols = SymbolTable::from_spellings(["run", "before", "after"]);
    let mut reservations = Reservations::new(symbols.clone());
    let [root, before, after] =
        ["run", "before", "after"].map(|name| reservations.reserve(symbols.get(name).unwrap(), 0));
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            root,
            CodeKind::Function(CompileTimeGuarantee::Evaluatable),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let leaf = DataReference::Function(Box::new(FunctionDefinition {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: BuiltinType::I32.into(),
    }));
    let empty = DataReference::Array {
        element: std::sync::Arc::new(leaf.clone()),
        length: nocter_model::UsizeTerm::Value(0),
    };
    for (id, a, b) in [(before, empty.clone(), leaf.clone()), (after, leaf, empty)] {
        builder
            .define(
                id,
                CodeKind::Function(CompileTimeGuarantee::Evaluatable),
                [],
                DataReference::Tuple(TupleData::new(a, b, [])),
            )
            .unwrap();
    }
    let header = builder.finish().unwrap();
    let mut body = header.body_types(root).unwrap();
    let site = ValueApplication {
        owner: root.into(),
        call: ArenaBuilder::<AccessExpressionId, ()>::new().insert(()),
    };
    body.application(site.owner, site.call, before).unwrap();
    let original = ValueAppliedResult {
        application: site,
        ordinal: 1,
    };
    let template = body.program().application_template(original).unwrap();
    let types = body.program().types.store().type_count();
    assert_eq!(
        body.application(site.owner, site.call, after),
        Err(BodyTypeError::InvalidApplication)
    );
    assert_eq!(body.program().types.store().type_count(), types);
    assert_eq!(
        body.program().application_template(original),
        Some(template)
    );
    assert!(body.program().application_template(site.into()).is_none());
}

#[test]
fn application_templates_are_immutable_shape_facts_without_payload_or_copy_evidence() {
    let (header, [root, ..]) = application_headers();
    let mut body = header.body_types(root).unwrap();
    let code = body.reserve_closure().unwrap();
    let named = FunctionWitness::from(root);
    let result = header.function_data(named).unwrap();
    let void = header.data_type(BuiltinType::Void).unwrap();
    body.closure(
        code,
        void,
        FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result,
        },
    )
    .unwrap();
    let old = body.program().clone();
    let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
    let key = ValueApplication {
        owner: ValueCode::Declaration(root),
        call,
    };
    let data = body
        .application(
            key.owner,
            call,
            FunctionWitness {
                origin: ValueFunctionOrigin::Closure(code),
                invocation: ValueInvocation::Read,
            },
        )
        .unwrap();
    assert!(old.application_template(key.into()).is_none());
    assert_eq!(body.program().application_template(key.into()), Some(named));
    assert_eq!(
        body.program().payload_readiness(data),
        PayloadReadiness::Pending
    );
    assert_eq!(body.program().data_copyable(data), Some(false));
    assert_eq!(
        body.program().function_contract(FunctionWitness {
            origin: ValueFunctionOrigin::Application(key.into()),
            invocation: ValueInvocation::Read
        }),
        header.function_contract(named)
    );
    for ordinal in [1, usize::MAX] {
        let selected = nocter_model::ValueAppliedResult {
            application: key,
            ordinal,
        };
        assert!(body.program().application_template(selected).is_none());
        let witness = FunctionWitness {
            origin: ValueFunctionOrigin::Application(selected),
            invocation: ValueInvocation::Read,
        };
        assert!(body.program().function_contract(witness).is_none());
        assert!(body.program().function_data(witness).is_none());
    }
}

#[test]
fn deep_application_template_queries_are_iterative_and_reject_a_cycle() {
    let (header, [root, ..]) = application_headers();
    let mut body = header.body_types(root).unwrap();
    let mut calls = ArenaBuilder::<AccessExpressionId, ()>::new();
    let mut selected = FunctionWitness::from(root);
    for _ in 0..10_000 {
        let key = ValueApplication {
            owner: ValueCode::Declaration(root),
            call: calls.insert(()),
        };
        body.program.applications.insert(key.into(), selected);
        selected = FunctionWitness {
            origin: ValueFunctionOrigin::Application(key.into()),
            invocation: ValueInvocation::Read,
        };
    }
    assert_eq!(
        body.program().function_contract(selected),
        header.function_contract(root.into())
    );
    let ValueFunctionOrigin::Application(key) = selected.origin else {
        unreachable!()
    };
    body.program.applications.insert(key, selected);
    assert!(body.program().function_contract(selected).is_none());
}
