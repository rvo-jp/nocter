//! Actual source typing, with the combined array body/storage admission gate still closed.
use super::*;
use crate::value_access::Rule as BodyRule;

fn rejected(parameters: &str, body: &str) -> BodyRule {
    let text = format!("func run({parameters}): void {{ {body} }}");
    let (sources, tree) = input(&text);
    let failure = accepted::SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    let SourceFailure::Checking(Failure::Body {
        failure: BodyFailure::Rejected(error),
        ..
    }) = failure
    else {
        panic!("ordinary source rejection: {text}: {failure:?}");
    };
    error.rule()
}

#[test]
fn array_context_and_exact_inference_precede_the_separate_body_admission_gate() {
    for (parameters, body, rule) in [
        ("", "let values = []", BodyRule::UnknownResultType),
        ("", "let values = [true, 1]", BodyRule::TypeMismatch),
        ("", "let values: [i32; 1] = [1, 2]", BodyRule::TypeMismatch),
        ("", "let values: i32 = [1]", BodyRule::TypeMismatch),
        (
            "size: usize",
            "let values = [size, 2]",
            BodyRule::TypeMismatch,
        ),
        (
            "size: usize",
            "let values: [usize; 2] = [size, 2]",
            BodyRule::UnsupportedData,
        ),
        ("", "let values: [i32; 0] = []", BodyRule::UnsupportedData),
        (
            "",
            "let values = [[1, 2], [3, 4]]",
            BodyRule::UnsupportedData,
        ),
        ("", "let values = [[1, 2], [3]]", BodyRule::TypeMismatch),
        ("", "let value = ([1, 2], true)", BodyRule::UnsupportedData),
        ("", "let values = [run]", BodyRule::UnsupportedData),
    ] {
        assert_eq!(rejected(parameters, body), rule, "{body}");
    }
}

#[test]
fn repetition_prepares_a_seed_even_at_zero_and_requires_copy_only_above_one() {
    for (parameters, body, rule) in [
        ("", "let values = [missing; 0]", BodyRule::UnknownName),
        (
            "take value: error",
            "let values = [value; 0]",
            BodyRule::MissingTransfer,
        ),
        (
            "value: error",
            "let values = [value^; 0]",
            BodyRule::MissingTransfer,
        ),
        (
            "take value: error",
            "let values = [value^; 0]",
            BodyRule::UnsupportedData,
        ),
        (
            "take value: error",
            "let values = [value^; 1]",
            BodyRule::UnsupportedData,
        ),
        (
            "take value: error",
            "let values = [value^; 2]",
            BodyRule::InvalidConstruction,
        ),
        (
            "size: usize",
            "let values = [size; 18446744073709551615]",
            BodyRule::UnsupportedData,
        ),
        ("", "let values: [i32; 2] = [1; 1]", BodyRule::TypeMismatch),
    ] {
        assert_eq!(rejected(parameters, body), rule, "{body}");
    }
}

#[test]
fn a_zero_repetition_seed_keeps_its_required_constant_dependency() {
    let (sources, tree) =
        input("const BAD: i32 = 1 / 0\nfunc run(): void { let values = [BAD; 0] }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let run = context
        .program()
        .resolve_code(context.program().symbols().get("run").unwrap())
        .unwrap();
    let root =
        ValueCodeRoot::Expression(context.body_expressions(run).unwrap().ids().next().unwrap());
    let mut preparation = Preparation::new(context, Headers::Closed, state::Limits::default());
    let mut requests = Dependencies::default();
    let Err(QueryError::Computation(SourceFailure::Checking(Failure::Evaluation {
        diagnostic,
        ..
    }))) = preparation.body(&mut requests, run)
    else {
        panic!("seed initialization must not disappear at zero count");
    };
    assert_eq!(diagnostic.source().code(), "E0325");
    assert_eq!(
        dependencies::value(&requests, root).unwrap().frozen,
        state::Value::integer(0).freeze()
    );
    assert_eq!(requests.active_depth(), 0);
    assert!(preparation.causes.is_empty());
}
