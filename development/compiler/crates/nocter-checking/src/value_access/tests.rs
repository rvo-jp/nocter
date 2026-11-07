use super::*;
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_json::Value;
use nocter_model::{BuiltinType, ValueAccessMode as Mode};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{BoundSyntax, ParseGoal, SyntaxTree, parse};
use std::{fs, path::PathBuf};

fn input(text: &str) -> (SourceMap, SyntaxTree) {
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("body.nct"), text.as_bytes())
        .unwrap();
    let tree = parse(sources.get(id).unwrap(), ParseGoal::V2CoreUnit);
    assert!(!tree.has_errors(), "{text}: {:?}", tree.diagnostics());
    (sources, tree)
}
fn bound<'a>(sources: &'a SourceMap, tree: &'a SyntaxTree) -> BoundSyntax<'a> {
    BoundSyntax::new(sources.get(tree.source()).unwrap(), tree).unwrap()
}
fn queries(text: &str) -> Result<Vec<Query>, Failure> {
    let (sources, tree) = input(text);
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = header.bind(bound(&sources, &tree)).unwrap();
    context
        .program()
        .callables()
        .iter()
        .map(|(id, _)| Query::prepare(&context, id))
        .collect()
}
fn named<'a>(queries: &'a [Query], name: &str) -> &'a Query {
    queries
        .iter()
        .find(|q| {
            q.program().callables().get(q.callable()).unwrap().name()
                == q.program().symbols().get(name).unwrap()
        })
        .unwrap()
}
fn rejection(text: &str, rule: Rule) {
    let Err(Failure::Rejected(diagnostic)) = queries(text) else {
        panic!("expected {rule:?}: {text}");
    };
    assert_eq!(diagnostic.rule(), rule, "{text}: {:?}", diagnostic.source());
    assert!(!diagnostic.source().primary().span().range().is_empty());
}
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("object");
    };
    &members
        .iter()
        .find(|member| member.name.as_ref() == name)
        .unwrap()
        .value
}
fn string(value: &Value) -> &str {
    let Value::String(text) = value else {
        panic!("string");
    };
    text
}

#[test]
fn corpus_types_thirty_bodies_but_keeps_eleven_flow_rejections_pending() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v2-access");
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("cases");
    };
    let mut counts = [0; 5];
    for case in cases {
        let expectation = string(field(case, "expectation"));
        if expectation == "syntax-reject" {
            counts[0] += 1;
            continue;
        }
        if expectation == "declaration-reject" {
            counts[1] += 1;
            continue;
        }
        let text = fs::read_to_string(root.join(string(field(case, "file")))).unwrap();
        let outcome = queries(&text);
        if string(field(case, "body_typing")) == "reject" {
            let Err(Failure::Rejected(diagnostic)) = outcome else {
                panic!("{}: {outcome:?}", string(field(case, "id")));
            };
            assert_eq!(
                diagnostic.source().code(),
                string(field(case, "typing_code")),
                "{}",
                string(field(case, "id"))
            );
            let required = string(field(case, "primary_text"));
            let start = text.find(required).unwrap();
            let actual = diagnostic.source().primary().span().range();
            assert!(
                usize::try_from(actual.start().get()).unwrap() >= start
                    && usize::try_from(actual.end().get()).unwrap() <= start + required.len(),
                "{}: {actual:?} outside {required}",
                string(field(case, "id"))
            );
            counts[2] += 1;
        } else {
            assert!(
                outcome.is_ok(),
                "{}: {outcome:?}",
                string(field(case, "id"))
            );
            counts[if expectation == "accept" { 3 } else { 4 }] += 1;
        }
    }
    assert_eq!(counts, [6, 6, 8, 19, 11]);
}

#[test]
fn call_preparation_records_selected_targets_and_place_reservations() {
    let all = queries("func read(value: i32): i32 { value }\nfunc consume(value: ^i32): void {}\nfunc update(value: &i32, other: i32): void {}\nfunc run(value: ^i32, input: i32): void {\n update(value, read(value))\n consume(value)\n consume(input)\n}\n").unwrap();
    let run = named(&all, "run");
    let calls = run
        .body()
        .expressions()
        .iter()
        .filter_map(|(_, expr)| {
            if let ExpressionKind::Call { target, arguments } = &expr.kind {
                Some((*target, arguments))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0].0, named(&all, "read").callable());
    assert!(matches!(
        calls[0].1[0].action,
        ArgumentAction::ReserveRead(_)
    ));
    assert_eq!(calls[1].0, named(&all, "update").callable());
    assert!(matches!(
        calls[1].1[0].action,
        ArgumentAction::ReserveUpdate(_)
    ));
    assert_eq!(calls[1].1[1].action, ArgumentAction::ReadValue);
    assert!(matches!(
        calls[2].1[0].action,
        ArgumentAction::ReserveTransfer(_)
    ));
    assert_eq!(calls[3].1[0].action, ArgumentAction::CopyPayload);
    // The selected call never rewrites its direct place into an early transfer/update expression.
    assert!(!run.body().expressions().iter().any(|(_, expr)| matches!(
        expr.kind,
        ExpressionKind::Transfer(_) | ExpressionKind::Update(_)
    )));
}

#[test]
fn inference_and_materialization_do_not_promote_a_read_alias() {
    let all = queries("func identity(value: &i32): &i32 { value }\nfunc run(value: ^i32): void {\n let inspect = value\n let edit = &value\n let moved = ^value\n let answer: ^i32 = inspect\n let supplied = identity(edit)\n}\n").unwrap();
    let run = named(&all, "run");
    let modes = run
        .body()
        .locals()
        .iter()
        .map(|(_, local)| local.ty.mode())
        .collect::<Vec<_>>();
    assert_eq!(
        modes,
        [
            Mode::Own,
            Mode::Read,
            Mode::Update,
            Mode::Own,
            Mode::Own,
            Mode::Update
        ]
    );
    assert!(run.body().expressions().iter().any(|(_, expr)| matches!(
        expr.kind,
        ExpressionKind::Boundary {
            action: BoundaryAction::CopyPayload,
            ..
        }
    )));
    rejection(
        "func run(value: &i32): void {\n let inspect = value\n inspect = 1\n}\n",
        Rule::NotWritable,
    );
}

#[test]
fn copy_read_result_is_independent_but_keeps_declared_mode() {
    let all = queries("func read(value: &i32): i32 { value }\n").unwrap();
    let body = all[0].body();
    let result = body.blocks().get(body.root()).unwrap().result.unwrap();
    assert_eq!(
        body.expressions().get(result).unwrap().ty,
        ValueType::Value(nocter_model::ValueAccessType::new(
            all[0].program().data_type(BuiltinType::I32).unwrap(),
            Mode::Read
        ))
    );
    assert!(matches!(
        body.expressions().get(result).unwrap().kind,
        ExpressionKind::Boundary {
            action: BoundaryAction::CopyPayload,
            ..
        }
    ));
}

#[test]
fn contextual_branches_copy_without_silently_moving_places() {
    queries("func run(value: ^i32, flag: bool): ^i32 {\n let result: ^i32 = if flag { value } else { 20 }\n return ^result\n}\n").unwrap();
    rejection(
        "func run(value: ^i32, flag: bool): void {\n let result = if flag { value } else { 20 }\n}\n",
        Rule::TypeMismatch,
    );
    rejection(
        "func take(value: ^error): void {}\nfunc run(value: ^error, flag: bool): void {\n take(if flag { value } else { value })\n}\n",
        Rule::MissingTransfer,
    );
}

#[test]
fn literal_range_checks_include_grouped_signed_minimum_and_radices() {
    let all = queries("func minimum(): ^i32 { -(2_147_483_648) }\nfunc maximum(): ^i32 { 0x7fff_ffff }\nfunc binary(): ^i32 { -0b1000_0000 }\n").unwrap();
    assert!(
        named(&all, "minimum")
            .body()
            .expressions()
            .iter()
            .any(|(_, expr)| expr.kind == ExpressionKind::Integer(i32::MIN))
    );
    for text in [
        "2147483648",
        "-2147483649",
        "18446744073709551616",
        "0xffff_ffff_ffff_ffff",
    ] {
        rejection(
            &format!("func run(): ^i32 {{ {text} }}\n"),
            Rule::IntegerRange,
        );
    }
    queries("func run(value: i32): ^i32 { -value }\n").unwrap();
}

#[test]
fn unreachable_source_still_obeys_names_types_and_structural_operations() {
    rejection(
        "func run(): void {\n return\n let value = missing\n}\n",
        Rule::UnknownName,
    );
    rejection(
        "func run(): void {\n return\n let value = true + 1\n}\n",
        Rule::TypeMismatch,
    );
    rejection(
        "func run(value: i32): void {\n return\n let moved = ^value\n}\n",
        Rule::InvalidTransfer,
    );
    rejection(
        "func run(): void {\n return\n break\n}\n",
        Rule::InvalidLoopControl,
    );
    let all = queries("func run(): ^i32 {\n loop {\n return 1\n break\n }\n}\n").unwrap();
    assert_eq!(
        all[0].body().blocks().get(all[0].body().root()).unwrap().ty,
        ValueType::Never
    );
}

#[test]
fn names_calls_and_discard_have_one_structural_rule() {
    for text in [
        "func run(value: i32): void { let value = 1 }\n",
        "func other(): void {}\nfunc run(): void { let other = 1 }\n",
        "func run(): void { let i32 = 1 }\n",
    ] {
        rejection(text, Rule::ShadowedName);
    }
    rejection(
        "func run(value: i32): void { value() }\n",
        Rule::InvalidCall,
    );
    rejection(
        "func other(value: i32): void {}\nfunc run(): void { other() }\n",
        Rule::InvalidCall,
    );
    rejection(
        "func run(): void { let _: i32 = 1 }\n",
        Rule::AnnotatedDiscard,
    );
    queries("func run(): void {\n let _ = 1\n let owned = 2\n drop owned\n}\n").unwrap();
    rejection(
        "func run(value: i32): void { drop value }\n",
        Rule::InvalidDrop,
    );
    rejection("func run(): void {\n 1\n return\n}\n", Rule::UnusedValue);
    rejection("func run(): ^i32 {}\n", Rule::MissingResult);
    rejection("func run(): void { if 1 {} }\n", Rule::TypeMismatch);
}

#[test]
fn source_rebinding_requires_same_authority_and_exact_own_body() {
    let (sources, tree) =
        input("func run(value: ^i32): ^i32 { ^value }\nfunc sibling(): void {}\n");
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let first = header.bind(bound(&sources, &tree)).unwrap();
    let id = first
        .program()
        .resolve(first.program().symbols().get("run").unwrap())
        .unwrap();
    let query = Query::prepare(&first, id).unwrap();
    let (later_sources, later_tree) =
        input("\n\nfunc run(value: ^i32): ^i32 { ^value }\nfunc sibling(): void { let _ = 1 }\n");
    let later = header.bind(bound(&later_sources, &later_tree)).unwrap();
    let projected = query.bind(&later).unwrap();
    for (expr, _) in query.body().expressions().iter() {
        let origin = projected.expression_origin(expr).unwrap();
        assert_eq!(
            source_origin(projected.syntax(), origin)
                .unwrap()
                .span()
                .source(),
            later_tree.source()
        );
    }
    for (local, _) in query.body().locals().iter() {
        assert!(projected.local_origin(local).is_some());
    }
    let (changed_sources, changed_tree) =
        input("func run(value: ^i32): ^i32 { value + 1 }\nfunc sibling(): void {}\n");
    let changed = header.bind(bound(&changed_sources, &changed_tree)).unwrap();
    assert!(matches!(query.bind(&changed), Err(Failure::ChangedBody)));
    let independent_header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let independent = independent_header.bind(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        query.bind(&independent),
        Err(Failure::ChangedAuthority)
    ));
}

#[test]
fn completion_and_nested_loop_targets_preserve_control_meaning() {
    queries("func log(): void {}\nfunc run(flag: bool): void {\n if flag { log() }\n}\n").unwrap();
    queries("func run(): ^i32 {\n return 1\n true\n}\n").unwrap();
    queries("func run(): ^error {\n while true {}\n}\n").unwrap();
    let all =
        queries("func run(): void {\n loop {\n while false { break }\n continue\n }\n}\n").unwrap();
    let body = all[0].body();
    let loop_ids = body
        .statements()
        .iter()
        .filter_map(|(_, stmt)| match stmt {
            Statement::Break(id) | Statement::Continue(id) => Some(*id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(loop_ids.len(), 2);
    assert_ne!(loop_ids[0], loop_ids[1]);
}

#[test]
fn structural_access_error_points_back_to_the_declared_place() {
    let text = "func run(value: i32): void { let edit = &value }\n";
    let (sources, tree) = input(text);
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = header.bind(bound(&sources, &tree)).unwrap();
    let id = context.program().callables().iter().next().unwrap().0;
    let Err(Failure::Rejected(diagnostic)) = Query::prepare(&context, id) else {
        panic!("update must fail");
    };
    assert_eq!(diagnostic.rule(), Rule::InvalidUpdate);
    assert_eq!(diagnostic.source().notes().len(), 1);
    let note = &diagnostic.source().notes()[0];
    assert_eq!(
        context.syntax().text_at(note.origin().span().range()),
        Some("value")
    );
    assert!(note.message().contains("i32"));
}

#[test]
fn a_terminating_condition_does_not_require_results_from_unexecuted_branches() {
    queries("func run(flag: bool): ^i32 {\n if (if flag { return 1 } else { return 2 }) {} else {}\n}\n").unwrap();
    queries("func run(flag: bool): ^i32 {\n let unreachable: ^error = if (if flag { return 1 } else { return 2 }) {} else {}\n}\n").unwrap();
    rejection(
        "func run(flag: bool): ^i32 {\n if (if flag { return 1 } else { return 2 }) { let invalid = true + 1 } else {}\n}\n",
        Rule::TypeMismatch,
    );
}

#[test]
fn resolved_function_names_are_not_reported_as_unknown_places() {
    for (operation, rule) in [
        ("let value = target", Rule::TypeMismatch),
        ("let value = ^target", Rule::InvalidTransfer),
        ("let value = &target", Rule::InvalidUpdate),
        ("target = 1", Rule::NotWritable),
        ("drop target", Rule::InvalidDrop),
    ] {
        rejection(
            &format!("func target(): void {{}}\nfunc run(): void {{ {operation} }}\n"),
            rule,
        );
    }
    rejection(
        "func run(): void { let value = ^missing }\n",
        Rule::UnknownName,
    );
}

#[test]
fn primitive_equality_keeps_both_operands_and_rejects_unrelated_data() {
    let all = queries("func units(): ^bool { () == () }\nfunc booleans(): ^bool { true != false }\nfunc numbers(): ^bool { 1 == 2 }\n").unwrap();
    for query in &all {
        let body = query.body();
        let (_, equality) = body
            .expressions()
            .iter()
            .find(|(_, expr)| {
                matches!(
                    expr.kind,
                    ExpressionKind::Binary {
                        operation: Binary::Equal | Binary::NotEqual,
                        ..
                    }
                )
            })
            .unwrap();
        let ExpressionKind::Binary { left, right, .. } = equality.kind else {
            unreachable!();
        };
        assert_ne!(left, right);
        assert_eq!(
            equality.ty,
            ValueType::Value(nocter_model::ValueAccessType::new(
                query.program().data_type(BuiltinType::Bool).unwrap(),
                Mode::Own
            ))
        );
    }
    rejection("func run(): ^bool { true == 1 }\n", Rule::TypeMismatch);
    rejection(
        "func run(left: error, right: error): ^bool { left == right }\n",
        Rule::TypeMismatch,
    );
}
