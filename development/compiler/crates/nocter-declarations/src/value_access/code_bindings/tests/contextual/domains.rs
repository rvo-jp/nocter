//! Canonical domain union is metadata, separate from selecting a value or transporting it.
use super::*;
use crate::value_access::{DataDomainJoinFailure, InstanceSchema};
use nocter_model::ValueField;

#[test]
fn joins_retain_positive_contexts_and_explicit_exclusion_without_source_relabeling() {
    let fixture = fixture();
    let program = &fixture.program;
    let data = program.function_data(fixture.captured).unwrap();
    let keys = fixture.callbacks.map(|callback| {
        let frame = program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap();
        program.instantiate_data(data, &frame).unwrap()
    });
    let joined = program
        .join_data_domains(&keys[0], &keys[1], || Ok::<_, ()>(()))
        .unwrap();
    let reverse = program
        .join_data_domains(&keys[1], &keys[0], || Ok::<_, ()>(()))
        .unwrap();
    assert_eq!(joined, reverse);
    let InstanceSchema::FunctionAlternatives(choices) = joined.schema(program).unwrap() else {
        panic!("positive alternatives")
    };
    assert_eq!(choices.len(), 2);
    for key in &keys {
        let InstanceSchema::Function(actual) = key.schema(program).unwrap() else {
            panic!("original function")
        };
        assert!(choices.contains(&actual));
        assert!(!key.permits_guarded_transport(program, &joined));
    }
    assert_eq!(
        program
            .join_data_domains(&joined, &keys[0], || Ok::<_, ()>(()))
            .unwrap(),
        joined
    );
    assert_eq!(
        program
            .join_data_domains(&keys[1], &joined, || Ok::<_, ()>(()))
            .unwrap(),
        joined
    );
    let empty = program.instantiate_uninhabited_data(data).unwrap();
    assert_eq!(
        program
            .join_data_domains(&empty, &keys[0], || Ok::<_, ()>(()))
            .unwrap(),
        keys[0]
    );
    assert_eq!(
        program
            .join_data_domains(&joined, &empty, || Ok::<_, ()>(()))
            .unwrap(),
        joined
    );
    assert_eq!(
        program
            .join_data_domains(&empty, &empty, || Ok::<_, ()>(()))
            .unwrap(),
        empty
    );
    let wrong = program
        .instantiate_closed_data(program.data_type(BuiltinType::Void).unwrap())
        .unwrap();
    assert!(matches!(
        program.join_data_domains(&keys[0], &wrong, || Ok::<_, ()>(())),
        Err(DataDomainJoinFailure::Binding(
            CodeBindingError::InvalidWitness
        ))
    ));
}

#[test]
fn shared_maximum_array_domain_joins_are_bounded_and_retry_without_partial_publication() {
    let fixture = fixture_with_depth(24);
    let program = &fixture.program;
    let keys = fixture.callbacks.map(|callback| {
        let frame = program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap();
        program.instantiate_data(fixture.large, &frame).unwrap()
    });
    let original = keys.clone();
    let mut spent = 0;
    assert_eq!(
        program.join_data_domains(&keys[0], &keys[1], || {
            spent += 1;
            if spent > 1 { Err("work") } else { Ok(()) }
        }),
        Err(DataDomainJoinFailure::Work("work"))
    );
    assert_eq!(spent, 2);
    assert_eq!(keys, original);
    let mut spent = 0;
    let joined = program
        .join_data_domains(&keys[0], &keys[1], || {
            spent += 1;
            if spent > 512 { Err("work") } else { Ok(()) }
        })
        .unwrap();
    assert!(spent < 512);
    let total = spent;
    for limit in [5, 20, total - 1] {
        let mut spent = 0;
        assert_eq!(
            program.join_data_domains(&keys[0], &keys[1], || {
                spent += 1;
                if spent > limit {
                    Err("partial")
                } else {
                    Ok(())
                }
            }),
            Err(DataDomainJoinFailure::Work("partial"))
        );
        assert_eq!(spent, limit + 1);
        assert_eq!(keys, original);
    }
    let mut element = joined.component(program, ValueField::Array(0)).unwrap();
    for _ in 0..25 {
        element = element.component(program, ValueField::Tuple(0)).unwrap();
    }
    let InstanceSchema::FunctionAlternatives(choices) = element.schema(program).unwrap() else {
        panic!("positive leaf")
    };
    assert_eq!(choices.len(), 2);
    let repeated = program
        .join_data_domains(&keys[0], &keys[1], || Ok::<_, ()>(()))
        .unwrap();
    assert_eq!(joined, repeated);
    assert_eq!(keys, original);
}

#[test]
fn callable_case_projection_retains_order_code_view_and_nested_environment() {
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
    let InstanceSchema::FunctionAlternatives(actuals) = domain.schema(program).unwrap() else {
        panic!("domain")
    };
    let cases = domain.function_representations(program).unwrap().unwrap();
    for (actual, case) in actuals.iter().zip(cases.iter()) {
        let InstanceSchema::Function(selected) = case.schema(program).unwrap() else {
            panic!("case")
        };
        assert_eq!(&selected, actual);
        assert_eq!(
            case.data(),
            program.function_data(actual.witness()).unwrap()
        );
        assert_eq!(
            case.function_representation(program).unwrap().as_ref(),
            Some(case)
        );
        assert_eq!(
            case.function_representations(program)
                .unwrap()
                .unwrap()
                .as_ref(),
            std::slice::from_ref(case)
        );
    }
    assert_ne!(cases[0], cases[1]);
    assert_ne!(
        cases[0].environment(program).unwrap(),
        cases[1].environment(program).unwrap()
    );
    let empty = program.instantiate_uninhabited_data(data).unwrap();
    assert!(empty.function_representations(program).unwrap().is_none());
    let unit = program
        .instantiate_closed_data(program.data_type(BuiltinType::Void).unwrap())
        .unwrap();
    assert!(unit.function_representations(program).unwrap().is_none());
}

#[test]
fn value_injection_keeps_every_positive_case_and_the_stricter_relations_separate() {
    let fixture = fixture_with_depth(24);
    let program = &fixture.program;
    let keys = fixture.callbacks.map(|callback| {
        let frame = program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap();
        program.instantiate_data(fixture.large, &frame).unwrap()
    });
    let joined = program
        .join_data_domains(&keys[0], &keys[1], || Ok::<_, ()>(()))
        .unwrap();
    for source in &keys {
        assert!(source.permits_value_transport(program, &joined));
        assert!(!source.permits_guarded_transport(program, &joined));
        assert!(!source.has_same_representation(program, &joined));
        assert!(!joined.permits_value_transport(program, source));
    }
    assert!(joined.permits_value_transport(program, &joined));
    assert!(!keys[0].permits_value_transport(program, &keys[1]));
    let empty = program.instantiate_uninhabited_data(joined.data()).unwrap();
    assert!(!empty.permits_value_transport(program, &joined));
}

#[test]
fn closed_shared_representation_queries_reuse_schema_pairs_without_fresh_arc_expansion() {
    let (headers, [creator, ..]) = headers();
    let mut data = headers.data_type(BuiltinType::I32).unwrap();
    let mut body = headers.body_types(creator).unwrap();
    for _ in 0..48 {
        data = body.tuple(&[data, data]).unwrap();
    }
    let program = body.finish();
    let key = program.instantiate_closed_data(data).unwrap();
    assert!(key.has_same_representation(&program, &key));
    assert!(key.permits_guarded_transport(&program, &key));
    assert!(key.permits_value_transport(&program, &key));
}

#[test]
fn deep_closed_representation_comparisons_validate_the_schema_once() {
    let (headers, [creator, ..]) = headers();
    let mut data = headers.data_type(BuiltinType::I32).unwrap();
    let mut body = headers.body_types(creator).unwrap();
    for _ in 0..10_000 {
        data = body.optional(data).unwrap();
    }
    let program = body.finish();
    let key = program.instantiate_closed_data(data).unwrap();
    let independently_bound = program.instantiate_closed_data(data).unwrap();
    assert!(key.is_context_free());
    assert!(key.has_same_representation(&program, &independently_bound));
    assert!(key.permits_guarded_transport(&program, &independently_bound));
    assert!(key.permits_value_transport(&program, &independently_bound));
    // A key's Closed marker from another graph is not a certificate for this Program.
    assert!(program.data_schema(data).is_some());
    assert!(headers.data_schema(data).is_none());
    assert!(!key.has_same_representation(&headers, &independently_bound));
    assert!(!key.permits_guarded_transport(&headers, &independently_bound));
    assert!(!key.permits_value_transport(&headers, &independently_bound));
}

#[test]
fn equal_closed_markers_still_reject_unresolved_contexts_in_the_requested_program() {
    let fixture = fixture();
    assert!(fixture.program.data_schema(fixture.large).is_some());
    assert!(
        fixture
            .program
            .instantiate_closed_data(fixture.large)
            .is_err()
    );
    // Deliberately invalid metadata models a Closed key carried from an unrelated graph.
    let key = crate::value_access::DataInstanceKey {
        data: fixture.large,
        context: std::sync::Arc::new(crate::value_access::data_instances::Context::Closed),
    };
    assert!(!key.has_same_representation(&fixture.program, &key));
    assert!(!key.permits_guarded_transport(&fixture.program, &key));
    assert!(!key.permits_value_transport(&fixture.program, &key));
}

#[test]
fn closed_sibling_comparison_preserves_the_containing_callable_context() {
    let fixture = fixture_with_closed_sibling(1_000);
    let program = &fixture.program;
    let keys = fixture.callbacks.map(|callback| {
        let frame = program
            .bind_code(fixture.creator.into(), [(fixture.formal, callback)])
            .unwrap();
        program.instantiate_data(fixture.pair, &frame).unwrap()
    });
    assert!(!keys[0].is_context_free());
    for key in &keys {
        assert!(key.has_same_representation(program, key));
        assert!(key.permits_guarded_transport(program, key));
        assert!(key.permits_value_transport(program, key));
    }
    assert!(!keys[0].has_same_representation(program, &keys[1]));
    assert!(!keys[0].permits_guarded_transport(program, &keys[1]));
    assert!(!keys[0].permits_value_transport(program, &keys[1]));
}
