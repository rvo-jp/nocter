//! Definition-time source admission; required execution is covered by the initializer cases.
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
            .codes()
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
    assert_eq!(unit.bodies().count(), 3);
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
fn formal_function_invocation_needs_a_closed_witness() {
    rejected(
        "const func f(action: func(i32): i32): i32 { action(1) }",
        |d| assert_eq!(d.rule(), Rule::ValueInvocation),
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
    assert_eq!(accepted.bodies().count(), 2);
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

#[test]
fn source_dependencies_follow_cached_transitive_values_without_unrelated_initializers() {
    let text = "const BASE: bool = true\nconst USED: bool = helper()\nconst OTHER: bool = other()\nconst func helper(): bool { BASE }\nconst func other(): bool { false }\nfunc run(): i32 { if USED { 42 } else { 0 } }\nfunc plain(): i32 { 7 }";
    let (sources, tree) = input(text);
    let source = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    for (before, after, run_valid) in [
        ("BASE: bool = true", "BASE: bool = false", false),
        ("helper(): bool { BASE }", "helper(): bool { false }", false),
        ("USED: bool = helper()", "USED: bool = false", false),
        ("other(): bool { false }", "other(): bool { true }", true),
        ("OTHER: bool = other()", "OTHER: bool = true", true),
    ] {
        let (next_sources, next_tree) = input(&text.replace(before, after));
        let context = source
            .declarations()
            .bind(bound(&next_sources, &next_tree))
            .unwrap();
        assert_eq!(
            named(source.unit(), "run").bind(&context).is_ok(),
            run_valid,
            "{before}"
        );
        assert!(
            named(source.unit(), "plain").bind(&context).is_ok(),
            "{before}"
        );
        assert!(
            source.unit().bind(&context).is_err(),
            "whole Unit still checks every body"
        );
    }
}

#[test]
fn source_dependencies_retain_const_callees_in_unexecuted_branches() {
    let text = "const USED: usize = if true { 42 } else { helper() }\nconst func helper(): usize { 1 }\nfunc run(): usize { USED }";
    let (sources, tree) = input(text);
    let source = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    let (next_sources, next_tree) = input(&text.replace("usize { 1 }", "usize { 2 }"));
    let context = source
        .declarations()
        .bind(bound(&next_sources, &next_tree))
        .unwrap();
    assert!(named(source.unit(), "run").bind(&context).is_err());
}

#[test]
fn known_named_values_execute_with_views_factories_edit_inputs_and_sparse_arrays() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/const-named-values.nct"
    ));
    let unit = accepted(source);
    let answer = unit
        .program()
        .resolve_code(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer),
        Some(&nocter_constant_evaluation::state::Value::integer(42).freeze())
    );
}

#[test]
fn named_value_calls_require_authored_const_capability_in_every_branch() {
    for body in [
        "let action = runtime\naction()",
        "var action: mut(): i32 = runtime\nif false { action() } else { 42 }",
        "let action: once(): i32 = runtime\nlet _ = false && action^() == 1\n42",
    ] {
        rejected(
            &format!("func runtime(): i32 {{ 1 }}\nconst func unused(): i32 {{ {body} }}"),
            |d| assert!(matches!(d.rule(), Rule::RuntimeCall(_))),
        );
    }
    rejected(
        "func runtime(): i32 { 1 }\nconst func factory(): func(): i32 { runtime }\nconst func unused(): i32 { let action = factory()\naction() }",
        |d| assert!(matches!(d.rule(), Rule::RuntimeCall(_))),
    );
}

#[test]
fn named_witnesses_and_their_containers_have_temporary_but_no_final_data_evidence() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/const-named-values.nct"
    ));
    let unit = accepted(source);
    let mut types = crate::value_access::evaluation::Types::default();
    let mut kinds = [false; 3];
    for (_, local) in named(&unit, "calculate").typed().body().locals().iter() {
        assert!(types.supports(unit.program(), local.data).unwrap());
        if !crate::value_access::constant::supports_final(unit.program(), local.data).unwrap() {
            let kind = match unit.program().data_schema(local.data).unwrap() {
                nocter_declarations::value_access::PayloadSchema::Function(_) => 0,
                nocter_declarations::value_access::PayloadSchema::Aggregate(_) => 1,
                nocter_declarations::value_access::PayloadSchema::Array(_) => 2,
                nocter_declarations::value_access::PayloadSchema::Builtin(_)
                | nocter_declarations::value_access::PayloadSchema::Optional(_)
                | nocter_declarations::value_access::PayloadSchema::Fallible(_) => {
                    panic!("ordinary scalar/optional final admission must remain available")
                }
            };
            kinds[kind] = true;
        }
    }
    assert_eq!(kinds, [true; 3]);
}

#[test]
fn passing_a_named_value_cannot_fabricate_an_evaluator_formal_callable_representation() {
    let source = "const func identity(value: i32): i32 { value }\nconst func ignore(callback: func(i32): i32): i32 { 42 }\nconst func calculate(): i32 { ignore(identity) }";
    for suffix in ["", "\nconst ANSWER: i32 = calculate()"] {
        rejected(&format!("{source}{suffix}"), |d| {
            assert!(matches!(d.rule(), Rule::UnsupportedInput(_)));
        });
    }
}

#[test]
fn interrupted_named_argument_preparation_does_not_invent_a_formal_call() {
    let source = "const func identity(value: i32): i32 { value }\nconst func ignore(callback: func(i32): i32, other: i32): i32 { 0 }\nconst func calculate(): i32 { ignore(identity, if true { return 42 } else { return 0 }) }\nconst ANSWER: i32 = calculate()";
    assert_eq!(
        super::initializers::answer(source, "ANSWER"),
        nocter_constant_evaluation::state::Value::integer(42)
    );
}

#[test]
fn captured_shape_evidence_is_independent_of_source_const_and_final_admission() {
    for (source, supported) in [
        (
            "func run(): i32 { let value = 42\nlet action = (value;): i32 { value }\naction() }",
            true,
        ),
        (
            "func run(): i32 { let value = 42\nlet inner = (value;): i32 { value }\nlet outer = (inner;): i32 { inner() }\nouter() }",
            true,
        ),
        (
            "func run(take failure: error): void { let action = (failure^;): void {}\ndrop action }",
            false,
        ),
    ] {
        let unit = accepted(source);
        let mut shapes = crate::value_access::evaluation::Types::default();
        assert!(!unit.closures().is_empty());
        for &code in unit.closures().keys() {
            let data = unit
                .program()
                .function_data(nocter_declarations::value_access::FunctionWitness {
                    origin: nocter_model::ValueFunctionOrigin::Closure(code),
                    invocation: unit.program().closure(code).unwrap().contract().invocation,
                })
                .unwrap();
            assert_eq!(shapes.supports(unit.program(), data).unwrap(), supported);
            assert!(!shapes.supports_final(unit.program(), data).unwrap());
        }
    }
}

#[test]
fn bound_callable_shapes_keep_the_template_unavailable_and_actual_environments_distinct() {
    use crate::value_access::{ExpressionKind, instantiate_call};
    use nocter_declarations::value_access::PayloadSchema;
    let unit = accepted(
        "func identity(value: i32): i32 { value }\nfunc apply(take callback: once(i32): i32): i32 { callback^(1) }\nfunc run(): i32 { let seed = 41\nlet left = (seed; value: i32): i32 { seed + value }\nlet flag = true\nlet right = (flag; value: i32): i32 { if flag { value } else { 0 } }\nlet _ = apply(left^)\nlet _ = apply(right^)\napply(identity) }",
    );
    let program = unit.program();
    let apply = named(&unit, "apply").typed().code();
    let nocter_model::ValueCode::Declaration(apply) = apply else {
        panic!("named code");
    };
    let formal = program.codes().get(apply).unwrap().parameters()[0];
    let data = program.parameters().get(formal).unwrap().data();
    let run = named(&unit, "run");
    let caller = program.bind_code(run.typed().code(), []).unwrap();
    let mut types = crate::value_access::evaluation::Types::default();
    assert!(!types.supports(program, data).unwrap());
    assert!(types.get_in(program, &caller, data).is_err());
    let mut projected = std::collections::BTreeSet::new();
    let mut closure_codes = std::collections::BTreeSet::new();
    for (_, expression) in run.typed().body().expressions().iter() {
        let ExpressionKind::Call {
            target,
            kind,
            arguments,
        } = &expression.kind
        else {
            continue;
        };
        let key = instantiate_call(program, &caller, *target, *kind, arguments).unwrap();
        let index = types.get_in(program, &key, data).unwrap();
        assert!(projected.insert(index));
        assert_eq!(types.get_in(program, &key, data).unwrap(), index);
        let actual = key.witnesses().get(&formal.into()).unwrap().witness();
        let actual_data = program.function_data(actual).unwrap();
        assert!(matches!(
            program.data_schema(actual_data),
            Some(PayloadSchema::Function(_))
        ));
        assert!(types.supports(program, actual_data).unwrap());
        assert!(!types.supports_final(program, actual_data).unwrap());
        assert_eq!(types.get_in(program, &key, actual_data).unwrap(), index);
        closure_codes.insert(actual.origin);
    }
    assert_eq!(projected.len(), 3);
    assert_eq!(closure_codes.len(), 3);
    assert!(!types.supports(program, data).unwrap());
}
