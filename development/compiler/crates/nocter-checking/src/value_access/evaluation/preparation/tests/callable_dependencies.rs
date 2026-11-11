//! Required call closure follows resolved invocation, not dormant function-value formation.
use super::*;

#[test]
fn a_factory_only_result_call_loads_its_resolved_named_body() {
    assert_answer_42(
        "const func increment(value: i32): i32 { value + 1 }\nconst func factory(): func(i32): i32 { increment }\nconst ANSWER: i32 = factory()(41)",
    );
}
#[test]
fn a_dormant_const_value_does_not_create_an_initializer_body_cycle() {
    assert_answer_42(
        "const func deferred(): i32 { ANSWER }\nconst func calculate(): i32 { let unused = deferred\ndrop unused\n42 }\nconst ANSWER: i32 = calculate()",
    );
}

#[test]
fn resolved_named_targets_close_transitive_and_recursive_call_bodies() {
    for source in [
        "const func increment(value: i32): i32 { value + 1 }\nconst func twice(value: i32): i32 { increment(increment(value)) }\nconst func factory(): func(i32): i32 { twice }\nconst ANSWER: i32 = factory()(40)",
        "const func descend(value: i32): i32 { if value == 0 { 42 } else { descend(value - 1) } }\nconst func factory(): func(i32): i32 { descend }\nconst ANSWER: i32 = factory()(4)",
    ] {
        assert_answer_42(source);
    }
}
#[test]
fn a_factory_only_result_call_can_supply_an_independent_local_count() {
    assert_answer_42(
        "const func increment(value: usize): usize { value + 1 }\nconst func factory(): func(usize): usize { increment }\nconst ANSWER: i32 = if true { let values: [i32; factory()(41)] = [0; 42]\n42 } else { 0 }",
    );
}

#[test]
fn resolved_target_effects_close_before_numeric_prefix_admission() {
    assert_answer_42(
        "const func increment(value: i32): i32 { value + 1 }\nconst func factory(): func(i32): i32 { increment }\nconst func calculate(): i32 { let items = [1, 2]\nvar index: usize = 0\nvar total = 0\nwhile index < 2 { let selected = items[index]^\ntotal += factory()(selected)\nindex += 1 }\nif total == 5 { 42 } else { 0 } }\nconst ANSWER: i32 = calculate()",
    );
}

#[test]
fn a_late_body_failure_keeps_its_resolved_call_cause_and_unwinds_requests() {
    let source = "const NEED: i32 = 1 / 0\nconst func deferred(): i32 { NEED }\nconst func factory(): func(): i32 { deferred }\nconst ANSWER: i32 = factory()()";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let headers = declarations::Query::prepare(syntax).unwrap();
    let context = headers.bind(syntax).unwrap();
    let id = context
        .program()
        .resolve_constant(context.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    let mut preparation = Preparation::new(context, Headers::Closed, state::Limits::default());
    let mut requests = Dependencies::default();
    let Err(QueryError::Computation(SourceFailure::Checking(Failure::Evaluation {
        diagnostic,
        ..
    }))) = requests.resolve_steps(&mut preparation, Request::Value(id.into()))
    else {
        panic!("actual required arithmetic failure");
    };
    assert_eq!(
        diagnostic.rule(),
        Rule::Execution(state::Rule::ArithmeticFailure)
    );
    assert!(
        diagnostic
            .source()
            .notes()
            .iter()
            .any(|note| syntax.text_at(note.origin().span().range()) == Some("factory()()"))
    );
    assert_eq!(requests.active_depth(), 0);
    assert_eq!(requests.active_keys().len(), 0);
    assert!(dependencies::value(&requests, id).is_none());
}

#[test]
fn resolved_count_supplier_bodies_guard_header_reuse_without_unrelated_bodies() {
    let source = "const func increment(value: usize): usize { value + 1 }\nconst func factory(): func(usize): usize { increment }\nstruct Data { values: [i32; factory()(41)] }\nfunc unrelated(): i32 { 0 }";
    let (sources, tree) = input(source);
    let unit = accepted::SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    for (changed, reusable) in [
        (source.replace("value + 1", "value + 2"), false),
        (source.replace("{ 0 }", "{ 1 }"), true),
    ] {
        let (sources, tree) = input(&changed);
        assert_eq!(
            unit.declarations().bind(bound(&sources, &tree)).is_ok(),
            reusable,
            "{changed}"
        );
    }
}
