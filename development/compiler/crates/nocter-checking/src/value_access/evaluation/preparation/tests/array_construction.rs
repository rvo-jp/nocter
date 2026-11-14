//! Ordinary construction/selection checking and local failure causes.
use super::*;
use crate::value_access::Rule as BodyRule;

fn rejected(parameters: &str, body: &str) -> BodyRule {
    let text = format!("func run({parameters}): void {{ {body} }}");
    rejected_source(&text)
}
fn rejected_source(text: &str) -> BodyRule {
    let (sources, tree) = input(text);
    let Err(failure) = accepted::SourceUnit::prepare(bound(&sources, &tree)) else {
        panic!("expected source rejection: {text}")
    };
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
fn array_context_and_exact_inference_reject_invalid_construction() {
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
        ("", "let values = [[1, 2], [3]]", BodyRule::TypeMismatch),
        (
            "callback: func(): void",
            "let values = [callback]",
            BodyRule::MissingTransfer,
        ),
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
            "let values = [value^; 2]",
            BodyRule::InvalidConstruction,
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

#[test]
fn primitive_index_operands_keep_ordinary_local_diagnostics() {
    for (parameters, body, rule) in [
        (
            "items: [i32; 2], index: i32",
            "let value = items[index]",
            BodyRule::TypeMismatch,
        ),
        (
            "items: [i32; 2]",
            "read value = items[missing]",
            BodyRule::UnknownName,
        ),
        (
            "items: [i32; 2]",
            "edit value = items[true]",
            BodyRule::TypeMismatch,
        ),
        (
            "items: [i32; 2]",
            "let value = items[-1]^",
            BodyRule::IntegerRange,
        ),
        ("items: [i32; 2]", "items[true] = 1", BodyRule::TypeMismatch),
        ("items: i32", "let value = items[0]", BodyRule::TypeMismatch),
        (
            "items: [i32; 2]",
            "let value = items[0].field",
            BodyRule::UnknownField,
        ),
        (
            "items: [i32; 2]",
            "let value = items[true].field",
            BodyRule::TypeMismatch,
        ),
        (
            "index: i32",
            "let value = [1, 2][index]",
            BodyRule::TypeMismatch,
        ),
        (
            "index: usize",
            "let value = [][index]",
            BodyRule::UnknownResultType,
        ),
    ] {
        assert_eq!(rejected(parameters, body), rule, "{parameters}: {body}");
    }
    assert_eq!(
        rejected_source(
            "struct Holder { items: [i32; 2] }\nfunc run(holder: Holder): void { let value = holder.items[true] }"
        ),
        BodyRule::TypeMismatch
    );
}

#[test]
fn long_postfix_index_chains_reach_a_local_gate_without_recursive_index_preparation() {
    for suffix in ["[0]", "[0]^", "[0]()"] {
        let text = format!("let value = items{}", suffix.repeat(1024));
        assert_eq!(
            rejected("take items: [i32; 2]", &text),
            if suffix == "[0]()" {
                BodyRule::InvalidCall
            } else {
                BodyRule::TypeMismatch
            }
        );
    }
}
