//! Immutable global places use ordinary reads/aliases, with no hidden ownership or input channel.
use super::*;
use crate::value_access::{LocalOrigin, evaluation::Rule as EvaluationRule};
use nocter_constant_evaluation::state::Value as Data;

#[test]
fn static_values_are_evaluated_but_their_names_denote_readonly_places() {
    let unit = accepted(
        "struct Pair { left: i32\nright: i32 }\nstatic PAIR: Pair = Pair { left: 40, right: 2 }\nstatic BASE: i32 = 40\nconst func calculate(): i32 { read pair = PAIR\nread left = pair.left\nBASE + left + pair.right }\nconst ANSWER: i32 = calculate()\nfunc run(): i32 { calculate() }",
    );
    let symbols = unit.program().symbols();
    let source = unit
        .program()
        .resolve_static(symbols.get("PAIR").unwrap())
        .unwrap();
    assert!(matches!(
        Data::from_frozen(unit.static_value(source).unwrap())
            .unwrap()
            .data(),
        nocter_constant_evaluation::state::Data::Aggregate(_)
    ));
    assert!(unit.constant_value(source.initializer).is_none());
    let answer = unit
        .program()
        .resolve_constant(symbols.get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        Data::from_frozen(unit.constant_value(answer).unwrap()).unwrap(),
        Data::integer(82)
    );
    let body = named(&unit, "calculate").typed().body();
    assert_eq!(body.inputs().ordered().count(), 0);
    assert_eq!(
        body.locals()
            .iter()
            .filter(|(_, local)| matches!(local.origin, LocalOrigin::Static(_)))
            .count(),
        2
    );
}
#[test]
fn static_reads_need_copy_for_values_and_never_grant_update_move_or_drop() {
    for body in [
        "let value = PAIR",
        "let value = PAIR^",
        "edit value = PAIR",
        "PAIR.left = 0",
        "drop PAIR",
        "take_pair(PAIR^)",
        "update(PAIR.left)",
    ] {
        let source = format!(
            "struct Pair {{ left: i32 }}\nstatic PAIR: Pair = Pair {{ left: 42 }}\nfunc take_pair(take value: Pair): void {{}}\nfunc update(edit value: i32): void {{ value = 1 }}\nfunc run(): void {{ {body} }}"
        );
        let (sources, tree) = input(&source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(
            matches!(
                Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
                Err(Failure::Body { .. })
            ),
            "{source}"
        );
    }
    accepted(
        "copy struct Pair { left: i32 }\nstatic PAIR: Pair = Pair { left: 42 }\nfunc run(): i32 { var value = PAIR\nvalue.left = 1\nPAIR.left }",
    );
}
#[test]
fn mixed_initializer_cycles_keep_the_dependency_query_and_exact_source_cause() {
    for (source, cause) in [
        ("static A: i32 = A", "A"),
        ("static A: i32 = if false { A } else { 42 }", "A"),
        (
            "static A: i32 = helper()\nstatic B: i32 = helper()\nconst func helper(): i32 { B }",
            "B",
        ),
        ("static A: i32 = B\nconst B: i32 = A", "A"),
        ("const A: i32 = B\nstatic B: i32 = A", "A"),
    ] {
        let (sources, tree) = input(source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let result = Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap());
        let Err(Failure::Evaluation { diagnostic, .. }) = result else {
            panic!("{source}: {result:?}");
        };
        assert_eq!(diagnostic.rule(), EvaluationRule::DependencyCycle);
        assert_eq!(
            sources
                .get(tree.source())
                .unwrap()
                .text_at(diagnostic.source().primary().span().range()),
            Some(cause),
            "{source}"
        );
    }
}
#[test]
fn static_initializer_changes_invalidate_callers_and_views_in_capture_bodies() {
    let source = "static BASE: i32 = 40\nfunc run(): i32 { let value = 2\nlet action = (value;): i32 { BASE + value }\naction() }";
    let (sources, tree) = input(source);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()).unwrap();
    let (changed, parsed) = input(&source.replace("= 40", "= 41"));
    let context = headers.bind(bound(&changed, &parsed)).unwrap();
    assert!(unit.bind(&context).is_err());
    assert!(named(&unit, "run").bind(&context).is_err());
}

#[test]
fn typed_static_views_need_metadata_without_computing_the_initializer() {
    let (sources, tree) = input("static VALUE: i32 = 1 / 0\nfunc run(): i32 { VALUE }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let id = context
        .program()
        .resolve(context.program().symbols().get("run").unwrap())
        .unwrap();
    let typed = crate::value_access::Query::prepare(&context, id).unwrap();
    assert_eq!(typed.body().static_views().count(), 1);
    assert!(matches!(
        Unit::prepare(&context),
        Err(Failure::Evaluation { .. })
    ));
}
