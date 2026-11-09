//! Source admission, not execution evidence: required initializers have no private evaluator yet.
use super::*;
use crate::value_access::constant::{Diagnostic, Rule};
use nocter_model::CompileTimeGuarantee;
use std::fmt::Write;

fn rejected(text: &str, check: impl FnOnce(&Diagnostic)) {
    let (sources, tree) = input(text);
    let query = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = query.bind(bound(&sources, &tree)).unwrap();
    let Err(Failure::Const {
        callable,
        diagnostic,
    }) = Unit::prepare(&context)
    else {
        panic!("expected const domain rejection: {text}");
    };
    assert_eq!(
        context
            .program()
            .callables()
            .get(callable)
            .unwrap()
            .compile_time(),
        CompileTimeGuarantee::Evaluatable
    );
    assert_eq!(diagnostic.source().code(), "E0421");
    assert!(!diagnostic.source().primary().span().range().is_empty());
    check(&diagnostic);
}

#[test]
fn ordinary_operations_and_state_updates_share_one_checked_runtime_body() {
    let unit = accepted(
        "struct Pair {\nleft: i32\nright: i32\n}\nconst func increment(edit value: i32): void { value += 1 }\nconst func run(): i32 {\nvar pair = Pair { left: 0, right: 2 }\nedit target = pair.left\nwhile target < 3 { increment(target) }\nread snapshot = pair.right\nlet answer = pair.left + snapshot\nreturn answer\n}\nfunc runtime(): i32 { run() }",
    );
    assert_eq!(unit.bodies().len(), 3);
    let run = named(&unit, "run");
    assert!(run.graph().points().iter().any(|(_, p)| {
        p.actions
            .iter()
            .any(|a| matches!(a.operation, Operation::CommitCall(_)))
    }));
    assert!(
        named(&unit, "increment")
            .graph()
            .points()
            .iter()
            .any(|(_, p)| p
                .actions
                .iter()
                .any(|a| matches!(a.operation, Operation::CompoundAssign { .. })))
    );
}

#[test]
fn recursive_const_headers_close_without_executing_or_proving_termination() {
    accepted(
        "const func alpha(value: i32): i32 { if value == 0 { 1 } else { beta(value - 1) } }\nconst func beta(value: i32): i32 { alpha(value) }\nconst func forever(): i32 { loop {} }",
    );
}

#[test]
fn arithmetic_traps_are_evaluator_failures_not_forbidden_operation_domains() {
    for expression in ["1 / 0", "2147483647 + 1", "(-2147483647 - 1) / -1"] {
        accepted(&format!("const func f(): i32 {{ {expression} }}"));
    }
}

#[test]
fn all_const_definitions_and_unevaluated_branches_check_selected_calls() {
    for text in [
        "func runtime(): i32 { 1 }\nconst func unused(): i32 { runtime() }",
        "func runtime(): i32 { 1 }\nconst func unused(): i32 { if false { runtime() } else { 0 } }",
        "func runtime(): bool { true }\nconst func unused(): bool { false && runtime() }",
        "func runtime(): i32 { 1 }\nconst func unused(): i32 { loop {}\nruntime() }",
        "func runtime(value: i32): i32 { value }\nconst func unused(): i32 { runtime(if true { return 0 } else { return 1 }) }",
    ] {
        rejected(text, |diagnostic| {
            assert!(matches!(diagnostic.rule(), Rule::RuntimeCall(_)));
            assert!(diagnostic.source().message().contains("`runtime`"));
        });
    }
    // A trivial implementation does not confer an unauthored const capability.
    accepted("func runtime(): i32 { 1 }\nfunc ordinary(): i32 { runtime() }");
}

#[test]
fn explicit_and_implicit_resource_cleanup_need_domain_evidence() {
    for text in [
        "const func f(take input: error): void {}",
        "struct Resource {\nvalue: error\n}\nconst func f(take input: Resource): void {}",
        "const func f(take input: error): void { if false { drop input }\nloop {} }",
    ] {
        rejected(text, |diagnostic| {
            assert!(matches!(diagnostic.rule(), Rule::UnsupportedCleanup(_)));
        });
    }
    // A read input is not an owned resource, and an interrupted result is not constructed.
    accepted(
        "const func inspect(input: error): i32 { 1 }\nconst func diverge(value: i32): error { loop {} }\nconst func call(): i32 { let _ = diverge(if true { return 1 } else { return 2 }) }",
    );
    rejected("const func f(take input: error): error { input^ }", |d| {
        assert!(matches!(d.rule(), Rule::UnsupportedData(_)));
    });
}

#[test]
fn function_value_formation_and_invocation_need_a_later_evaluator_domain() {
    rejected(
        "const func identity(value: i32): i32 { value }\nconst func f(): i32 { let action = identity\naction(1) }",
        |d| assert_eq!(d.rule(), Rule::FunctionValue),
    );
    rejected(
        "const func f(action: func(i32): i32): i32 { action(1) }",
        |d| assert_eq!(d.rule(), Rule::ValueInvocation),
    );
    rejected(
        "const func f(): i32 { let value = 1\nlet action = (value;): i32 { value }\naction() }",
        |d| assert_eq!(d.rule(), Rule::FunctionValue),
    );
}

#[test]
fn ordinary_typing_and_flow_are_required_before_const_publication() {
    for (text, flow) in [
        ("const func f(): i32 { true }", false),
        (
            "const func f(take input: error): void { drop input\ndrop input }",
            true,
        ),
    ] {
        let (sources, tree) = input(text);
        let query = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let result = Unit::prepare(&query.bind(bound(&sources, &tree)).unwrap());
        if flow {
            assert!(matches!(result, Err(Failure::Flow { .. })));
        } else {
            assert!(matches!(result, Err(Failure::Body { .. })));
        }
    }
}

#[test]
fn current_source_projection_and_authority_invalidation_remain_exact() {
    let original = "func runtime(): i32 { 1 }\nconst func f(): i32 { runtime() }";
    let (a, ta) = input(original);
    let query = declarations::Query::prepare(bound(&a, &ta)).unwrap();
    let changed = format!("// shifted source\n{}", original.replace("1 }", "7 }"));
    let (b, tb) = input(&changed);
    let context = query.bind(bound(&b, &tb)).unwrap();
    let Err(Failure::Const { diagnostic, .. }) = Unit::prepare(&context) else {
        panic!("rejection")
    };
    assert_eq!(sources_text(&b, &tb, &diagnostic), "runtime()");
    let (c, tc) = input(&original.replace("func runtime", "const func runtime"));
    assert!(matches!(
        query.bind(bound(&c, &tc)),
        Err(declarations::Failure::ChangedHeader)
    ));
    let new_query = declarations::Query::prepare(bound(&c, &tc)).unwrap();
    let accepted = Unit::prepare(&new_query.bind(bound(&c, &tc)).unwrap()).unwrap();
    assert_eq!(accepted.bodies().len(), 2);
    assert!(accepted.bind(&context).is_err());
}

fn sources_text<'a>(
    sources: &'a nocter_source::SourceMap,
    tree: &nocter_syntax::SyntaxTree,
    diagnostic: &Diagnostic,
) -> &'a str {
    sources
        .get(tree.source())
        .unwrap()
        .text_at(diagnostic.source().primary().span().range())
        .unwrap()
}

#[test]
fn finite_move_only_records_and_sparse_cleanup_share_the_ordinary_domain() {
    accepted(
        "struct Pair {\nleft: i32\nright: i32\n}\nconst func select(take pair: Pair): i32 { pair.left^ }\nconst func run(): i32 { select(Pair { left: 1, right: 2 }) }",
    );
    let mut text = String::from("struct Level0 {\nvalue: i32\n}\n");
    for level in 1..64 {
        writeln!(
            text,
            "struct Level{level} {{\nleft: Level{}\nright: Level{}\n}}",
            level - 1,
            level - 1
        )
        .unwrap();
    }
    text.push_str("const func dispose(take value: Level63): void {}");
    accepted(&text);
}
