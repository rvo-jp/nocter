//! Constructed callable outcomes retain original result families and concrete instance domains.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value;

#[test]
fn compound_received_optional_domains_preserve_adjacent_live_components() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-compound-outcome-control.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn nested_received_optional_domains_preserve_each_original_layer() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-nested-outcome-control.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn received_recovery_merges_preserve_shared_witness_payloads() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-recovery-merge.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn received_outcome_control_uses_each_actual_input_domain() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/received-outcome-control.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn excluded_result_reprojection_is_not_a_result_family_receipt() {
    use crate::value_access::ExpressionKind;
    use nocter_model::{ValueApplication, ValueField, ValueFunctionOrigin as Origin};
    let unit = accepted(
        "func forward(take value: (func(i32): i32)?): (func(i32): i32)? { value^ }\nfunc other(): (func(i32): i32)? { none }\nfunc run(): void { let value = forward(none)\ndrop value }",
    );
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let caller = program.bind_code(code("run").into(), []).unwrap();
    let call = named(&unit, "run").typed().body().expressions().iter().find_map(|(call, expression)| matches!(expression.kind, ExpressionKind::Call { target: Origin::Declaration(target), .. } if target == code("forward")).then_some(call)).unwrap();
    let budget = || {
        nocter_constant_evaluation::state::WorkBudget::new(
            std::num::NonZeroU64::new(100_000).unwrap(),
        )
    };
    let frame = unit
        .instantiate_invocation(
            &caller,
            ValueApplication {
                owner: caller.code(),
                call,
            },
            &mut budget(),
        )
        .unwrap();
    let wanted = program.codes().get(code("forward")).unwrap().result();
    let result = unit
        .instantiate_result_data(&frame, wanted, &mut budget())
        .unwrap();
    assert!(
        result
            .component(program, ValueField::Variant(true))
            .unwrap()
            .is_uninhabited()
    );
    let foreign = program.codes().get(code("other")).unwrap().result();
    let received = unit
        .instantiate_result_data(&frame, foreign, &mut budget())
        .unwrap();
    assert!(received.has_same_representation(program, &result));
    assert!(
        result
            .reproject_result(
                program,
                code("forward"),
                &nocter_model::ValuePath::default(),
                foreign
            )
            .is_err()
    );
    assert_eq!(
        unit.instantiate_result_data(&frame, wanted, &mut budget())
            .unwrap(),
        result
    );
}

#[test]
fn result_context_projection_requires_the_original_declaration_and_leaf() {
    use nocter_constant_evaluation::state::WorkBudget;
    use nocter_declarations::value_access::FunctionWitness;
    use nocter_model::{
        ValueApplication, ValueAppliedResult, ValueField, ValueFunctionOrigin, ValueInvocation,
        ValuePath, ValueProjection,
    };
    let unit = accepted(
        "func wrong(value: u64): u64 { value }\nfunc first(value: i32): i32 { value }\nfunc second(value: i32): i32 { value + 1 }\nfunc pair(): (func(i32): i32, func(i32): i32) { (first, second) }\nfunc other(): (func(i32): i32, func(i32): i32) { pair() }\nfunc run(): void { let values = pair()\ndrop values }",
    );
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let frame = program.bind_code(code("pair").into(), []).unwrap();
    let mut work = WorkBudget::new(std::num::NonZeroU64::new(100_000).unwrap());
    let original = unit.instantiate_result_contract(&frame, &mut work).unwrap();
    let declared = program.codes().get(code("pair")).unwrap().result();
    assert!(original.reproject(program, declared).is_err());
    let qualified = original
        .reproject_result(program, code("pair"), &ValuePath::default(), declared)
        .unwrap();
    assert!(qualified.has_same_representation(program, &original));
    assert!(
        original
            .reproject_result(program, code("other"), &ValuePath::default(), declared)
            .is_err()
    );
    let call = named(&unit, "run").typed().body().expressions().iter().find_map(|(id, expression)| matches!(expression.kind, crate::value_access::ExpressionKind::Call { target: ValueFunctionOrigin::Declaration(target), .. } if target == code("pair")).then_some(id)).unwrap();
    let target = |ordinal| {
        program
            .function_data(FunctionWitness {
                origin: ValueFunctionOrigin::Application(ValueAppliedResult {
                    application: ValueApplication {
                        owner: code("run").into(),
                        call,
                    },
                    ordinal,
                }),
                invocation: ValueInvocation::Read,
            })
            .unwrap()
    };
    let first = original.component(program, ValueField::Tuple(0)).unwrap();
    let path = ValuePath::new([ValueProjection::Tuple(0)]);
    let selected = first
        .reproject_result(program, code("pair"), &path, target(0))
        .unwrap();
    assert!(selected.has_same_representation(program, &first));
    assert!(first.reproject(program, target(0)).is_err());
    assert!(
        first
            .reproject_result(program, code("pair"), &path, target(1))
            .is_err()
    );
    assert!(
        selected
            .reproject_result(
                program,
                code("pair"),
                &ValuePath::new([ValueProjection::Tuple(1)]),
                target(0)
            )
            .is_err()
    );
    let wrong = program.concrete_witness(code("wrong").into()).unwrap();
    let wrong = program
        .instantiate_function(program.function_data(wrong.witness()).unwrap(), &wrong)
        .unwrap();
    assert!(
        wrong
            .reproject_result(program, code("pair"), &path, target(0))
            .is_err()
    );
    let taken = program
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Application(ValueAppliedResult {
                application: ValueApplication {
                    owner: code("run").into(),
                    call,
                },
                ordinal: 0,
            }),
            invocation: ValueInvocation::Take,
        })
        .unwrap();
    assert!(
        first
            .reproject_result(program, code("pair"), &path, taken)
            .is_err(),
        "metadata projection cannot adapt an invocation view"
    );
}

#[test]
fn constructed_absence_retains_positive_inactive_environment_metadata() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/constructed-outcome-environments.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn constructed_absence_keeps_each_original_positive_environment_instance() {
    use nocter_model::ValueField;
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/constructed-outcome-environments.nct"
    ));
    let unit = accepted(
        &source
            .replace("const ANSWER: i32 = calculate()", "")
            .replace("const func", "func"),
    );
    let variants: [(&str, &[ValueField]); 6] = [
        ("forward", &[]),
        ("forward_pair", &[ValueField::Tuple(0)]),
        (
            "forward_array",
            &[ValueField::Array(0), ValueField::Tuple(0)],
        ),
        ("forward_outcome", &[ValueField::Variant(true)]),
        ("forward_mixed", &[ValueField::Tuple(0)]),
        (
            "forward_mixed_outcome",
            &[
                ValueField::Variant(true),
                ValueField::Array(0),
                ValueField::Tuple(0),
            ],
        ),
    ];
    for (owner, fields) in variants {
        let contexts = inactive_contexts(&unit, owner, fields);
        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].data(), contexts[1].data());
        assert_ne!(contexts[0], contexts[1]);
        assert!(!contexts[0].has_same_representation(unit.program(), &contexts[1]));
    }
}

fn inactive_contexts(
    unit: &Unit,
    owner: &str,
    fields: &[nocter_model::ValueField],
) -> Vec<nocter_declarations::value_access::DataInstanceKey> {
    use crate::value_access::{ExpressionKind, ValueSource};
    use nocter_declarations::value_access::CodeInstanceKey;
    use nocter_model::{ValueApplication, ValueCode, ValueField, ValueFunctionOrigin as Origin};
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let budget = || {
        nocter_constant_evaluation::state::WorkBudget::new(
            std::num::NonZeroU64::new(100_000).unwrap(),
        )
    };
    let invoke = |caller: &CodeInstanceKey, call| {
        unit.instantiate_invocation(
            caller,
            ValueApplication {
                owner: caller.code(),
                call,
            },
            &mut budget(),
        )
        .unwrap()
    };
    let root = program.bind_code(code("calculate").into(), []).unwrap();
    let mut contexts = Vec::new();
    for (call, expression) in named(unit, "calculate").typed().body().expressions().iter() {
        if !matches!(expression.kind, ExpressionKind::Call { target: Origin::Declaration(target), .. } if target == code(owner))
        {
            continue;
        }
        let outer = invoke(&root, call);
        let task = named(unit, owner)
            .typed()
            .body()
            .expressions()
            .iter()
            .find_map(|(call, expression)| {
                matches!(
                    expression.kind,
                    ExpressionKind::Call {
                        target: Origin::Closure(_),
                        ..
                    }
                )
                .then_some(call)
            })
            .unwrap();
        let environment = invoke(&outer, task);
        let ValueCode::Closure(closure) = environment.code() else {
            panic!("expected captured code")
        };
        let body = &unit.closures()[&closure];
        let call = body.typed().body().expressions().iter().find_map(|(call, expression)| matches!(expression.kind, ExpressionKind::Call { target: Origin::Declaration(target), .. } if target == code(owner)).then_some(call)).unwrap();
        let inner = invoke(&environment, call);
        assert_eq!(inner.input_data(0), outer.input_data(0));
        let input = inner.input_data(0).unwrap();
        assert!(
            !input
                .component(program, ValueField::Variant(true))
                .unwrap()
                .is_uninhabited()
        );
        let graph = unit.value_sources(environment.code()).unwrap();
        let (source, node) = graph.nodes().find(|(_, node)| matches!(node.value, ValueSource::CallResult(site) if site.call == call)).unwrap();
        let result = unit
            .instantiate_value_data(&environment, source, node.data.unwrap(), &mut budget())
            .unwrap();
        let selected = fields
            .iter()
            .try_fold(result.clone(), |carrier, field| {
                carrier.component(program, *field)
            })
            .unwrap();
        assert!(selected.has_same_representation(program, input));
        let site = ValueApplication {
            owner: environment.code(),
            call,
        };
        assert_input_recipe(unit, site, &outer, &inner, &result, fields);
        assert_resources(unit, &environment, body);
        contexts.push(result);
    }
    contexts
}

fn assert_input_recipe(
    unit: &Unit,
    site: nocter_model::ValueApplication,
    outer: &nocter_declarations::value_access::CodeInstanceKey,
    inner: &nocter_declarations::value_access::CodeInstanceKey,
    result: &nocter_declarations::value_access::DataInstanceKey,
    inactive_path: &[nocter_model::ValueField],
) {
    use crate::value_access::{ProducerRelation, ProducerTerm};
    use nocter_declarations::value_access::FunctionWitness;
    use nocter_model::{
        ValueAppliedResult, ValueCode, ValueField, ValueFunctionOrigin, ValueInvocation,
        ValueParameter,
    };
    let program = unit.program();
    let ValueCode::Declaration(owner) = inner.code() else {
        panic!("expected declared callee")
    };
    let parameters = program.codes().get(owner).unwrap().parameters();
    let component = |index| ValueParameter::Component {
        input: parameters[index],
        ordinal: 0,
    };
    let recipe = unit.invocation_recipe(site).unwrap();
    assert!(recipe.inputs[&component(0)].excludes_active_callable());
    assert_eq!(
        recipe
            .inputs
            .values()
            .filter(|input| input.excludes_active_callable())
            .count(),
        1
    );
    if let Some(second) = inner.input_data(1) {
        assert_eq!(outer.input_data(1), Some(second));
        let (inactive, parent) = inactive_path.split_last().unwrap();
        assert_eq!(*inactive, ValueField::Tuple(0));
        let parent = parent
            .iter()
            .try_fold(result.clone(), |data, field| {
                data.component(program, *field)
            })
            .unwrap();
        assert!(
            parent
                .component(program, ValueField::Tuple(1))
                .unwrap()
                .has_same_representation(program, second)
        );
        assert_eq!(
            recipe.inputs[&component(1)].producers(),
            &std::collections::BTreeSet::from([FunctionWitness {
                origin: ValueFunctionOrigin::Parameter(component(1)),
                invocation: ValueInvocation::Read
            }])
        );
        let [first, second] = [0, 1].map(|ordinal| {
            unit.producer_relation(ProducerTerm::Application(ValueAppliedResult {
                application: site,
                ordinal,
            }))
            .unwrap()
        });
        let (ProducerRelation::Apply(first), ProducerRelation::Apply(second)) = (first, second)
        else {
            panic!("expected original applied results")
        };
        assert!(std::sync::Arc::ptr_eq(first, second));
    }
}

#[test]
fn constructed_absence_survives_original_return_application_equations() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/constructed-outcome-results.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let invalid = runtime.replace(
        "discard(forward(none))",
        "let original = forward(none)\nlet moved = original^\ndrop original\ndiscard(moved^)",
    );
    let fixture = nocter_test_support::ValueSourceFixture::new(&invalid);
    assert!(matches!(
        SourceUnit::prepare(fixture.syntax()),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
    let unit = accepted(&runtime);
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let caller = program.bind_code(code("calculate").into(), []).unwrap();
    let body = named(&unit, "calculate");
    let budget = || {
        nocter_constant_evaluation::state::WorkBudget::new(
            std::num::NonZeroU64::new(100_000).unwrap(),
        )
    };
    let mut excluded = Vec::new();
    for (call, expression) in body.typed().body().expressions().iter() {
        let crate::value_access::ExpressionKind::Call {
            target: nocter_model::ValueFunctionOrigin::Declaration(target),
            ..
        } = expression.kind
        else {
            continue;
        };
        let frame = unit
            .instantiate_invocation(
                &caller,
                nocter_model::ValueApplication {
                    owner: caller.code(),
                    call,
                },
                &mut budget(),
            )
            .unwrap();
        assert_resources(
            &unit,
            &frame,
            named(&unit, &program.code_label(target).unwrap()),
        );
        if target == code("forward") || target == code("absent") {
            let result = unit
                .instantiate_result_contract(&frame, &mut budget())
                .unwrap();
            excluded.push(
                result
                    .component(program, nocter_model::ValueField::Variant(true))
                    .unwrap()
                    .is_uninhabited(),
            );
        }
    }
    assert_eq!(excluded, [true, true, false]);
    unit.instantiate_value_data_map(&caller, &mut budget())
        .unwrap();
    assert_resources(&unit, &caller, body);
}

#[test]
fn lifted_callable_inputs_preserve_actual_code_through_nested_outcomes_and_components() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/lifted-callable-outcome-inputs.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
    let mismatched = source.replace(
        "const func identity(value: i32): i32",
        "const func identity(value: u64): u64",
    );
    let fixture = nocter_test_support::ValueSourceFixture::new(&mismatched);
    assert!(SourceUnit::prepare(fixture.syntax()).is_err());
}

#[test]
fn directly_constructed_absence_closes_a_receiving_callable_constraint() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/constructed-outcome-inputs.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn constructed_absence_cannot_retype_a_frozen_source_contract() {
    use crate::value_access::{InstantiationFailure, ValueSource};
    use nocter_constant_evaluation::state::WorkBudget;
    use nocter_model::ValueField;
    use std::num::NonZeroU64;
    let unit = accepted(
        "func discard(take value: (func(i32): i32)?): i32 { drop value\n42 }\nfunc other(take value: (func(u64): u64)?): void { drop value }\nfunc absent_u64(): (func(u64): u64)? { none }\nfunc run(): i32 { discard(none) }",
    );
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let frame = program.bind_code(code("run").into(), []).unwrap();
    let target = program
        .function_contract(code("other").into())
        .unwrap()
        .inputs[0]
        .data;
    let mut count = 0;
    for (value, node) in unit.value_sources(frame.code()).unwrap().nodes() {
        if !matches!(&node.value, ValueSource::Aggregate(fields) if fields.is_empty()) {
            continue;
        }
        let actual = node.data.unwrap();
        let budget = || WorkBudget::new(NonZeroU64::new(100_000).unwrap());
        let key = unit
            .instantiate_value_data(&frame, value, actual, &mut budget())
            .unwrap();
        assert!(
            key.component(program, ValueField::Variant(true))
                .unwrap()
                .is_uninhabited()
        );
        for target in [target, named(&unit, "absent_u64").typed().result_data()] {
            assert_eq!(
                unit.instantiate_value_data(&frame, value, target, &mut budget()),
                Err(InstantiationFailure::Unresolved)
            );
        }
        count += 1;
    }
    assert!(count > 0);
}
