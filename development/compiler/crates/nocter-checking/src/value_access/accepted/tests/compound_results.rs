//! Declared compound callable results retain original return paths and occurrence contexts.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::{Value as Datum, WorkBudget};
use nocter_declarations::value_access::CodeInstanceKey;
use std::num::NonZeroU64;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/contextual-compound-results.nct"
));
const OUTCOMES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/contextual-outcome-results.nct"
));
const PRODUCERLESS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/producerless-outcome-results.nct"
));

#[test]
fn producerless_optional_results_preserve_the_completed_carrier() {
    let source = PRODUCERLESS;
    let unit = accepted(
        &source
            .replace("const ANSWER: i32 = calculate()", "")
            .replace("const func", "func"),
    );
    assert_outcome_contracts(&unit);
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}

#[test]
fn producerless_fallible_results_preserve_actual_error_carriers() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/producerless-outcome-results-errors.nct"
    ));
    let (sources, tree) = input(source);
    let unit = SourceUnit::prepare_with_primitive_authority(
        bound(&sources, &tree),
        &super::primitives::authority(),
    )
    .unwrap()
    .into_unit();
    assert_outcome_contracts(&unit);
}

#[test]
fn optional_callable_results_preserve_present_and_absent_returns() {
    let source = OUTCOMES;
    let unit = accepted(
        &source
            .replace("const ANSWER: i32 = calculate()", "")
            .replace("const func", "func"),
    );
    assert_outcome_contracts(&unit);
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}

#[test]
fn outcome_result_metadata_preserves_quotas_and_healthy_retry() {
    use crate::value_access::InstantiationFailure;
    for (source, name) in [(OUTCOMES, "choose_owned"), (PRODUCERLESS, "forward")] {
        let unit = accepted(
            &source
                .replace("const ANSWER: i32 = calculate()", "")
                .replace("const func", "func"),
        );
        let code = unit
            .program()
            .resolve(unit.program().symbols().get(name).unwrap())
            .unwrap();
        let frame = unit.program().bind_code(code.into(), []).unwrap();
        let budget = |units| WorkBudget::new(NonZeroU64::new(units).unwrap());
        let expected = unit
            .instantiate_result_contract(&frame, &mut budget(100_000))
            .unwrap();
        assert_eq!(
            unit.instantiate_result_contract(&frame, &mut budget(1)),
            Err(InstantiationFailure::WorkLimit)
        );
        assert_eq!(
            unit.instantiate_result_contract(&frame, &mut budget(100_000)),
            Ok(expected)
        );
    }
}

#[test]
fn outcome_result_representations_do_not_grant_transfers_code_identity_or_const_capability() {
    for (index, source) in [
        OUTCOMES.replace(
            "let retained = owned^ otherwise",
            "let duplicate = owned\nlet retained = owned^ otherwise",
        ),
        OUTCOMES.replace(
            "let retained = owned^ otherwise { return 0 }",
            "let retained = owned^ otherwise { return 0 }\nlet again = owned^",
        ),
        OUTCOMES.replace(
            "func forward(present: bool): (func(i32): i32)?",
            "func forward(present: bool): (func(bool): i32)?",
        ),
        OUTCOMES.replace("const func identity(", "func identity("),
        "func unresolved(): func(): i32 { unresolved() }".to_owned(),
    ]
    .into_iter()
    .enumerate()
    {
        assert_ne!(source, OUTCOMES);
        let (sources, tree) = input(&source);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(
                (index, &failure),
                (0 | 2 | 4, SourceFailure::Checking(Failure::Body { .. }))
                    | (1, SourceFailure::Checking(Failure::Flow { .. }))
                    | (3, SourceFailure::Checking(Failure::Const { .. }))
            ),
            "case {index}: {failure:?}"
        );
    }
}

#[test]
fn empty_compound_results_do_not_manufacture_element_producers_or_shift_ordinals() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/empty-compound-results.nct"
    ));
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let unit = accepted(&runtime);
    let program = unit.program();
    for (_, body) in unit.bodies() {
        if let Ok(frame) = program.bind_code(body.typed().code(), []) {
            assert_resources(&unit, &frame, body);
        }
    }
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}

#[test]
fn empty_result_descriptors_still_reject_incompatible_signatures_and_seed_access() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/empty-compound-results.nct"
    ));
    for changed in [
        source.replace(
            "func forward(): ([func(i32): i32; 0]",
            "func forward(): ([func(bool): i32; 0]",
        ),
        source.replace("[action^; 0]", "[action; 0]"),
    ] {
        assert_ne!(changed, source);
        let (sources, tree) = input(&changed);
        assert!(matches!(
            SourceUnit::prepare(bound(&sources, &tree)),
            Err(SourceFailure::Checking(Failure::Body { .. }))
        ));
    }
}

#[test]
fn declared_compound_results_preserve_independent_returned_callable_producers() {
    assert_eq!(answer(SOURCE, "ANSWER"), Datum::integer(42));
    let unit = accepted(SOURCE);
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    for (ordinal, name) in ["addition", "subtraction"].into_iter().enumerate() {
        assert_eq!(
            unit.result_witness(nocter_model::ValueCallableResult {
                owner: code("pair"),
                ordinal
            })
            .unwrap()
            .origin,
            nocter_model::ValueFunctionOrigin::Declaration(code(name))
        );
    }
    assert!(
        unit.result_witness(nocter_model::ValueCallableResult {
            owner: code("pair"),
            ordinal: 2
        })
        .is_none()
    );
}

#[test]
fn compound_result_channels_keep_the_original_received_contexts() {
    use crate::value_access::ExpressionKind;
    use nocter_model::{ValueApplication, ValueFunctionOrigin};
    let source = SOURCE
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let unit = accepted(&source);
    let program = unit.program();
    let calculate = program
        .resolve(program.symbols().get("calculate").unwrap())
        .unwrap();
    let caller = program.bind_code(calculate.into(), []).unwrap();
    assert_resource_contexts(&unit, &caller);
    let mut count = 0;
    for (call, expression) in named(&unit, "calculate")
        .typed()
        .body()
        .expressions()
        .iter()
    {
        let ExpressionKind::Call {
            target: ValueFunctionOrigin::Declaration(target),
            ..
        } = expression.kind
        else {
            continue;
        };
        let name = program.code_label(target).unwrap();
        if !["pack", "relay", "recursive"].contains(&name.as_str()) {
            continue;
        }
        let budget = || WorkBudget::new(NonZeroU64::new(100_000).unwrap());
        let frame = unit
            .instantiate_invocation(
                &caller,
                ValueApplication {
                    owner: caller.code(),
                    call,
                },
                &mut budget(),
            )
            .unwrap_or_else(|failure| panic!("{name} invocation: {failure:?}"));
        unit.instantiate_result_contract(&frame, &mut budget())
            .unwrap_or_else(|failure| panic!("{name} result: {failure:?}"));
        unit.instantiate_prepared_data(&frame, &mut budget())
            .unwrap_or_else(|failure| panic!("{name} preparations: {failure:?}"));
        unit.instantiate_value_data_map(&frame, &mut budget())
            .unwrap_or_else(|failure| panic!("{name} body values: {failure:?}"));
        if name == "relay" {
            assert_result_rejections(&unit, &frame);
        }
        count += 1;
    }
    assert_eq!(count, 5);
}

fn assert_result_rejections(unit: &Unit, frame: &CodeInstanceKey) {
    use crate::value_access::InstantiationFailure;
    let budget = |units| WorkBudget::new(NonZeroU64::new(units).unwrap());
    let expected = unit
        .instantiate_result_contract(frame, &mut budget(100_000))
        .unwrap();
    let incomplete = unit.program().bind_code(frame.code(), []).unwrap();
    assert_eq!(
        unit.instantiate_result_contract(&incomplete, &mut budget(100_000)),
        Err(InstantiationFailure::Unresolved)
    );
    let unrelated = named(unit, "pair").typed().result_data();
    assert_eq!(
        unit.instantiate_result_data(frame, unrelated, &mut budget(100_000)),
        Err(InstantiationFailure::Unresolved)
    );
    assert_eq!(
        unit.instantiate_result_contract(frame, &mut budget(1)),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        unit.instantiate_result_contract(frame, &mut budget(100_000)),
        Ok(expected)
    );
}

#[test]
fn compound_results_preserve_transfer_and_actual_const_requirements() {
    for (index, source) in [
        SOURCE.replace("(left^, right^)", "(left, right^)"),
        SOURCE.replace("func relay(take callbacks:", "func relay(callbacks:"),
        SOURCE.replace(
            "let saved = owning.0^",
            "let saved = owning.0^\nlet _ = owning.0(0)",
        ),
        SOURCE.replace("const func addition(", "func addition("),
    ]
    .into_iter()
    .enumerate()
    {
        assert_ne!(source, SOURCE);
        let (sources, tree) = input(&source);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(
                (index, &failure),
                (0 | 1, SourceFailure::Checking(Failure::Body { .. }))
                    | (2, SourceFailure::Checking(Failure::Flow { .. }))
                    | (3, SourceFailure::Checking(Failure::Const { .. }))
            ),
            "case {index}: {failure:?}"
        );
    }
}

fn assert_resource_contexts(unit: &Unit, caller: &CodeInstanceKey) {
    let body = named(unit, "calculate");
    assert_resources(unit, caller, body);
}

fn assert_resources(unit: &Unit, caller: &CodeInstanceKey, body: &flow::Query) {
    for (point, block) in body.graph().points().iter() {
        for (action, operation) in block.actions.iter().enumerate() {
            let at = crate::value_access::flow::Location { point, action };
            for transition in body.access().resources().at(at) {
                let mut work = WorkBudget::new(NonZeroU64::new(100_000).unwrap());
                unit.project_resource_data(caller, at, &mut work)
                    .and_then(|mut projection| {
                        projection.steps(transition.before_steps())?;
                        projection.after_steps(transition.after_steps())
                    })
                    .unwrap_or_else(|failure| {
                        panic!(
                            "{at:?} {:?} before {:?} after {:?}: {failure:?}",
                            operation.operation,
                            transition.before_steps(),
                            transition.after_steps()
                        )
                    });
            }
        }
    }
}

fn assert_outcome_contracts(unit: &Unit) {
    for (id, body) in unit.bodies() {
        if let Ok(frame) = unit.program().bind_code(body.typed().code(), []) {
            unit.instantiate_value_data_map(
                &frame,
                &mut WorkBudget::new(NonZeroU64::new(100_000).unwrap()),
            )
            .unwrap_or_else(|failure| {
                panic!(
                    "{} values: {failure:?}",
                    unit.program().code_label(id).unwrap()
                )
            });
            assert_resources(unit, &frame, body);
            unit.instantiate_result_contract(
                &frame,
                &mut WorkBudget::new(NonZeroU64::new(100_000).unwrap()),
            )
            .unwrap_or_else(|failure| {
                panic!(
                    "{} result: {failure:?}",
                    unit.program().code_label(id).unwrap()
                )
            });
        }
    }
}
