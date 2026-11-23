//! Structural tuples use the ordinary supply, partial-place and required-evaluation boundaries.
use super::support::steps;
use super::*;
use nocter_model::{BuiltinType, ValueField};

#[test]
fn contextual_component_transfer_neither_implies_copy_nor_hides_retired_storage() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-transfers.nct"
    ));
    for (original, replacement, rule) in [
        (
            "let action = pair.0^",
            "let action = pair.0",
            Some(crate::value_access::Rule::MissingTransfer),
        ),
        (
            "let action = values[0]^",
            "let action = values[0]",
            Some(crate::value_access::Rule::MissingTransfer),
        ),
        (
            "action(sibling)",
            "let _ = pair.0(0)\n    action(sibling)",
            None,
        ),
    ] {
        let changed = source.replace(original, replacement);
        assert_ne!(changed, source);
        let (sources, tree) = input(&changed);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        match rule {
            Some(rule) => assert!(
                matches!(&failure, SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(error), .. }) if error.rule() == rule),
                "{failure:?}"
            ),
            None => assert!(
                matches!(failure, SourceFailure::Checking(Failure::Flow { .. })),
                "{failure:?}"
            ),
        }
    }
}

#[test]
fn inferred_and_annotated_tuple_shapes_share_one_identity_and_structural_copy() {
    let unit = accepted(
        "func inspect(value: (i32, bool)): i32 { if value.1 { value.0 } else { 0 } }\nfunc run(): i32 { var pair = (40, true)\nlet snapshot: ((i32, bool)) = pair\npair.0 = 0\nlet nested = (snapshot, ((), 2))\ninspect(nested.0) + nested.1.1 }",
    );
    let pair = named(&unit, "inspect")
        .typed()
        .body()
        .locals()
        .iter()
        .next()
        .unwrap()
        .1
        .data;
    assert_eq!(unit.program().data_copyable(pair), Some(true));
    let shape = unit
        .program()
        .component(pair, ValueField::Tuple(1))
        .unwrap();
    assert_eq!(unit.program().component_position(shape.field), Some(1));
    assert_eq!(
        shape.data,
        unit.program().data_type(BuiltinType::Bool).unwrap()
    );
}

#[test]
fn tuple_components_preserve_noncopy_moves_reverse_cleanup_and_restoration() {
    let unit = accepted(
        "func run(take first: error, take second: error, take replacement: error): error {\nvar pair = (first^, second^)\nlet saved = pair.0^\npair.0 = replacement^\nsaved^\n}",
    );
    let trace = steps(named(&unit, "run"));
    assert!(trace.iter().any(|step| matches!(step, Step::Assemble { fields, .. } if fields.iter().map(|(field, _)| *field).collect::<Vec<_>>() == [ValueField::Tuple(0), ValueField::Tuple(1)])));
    let unit = accepted(
        "func run(take first: error, take second: error): error {\nlet pair = (first^, second^)\npair.0^\n}",
    );
    let trace = steps(named(&unit, "run"));
    assert!(trace.iter().any(|step| matches!(step, Step::Dispose { source, payload } if source.fields().concrete().unwrap().fields() == [ValueField::Tuple(1)] && payload.disposal() == Disposal::OwnedError)));
    assert!(!trace.iter().any(|step| matches!(step, Step::Dispose { source, .. } if source.fields().concrete().unwrap().fields() == [ValueField::Tuple(0)])));
}

#[test]
fn tuple_pattern_forms_its_named_owner_before_discarding_the_ordered_remainder() {
    let unit = accepted(
        "func run(take first: error, take middle: error, take last: error): error {\n\
         let ((kept, _), _) = ((first^, middle^), last^)\nkept^\n}",
    );
    let body = named(&unit, "run");
    let kept = body
        .typed()
        .body()
        .locals()
        .iter()
        .find_map(|(id, local)| {
            (local.origin == crate::value_access::LocalOrigin::Binding
                && unit.program().data_kind(local.data) == Some(BuiltinType::Error))
            .then_some(Holder::Local(id))
        })
        .unwrap();
    let trace = steps(body);
    let (bound, supplied) = trace
        .iter()
        .enumerate()
        .find_map(|(position, step)| match step {
            Step::Move {
                source,
                destination,
                ..
            } if destination.holder() == kept => Some((position, source.holder())),
            _ => None,
        })
        .unwrap();
    assert!(trace[..bound].iter().any(|step| matches!(step,
        Step::Move { source, destination, .. }
        if destination.holder() == supplied
            && source.fields().concrete().unwrap().fields() == [ValueField::Tuple(0), ValueField::Tuple(0)]
    )));
    let mut retired = Vec::new();
    for (position, step) in trace.iter().enumerate() {
        if let Step::Dispose { source, payload } = step
            && payload.disposal() == Disposal::OwnedError
        {
            assert!(position > bound);
            retired.push(source.fields().concrete().unwrap().fields().to_vec());
        }
    }
    assert_eq!(
        retired,
        [
            vec![ValueField::Tuple(1)],
            vec![ValueField::Tuple(0), ValueField::Tuple(1)],
        ]
    );
}

#[test]
fn tuple_rejects_unmarked_noncopy_alias_extraction_holes_and_wrong_selectors() {
    for body in [
        "let pair = (first^, second^); let saved = pair",
        "let pair = (first, 1)",
        "read pair = input; let saved = pair.0^",
        "let pair = (first^, second^); let saved = pair.0^; let next = pair^",
        "let pair = (1, 2); pair.2",
        "let pair = (1, 2); pair.first",
        "let pair: (i32, bool) = (1, 2)",
        "let pair: (i32, i32, i32) = (1, 2)",
    ] {
        let source = format!(
            "func run(take first: error, take second: error, input: (error, error)): void {{ {} }}",
            body.replace(';', "\n")
        );
        let (sources, tree) = input(&source);
        assert!(!tree.has_errors(), "{source}: {:?}", tree.diagnostics());
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(
            matches!(
                Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
                Err(Failure::Body { .. } | Failure::Flow { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn tuple_constants_and_statics_share_evaluation_without_granting_nominal_copy() {
    let unit = accepted(
        "struct Token { value: i32 }\nconst DATA: (Token, (bool, i32)) = (Token { value: 40 }, (false, 2))\nstatic STATIC: (Token, i32) = (Token { value: 41 }, 1)\nconst func calculate(): i32 { let pair = DATA\nif DATA.1.0 { drop pair }\nread view = STATIC.0\npair.0.value + pair.1.1 + view.value - STATIC.1 - 40 }\nconst ANSWER: i32 = calculate()\nfunc run(): i32 { ANSWER }",
    );
    let answer = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer),
        Some(&nocter_constant_evaluation::state::Value::integer(42).freeze())
    );
    let data = unit
        .program()
        .resolve_constant(unit.program().symbols().get("DATA").unwrap())
        .unwrap();
    assert_eq!(
        unit.program()
            .data_copyable(unit.program().codes().get(data).unwrap().result()),
        Some(false)
    );
}

#[test]
fn tuples_in_sibling_bodies_and_capture_bodies_replay_their_component_types() {
    let unit = accepted(
        "func alpha(): i32 { let pair = (true, (1, 2))\npair.1.0 }\nfunc beta(): i32 { let pair = (1, false)\npair.0 }\nfunc run(): i32 { let captured = (true, 40)\nlet action = (captured;): i32 { let nested = (captured, (1, 1))\nif nested.0.0 { nested.0.1 + nested.1.0 + nested.1.1 } else { 0 } }\naction() }",
    );
    assert_eq!(unit.closures().len(), 1);
    for (_, query) in unit.bodies() {
        for (_, place) in query.typed().body().places().iter() {
            let root = query.typed().body().locals().get(place.local).unwrap().data;
            assert_eq!(
                unit.program().project_data(root, &place.fields),
                Some(place.data)
            );
        }
    }
}

#[test]
fn tuple_interruption_cleans_completed_components_without_assembling_a_value() {
    let unit = accepted(
        "func run(take first: error, take second: error, take answer: error): error {\nlet pair: (error, error, i32) = (first^, second^, if true { return answer^ } else { 0 })\nanswer^\n}",
    );
    let trace = steps(named(&unit, "run"));
    assert!(
        !trace
            .iter()
            .any(|step| matches!(step, Step::Assemble { .. }))
    );
    let components = named(&unit, "run")
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| {
            if let crate::value_access::ExpressionKind::Aggregate { fields, .. } = &expression.kind
            {
                Some(fields)
            } else {
                None
            }
        })
        .unwrap();
    let cleaned = trace
        .iter()
        .filter_map(|step| match step {
            Step::Dispose { source, payload } if payload.disposal() == Disposal::OwnedError => {
                Some(source.holder())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cleaned,
        [
            Holder::Expression(components[1].value),
            Holder::Expression(components[0].value)
        ]
    );
}

#[test]
fn record_tuple_composition_keeps_nominal_copy_and_mixed_projection_keys() {
    accepted(
        "copy struct Wrapped { pair: (i32, (bool, i32)) }\nfunc run(): i32 { var value = Wrapped { pair: (20, (true, 22)) }\nlet snapshot = value\nvalue.pair.0 = 0\nif snapshot.pair.1.0 { snapshot.pair.0 + snapshot.pair.1.1 } else { 0 } }",
    );
    for source in [
        "struct Wrapped { pair: (i32, Missing) }",
        "struct Wrapped { pair: (bool, Wrapped) }",
        "struct Item {}\ncopy struct Wrapped { pair: (Item, i32) }",
    ] {
        let (sources, tree) = input(source);
        assert!(!tree.has_errors(), "{source}: {:?}", tree.diagnostics());
        assert!(
            matches!(
                declarations::Query::prepare(bound(&sources, &tree)),
                Err(declarations::Failure::Headers(_))
            ),
            "{source}"
        );
    }
}

#[test]
fn inferred_tuple_components_snapshot_before_later_initializer_writes() {
    let unit = accepted(
        "const func increment(edit value: i32): i32 { value += 1\nvalue }\nconst func calculate(): i32 { var value = 20\nlet pair = (value, increment(value))\npair.0 + pair.1 + 1 }\nconst ANSWER: i32 = calculate()",
    );
    let id = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(id),
        Some(&nocter_constant_evaluation::state::Value::integer(42).freeze())
    );
}

#[test]
fn shared_tuple_diagnostics_bound_rendering_without_expanding_the_type_tree() {
    use std::fmt::Write as _;
    let mut source = String::from("func run(): void { let value0 = (1, 2)\n");
    for depth in 1..64 {
        writeln!(
            source,
            "let value{depth} = (value{}, value{})",
            depth - 1,
            depth - 1
        )
        .unwrap();
    }
    source.push_str("value63.missing\n}");
    let (sources, tree) = input(&source);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let Err(Failure::Body {
        failure: BodyFailure::Rejected(diagnostic),
        ..
    }) = Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap())
    else {
        panic!("a missing tuple component needs a source error");
    };
    assert_eq!(diagnostic.rule(), crate::value_access::Rule::UnknownField);
    assert!(diagnostic.source().message().contains('…'));
    assert!(diagnostic.source().message().len() < 1024);
}

#[test]
fn interrupted_tuple_components_use_expected_data_without_manufacturing_a_value() {
    let unit = accepted(
        "func run(): i32 { let absent: (i32, bool) = (if true { return 42 } else { return 43 }, false) }",
    );
    assert!(
        !steps(named(&unit, "run"))
            .iter()
            .any(|step| matches!(step, Step::Assemble { .. }))
    );
    for source in [
        "func run(): i32 { let absent = (if true { return 42 } else { return 43 }, false) }",
        "func run(): i32 { let absent: (i32, bool) = (if true { return 42 } else { return 43 }, 1) }",
    ] {
        let (sources, tree) = input(source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(
            matches!(
                Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
                Err(Failure::Body {
                    failure: BodyFailure::Rejected(_),
                    ..
                })
            ),
            "{source}"
        );
    }
}

#[test]
fn tuple_formal_callable_contract_does_not_supply_copy_evidence() {
    let text = "func run(callback: func(): void): void { let pair = (callback, 1) }";
    let (sources, tree) = input(text);
    assert!(matches!(SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. }))
        if error.rule() == crate::value_access::Rule::MissingTransfer));
}
