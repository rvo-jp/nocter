//! Definition-owned const products, symbolic call edges and concrete closure boundaries.
use super::*;
use crate::value_access::constant::Rule;
use nocter_model::CompileTimeGuarantee;

const FORWARDING: &str = "const func identity(value: i32): i32 { value }\nconst func apply(action: func(i32): i32, value: i32): i32 { action(value) }\nconst func forward(action: func(i32): i32, value: i32): i32 { apply(action, value) }\nconst func anonymous(): i32 { let pass = (action: func(i32): i32, value: i32): i32 { forward(action, value) }\npass(identity, 42) }\nconst func interrupted(): i32 { forward(identity, if true { return 42 } else { return 0 }) }\nconst ANSWER: i32 = if anonymous() == 42 && interrupted() == 42 { 42 } else { 0 }";

#[test]
fn optional_runtime_witnesses_keep_their_unqualified_const_obligation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/value-guarded-callable-outcomes.nct"
    ));
    let source = format!(
        "{source}\nconst func forward(take value: (func(i32): i32)?): i32 {{ inspect(value^) }}"
    );
    accepted(&source);
    for target in ["inspect", "forward"] {
        for present in [false, true] {
            let required =
                format!("{source}\nconst ANSWER: i32 = {target}(produce({present})) + 31");
            let fixture = nocter_test_support::ValueSourceFixture::new(&required);
            let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
                SourceUnit::prepare(fixture.syntax())
            else {
                panic!("tag state alone is not a closed const-guard proof: {target}")
            };
            assert!(matches!(diagnostic.rule(), Rule::RuntimeCall(_)));
            let permitted = required.replace("func runtime(", "const func runtime(");
            assert_eq!(
                super::initializers::answer(&permitted, "ANSWER"),
                nocter_constant_evaluation::state::Value::integer(if present { 73 } else { 42 })
            );
        }
    }
}

#[test]
fn invocation_resource_attribution_matches_original_nullable_input_witnesses() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/value-guarded-callable-outcomes.nct"
    ));
    let source = format!(
        "{source}\nconst func plain(action: func(i32): i32, flag: bool): i32 {{ if flag {{ action(35) }} else {{ 11 }} }}\nfunc fixed(take value: (func(i32): i32)?): i32 {{ runtime(35) }}\nconst func second(take other: (func(i32): i32)?, take value: (func(i32): i32)?): i32 {{ let action = value^ otherwise {{ return 11 }}\naction(35) }}"
    );
    let unit = accepted(&source);
    for (name, input) in [
        ("inspect", Some(0)),
        ("second", Some(1)),
        ("plain", None),
        ("fixed", None),
    ] {
        let body = named(&unit, name);
        let requirements = unit.const_requirements(body.typed().code()).unwrap();
        let attributed: Vec<_> = requirements.invocation_input_resources().collect();
        if let Some(input) = input {
            let [(call, local)] = attributed.as_slice() else {
                panic!("one exact nullable input: {name}")
            };
            assert_eq!(*local, body.typed().body().inputs().explicit()[input].local);
            assert!(matches!(
                body.typed().body().expressions().get(*call).unwrap().kind,
                crate::value_access::ExpressionKind::Call {
                    target: nocter_model::ValueFunctionOrigin::Parameter(_),
                    ..
                }
            ));
        } else {
            assert!(attributed.is_empty(), "{name}");
        }
    }
}

#[test]
fn nullable_input_stability_uses_original_alias_and_call_write_correspondence() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/value-guarded-callable-outcomes.nct"
    ));
    let source = format!(
        "{source}\nconst func no_change(edit value: (func(i32): i32)?): void {{}}\nconst func increment(edit number: i32): void {{ number += 1 }}\nconst func changed(edit value: (func(i32): i32)?): i32 {{ no_change(value)\nread action = value otherwise {{ return 11 }}\naction(35) }}\nconst func aliased(edit value: (func(i32): i32)?): i32 {{ edit alias = value\nno_change(alias)\nread action = value otherwise {{ return 11 }}\naction(35) }}\nconst func unrelated(value: (func(i32): i32)?): i32 {{ var number = 0\nincrement(number)\nread action = value otherwise {{ return 11 }}\naction(35) }}"
    );
    let unit = accepted(&source);
    for (name, stable) in [
        ("inspect", true),
        ("changed", false),
        ("aliased", false),
        ("unrelated", true),
    ] {
        let body = named(&unit, name);
        let requirements = unit.const_requirements(body.typed().code()).unwrap();
        assert_eq!(
            requirements.invocation_input_resources().count(),
            1,
            "{name}"
        );
        assert_eq!(
            requirements.stable_invocation_inputs().count(),
            usize::from(stable),
            "{name}"
        );
    }
    for (parameter, extraction) in [
        ("value", "read action = value"),
        ("edit value", "edit action = value"),
    ] {
        let source = source
            .replace("inspect(take value", &format!("inspect({parameter}"))
            .replace("let action = value^", extraction);
        assert!(source.contains(&format!("func inspect({parameter}:")));
        let unit = accepted(&source);
        let body = named(&unit, "inspect");
        assert_eq!(
            unit.const_requirements(body.typed().code())
                .unwrap()
                .stable_invocation_inputs()
                .count(),
            1
        );
    }
}

#[test]
fn const_requirements_share_the_exact_body_and_keep_capability_conditional() {
    use nocter_declarations::value_access::FunctionWitness;
    let source = "func runtime(value: i32): i32 { value }\nconst func apply(action: func(i32): i32, value: i32): i32 { action(value) }\nconst func ignore(action: func(i32): i32): i32 { 42 }\nfunc run(): i32 { let local = (): i32 { 42 }\ndrop local\napply(runtime, 41) }";
    let unit = accepted(source);
    let clone = unit.clone();
    for (id, body) in unit.bodies() {
        let requirements = unit.const_requirements(id.into()).unwrap();
        assert_eq!(requirements.code(), id.into());
        assert!(std::ptr::eq(requirements.body(), body));
        assert!(std::ptr::eq(
            requirements,
            clone.const_requirements(id.into()).unwrap()
        ));
    }
    for (&id, body) in unit.closures() {
        let requirements = unit
            .const_requirements(nocter_model::ValueCode::Closure(id))
            .unwrap();
        assert!(std::ptr::eq(requirements.body(), body.as_ref()));
    }
    let program = unit.program();
    let runtime = program
        .resolve_code(program.symbols().get("runtime").unwrap())
        .unwrap();
    let apply = named(&unit, "apply").typed().code();
    let ignore = named(&unit, "ignore").typed().code();
    let bind = |code: nocter_model::ValueCode| {
        let nocter_model::ValueCode::Declaration(id) = code else {
            panic!("named code");
        };
        let formal = program.codes().get(id).unwrap().parameters()[0];
        program
            .bind_code(code, [(formal.into(), FunctionWitness::from(runtime))])
            .unwrap()
    };
    let apply_key = bind(apply);
    let ignore_key = bind(ignore);
    let requirements = unit.const_requirements(apply).unwrap();
    assert!(requirements.check(&|_| true).unwrap().is_none());
    assert_eq!(
        bound_check(requirements, &apply_key)
            .unwrap()
            .unwrap()
            .rule(),
        Rule::RuntimeCall(runtime)
    );
    assert!(bound_check(requirements, &ignore_key).is_err());
    assert!(
        bound_check(unit.const_requirements(ignore).unwrap(), &ignore_key)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        program.codes().get(runtime).unwrap().compile_time(),
        CompileTimeGuarantee::RuntimeOnly
    );

    let changed = source.replace("action(value)", "action(value + 1)");
    let next = accepted(&changed);
    let next_requirements = next.const_requirements(apply).unwrap();
    assert_eq!(requirements.code(), next_requirements.code());
    assert!(!std::ptr::eq(requirements.body(), next_requirements.body()));
    let (sources, tree) = input(&changed);
    let syntax = bound(&sources, &tree);
    let headers = declarations::Query::prepare(syntax).unwrap();
    let context = headers.bind(syntax).unwrap();
    assert!(requirements.body().bind(&context).is_err());
    assert!(unit.bind(&context).is_err());
}

#[test]
fn stored_callable_cleanup_conditions_survive_an_unused_consuming_input() {
    use crate::value_access::ExpressionKind;
    let unit = accepted(
        "const func discard(take action: once(i32): i32): void {}\nfunc run(take input: error): void { let action = (input^; value: i32): i32 { let moved = input^\nvalue }\ndiscard(action^) }",
    );
    let program = unit.program();
    let run = named(&unit, "run");
    let caller = program.bind_code(run.typed().code(), []).unwrap();
    let (call, _) = run
        .typed()
        .body()
        .expressions()
        .iter()
        .find(|(_, expression)| matches!(expression.kind, ExpressionKind::Call { .. }))
        .unwrap();
    let mut work = nocter_constant_evaluation::state::WorkBudget::new(
        nocter_constant_evaluation::state::Limits::default().steps(),
    );
    let key = unit
        .instantiate_invocation(
            &caller,
            nocter_model::ValueApplication {
                owner: caller.code(),
                call,
            },
            &mut work,
        )
        .unwrap();
    let requirements = unit.const_requirements(key.code()).unwrap();
    assert!(requirements.check(&|_| true).unwrap().is_none());
    let violation = bound_check(requirements, &key).unwrap().unwrap();
    assert!(matches!(violation.rule(), Rule::UnsupportedCleanup(_)));
    assert_eq!(
        violation.dependency(requirements.body(), &key).unwrap(),
        key.witnesses().keys().next().copied()
    );
    assert!(violation.dependency(run, &key).is_err());
}

#[test]
fn source_condition_edges_compose_named_formals_without_inventing_code() {
    use crate::value_access::CallTemplate;
    use nocter_declarations::value_access::FunctionWitness;
    use nocter_model::{ValueCode, ValueFunctionOrigin, ValueParameter};
    let unit = accepted(FORWARDING);
    let program = unit.program();
    let apply = named(&unit, "apply").typed().code();
    let forward = named(&unit, "forward").typed().code();
    let identity = named(&unit, "identity").typed().code();
    let formal = |code| {
        let ValueCode::Declaration(code) = code else {
            panic!("named code");
        };
        ValueParameter::from(program.codes().get(code).unwrap().parameters()[0])
    };
    let forward_formal = formal(forward);
    let apply_formal = formal(apply);
    let template = program.code_template(forward).unwrap();
    let edges = unit
        .const_requirements(forward)
        .unwrap()
        .template_calls(&template)
        .unwrap();
    let [(_, CallTemplate::Code(inner))] = edges.as_slice() else {
        panic!("selected apply frame");
    };
    assert_eq!(inner.code(), apply);
    assert_eq!(
        inner.witnesses()[&apply_formal].origin,
        ValueFunctionOrigin::Parameter(forward_formal)
    );
    let edges = unit
        .const_requirements(apply)
        .unwrap()
        .template_calls(inner)
        .unwrap();
    let [(_, CallTemplate::Parameter(selected))] = edges.as_slice() else {
        panic!("outer parameter condition");
    };
    assert_eq!(
        selected.origin,
        ValueFunctionOrigin::Parameter(forward_formal)
    );
    let actual_witness = FunctionWitness {
        origin: identity.origin().unwrap(),
        invocation: nocter_model::ValueInvocation::Read,
    };
    let actual = program
        .bind_code(forward, [(forward_formal, actual_witness)])
        .unwrap();
    assert_eq!(
        inner.close(program, &actual).unwrap(),
        program
            .bind_code(apply, [(apply_formal, actual_witness)])
            .unwrap()
    );
    assert!(
        unit.const_requirements(apply)
            .unwrap()
            .template_calls(&template)
            .is_err()
    );
}

#[test]
fn source_condition_edges_preserve_lexical_formals_and_interrupted_preparation() {
    use crate::value_access::CallTemplate;
    use nocter_model::{ValueCode, ValueFunctionOrigin, ValueParameter};
    let unit = accepted(FORWARDING);
    let program = unit.program();
    let (&closure, _) = unit.closures().iter().next().unwrap();
    let lexical = ValueParameter::Closure {
        owner: closure,
        position: 0,
    };
    let closure_code = ValueCode::Closure(closure);
    let lexical_template = program.code_template(closure_code).unwrap();
    let first = unit
        .const_requirements(closure_code)
        .unwrap()
        .template_calls(&lexical_template)
        .unwrap();
    let [(_, CallTemplate::Code(first))] = first.as_slice() else {
        panic!("lexical forward");
    };
    let second = unit
        .const_requirements(first.code())
        .unwrap()
        .template_calls(first)
        .unwrap();
    let [(_, CallTemplate::Code(second))] = second.as_slice() else {
        panic!("lexical apply");
    };
    let final_edge = unit
        .const_requirements(second.code())
        .unwrap()
        .template_calls(second)
        .unwrap();
    let [(_, CallTemplate::Parameter(selected))] = final_edge.as_slice() else {
        panic!("lexical condition");
    };
    assert_eq!(selected.origin, ValueFunctionOrigin::Parameter(lexical));
    let interrupted = named(&unit, "interrupted").typed().code();
    assert!(
        unit.const_requirements(interrupted)
            .unwrap()
            .template_calls(&program.code_template(interrupted).unwrap())
            .unwrap()
            .is_empty()
    );
    let answer = program
        .resolve_constant(program.symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer),
        Some(&nocter_constant_evaluation::state::Value::integer(42).freeze())
    );
}

#[test]
fn recursive_condition_edges_form_a_finite_permutation_without_chasing_root_variables() {
    use crate::value_access::CallTemplate;
    use nocter_declarations::value_access::FunctionWitness;
    use nocter_model::{ValueCode, ValueFunctionOrigin, ValueParameter};
    let unit = accepted(
        "const func identity(value: i32): i32 { value }\nconst func increment(value: i32): i32 { value + 1 }\nconst func rotate(action: func(i32): i32, other: func(i32): i32, count: i32, value: i32): i32 { if count == 0 { action(value) } else { rotate(other, action, count - 1, value) } }\nconst ANSWER: i32 = rotate(identity, increment, 1, 41)",
    );
    let program = unit.program();
    let code = named(&unit, "rotate").typed().code();
    let ValueCode::Declaration(id) = code else {
        panic!("named code");
    };
    let parameters = program.codes().get(id).unwrap().parameters();
    let first = ValueParameter::from(parameters[0]);
    let second = ValueParameter::from(parameters[1]);
    let root = program.code_template(code).unwrap();
    let requirements = unit.const_requirements(code).unwrap();
    let edges = requirements.template_calls(&root).unwrap();
    let rotated = edges
        .iter()
        .find_map(|(_, edge)| match edge {
            CallTemplate::Code(key) => Some(key),
            CallTemplate::Parameter(_) => None,
        })
        .unwrap();
    assert_ne!(rotated, &root);
    assert_eq!(rotated.code(), root.code());
    assert_eq!(
        rotated.witnesses()[&first].origin,
        ValueFunctionOrigin::Parameter(second)
    );
    let next = requirements.template_calls(rotated).unwrap();
    assert!(
        next.iter()
            .any(|(_, edge)| matches!(edge, CallTemplate::Code(key) if key == &root))
    );
    assert!(
        next.iter()
            .any(|(_, edge)| matches!(edge, CallTemplate::Parameter(witness)
        if witness.origin == ValueFunctionOrigin::Parameter(second)))
    );
    let identity = named(&unit, "identity").typed().code();
    let increment = named(&unit, "increment").typed().code();
    let witness = |code: ValueCode| FunctionWitness {
        origin: code.origin().unwrap(),
        invocation: nocter_model::ValueInvocation::Read,
    };
    let actual = program
        .bind_code(
            code,
            [(first, witness(identity)), (second, witness(increment))],
        )
        .unwrap();
    let closed = rotated.close(program, &actual).unwrap();
    assert_eq!(closed.witnesses()[&first].code(), increment);
    assert_eq!(closed.witnesses()[&second].code(), identity);
    let answer = program
        .resolve_constant(program.symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer),
        Some(&nocter_constant_evaluation::state::Value::integer(42).freeze())
    );
}

#[test]
fn bound_condition_calls_reuse_original_frames_across_clauses() {
    use crate::value_access::constant::BoundCheck;
    use nocter_constant_evaluation::state::WorkBudget;
    use nocter_declarations::value_access::FunctionWitness;
    use std::num::NonZeroU64;
    let unit = accepted(FORWARDING);
    let program = unit.program();
    let identity = program
        .resolve_code(program.symbols().get("identity").unwrap())
        .unwrap();
    for name in ["apply", "forward"] {
        let body = named(&unit, name);
        let code = body.typed().code();
        let nocter_model::ValueCode::Declaration(id) = code else {
            unreachable!();
        };
        let formal = program.codes().get(id).unwrap().parameters()[0];
        let caller = program
            .bind_code(code, [(formal.into(), FunctionWitness::from(identity))])
            .unwrap();
        let requirements = unit.const_requirements(code).unwrap();
        let call = requirements.completed_calls().next().unwrap();
        assert_eq!(requirements.completed_calls().count(), 1);
        let id = call.expression();
        let needed = (1..=10_000)
            .find(|&units| {
                let mut work = WorkBudget::new(NonZeroU64::new(units).unwrap());
                crate::value_access::instances::condition_call(
                    body.typed(),
                    &caller,
                    id,
                    None,
                    &mut work,
                )
                .is_ok()
            })
            .unwrap();
        let mut limited = WorkBudget::new(NonZeroU64::new(needed).unwrap());
        limited.step().unwrap();
        assert!(
            matches!(requirements.check_bound(&|_| true, &caller, &mut limited),
            Err(crate::value_access::constant::BoundCheckFailure::Invocation { expression, failure: crate::value_access::InstantiationFailure::WorkLimit }) if expression == id)
        );
        // Exact expression-data selection has its own metadata work, separate from discovery
        // of the callee. The request-local call cache has an independent exhausted-budget test.
        let mut work =
            WorkBudget::new(nocter_constant_evaluation::state::Limits::default().steps());
        let BoundCheck::Calls { calls, .. } = requirements
            .check_bound(&|_| true, &caller, &mut work)
            .unwrap()
        else {
            panic!("the bound clauses are eligible");
        };
        let expected = unit
            .instantiate_invocation(
                &caller,
                nocter_model::ValueApplication {
                    owner: code,
                    call: id,
                },
                &mut WorkBudget::new(NonZeroU64::new(10_000).unwrap()),
            )
            .unwrap();
        assert_eq!(calls[&id], expected);
        assert_eq!(
            calls.len(),
            1,
            "invocation and receiving checks share the same resolved call"
        );
        let wrong = program.bind_code(identity.into(), []).unwrap();
        assert!(
            requirements
                .check_bound(&|_| true, &wrong, &mut work)
                .is_err()
        );
    }
}

#[test]
fn uninvoked_const_definitions_reject_a_statically_false_forwarded_invocation_condition() {
    let source = "func runtime(value: i32): i32 { value }\nconst func apply(action: func(i32): i32): i32 { action(42) }\nconst func impossible(): i32 { apply(runtime) }\nconst ANSWER: i32 = 42";
    let (sources, tree) = input(source);
    let syntax = bound(&sources, &tree);
    let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
        SourceUnit::prepare(syntax)
    else {
        panic!(
            "an authored const definition cannot hide a fixed runtime binding behind a const helper"
        );
    };
    assert!(matches!(diagnostic.rule(), Rule::RuntimeCall(_)));
}

#[test]
fn closed_invocation_obligations_cross_forwarding_and_recursion_without_closing_outer_inputs() {
    let prefix = "func runtime(value: i32): i32 { value }\nconst func valid(value: i32): i32 { value }\nconst func apply(action: func(i32): i32): i32 { action(42) }\nconst func forward(action: func(i32): i32): i32 { apply(action) }\nconst func rotate(action: func(i32): i32, other: func(i32): i32, count: i32): i32 { if count == 0 { action(42) } else { rotate(other, action, count - 1) } }\n";
    accepted(&format!(
        "{prefix}func ordinary(): i32 {{ forward(runtime) }}\nconst ANSWER: i32 = forward(valid)"
    ));
    for call in ["forward(runtime)", "rotate(valid, runtime, 1)"] {
        let source =
            format!("{prefix}const func impossible(): i32 {{ {call} }}\nconst ANSWER: i32 = 42");
        let (sources, tree) = input(&source);
        let syntax = bound(&sources, &tree);
        let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
            SourceUnit::prepare(syntax)
        else {
            panic!("closed invocation condition: {call}");
        };
        assert!(matches!(diagnostic.rule(), Rule::RuntimeCall(_)));
        assert!(
            diagnostic
                .source()
                .notes()
                .iter()
                .any(|note| syntax.text_at(note.origin().span().range()) == Some("runtime"))
        );
    }
}

#[test]
fn an_indirectly_unavailable_closure_may_be_formed_and_dropped_but_not_invoked() {
    let prefix = "func runtime(value: i32): i32 { value }\nconst func apply(action: func(i32): i32): i32 { action(42) }\n";
    let unit = accepted(&format!(
        "{prefix}const func calculate(): i32 {{ let action = (): i32 {{ apply(runtime) }}\ndrop action\n42 }}\nconst ANSWER: i32 = calculate()"
    ));
    let (&code, _) = unit.closures().iter().next().unwrap();
    assert!(!unit.const_templates.permitted_template(code));
    let source = format!(
        "{prefix}const func calculate(): i32 {{ let action = (): i32 {{ apply(runtime) }}\naction() }}\nconst ANSWER: i32 = calculate()"
    );
    let (sources, tree) = input(&source);
    let syntax = bound(&sources, &tree);
    let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
        SourceUnit::prepare(syntax)
    else {
        panic!("inferred invocation is unavailable");
    };
    assert!(matches!(diagnostic.rule(), Rule::CapturedCall(_)));
    for cause in ["runtime", "apply(runtime)", "action(42)"] {
        assert!(
            diagnostic
                .source()
                .notes()
                .iter()
                .any(|note| syntax.text_at(note.origin().span().range()) == Some(cause)),
            "{cause}"
        );
    }
}

#[test]
fn mutual_conditional_recursion_keeps_outer_inputs_symbolic_and_exposes_fixed_false_bindings() {
    let prefix = "func runtime(value: i32): i32 { value }\nconst func first(action: func(i32): i32): i32 { second(action) }\nconst func second(action: func(i32): i32): i32 { if false { first(action) } else { action(42) } }\n";
    accepted(&format!("{prefix}const ANSWER: i32 = 42"));
    let source = format!(
        "{prefix}const func impossible(): i32 {{ first(runtime) }}\nconst ANSWER: i32 = 42"
    );
    let (sources, tree) = input(&source);
    let syntax = bound(&sources, &tree);
    let Err(SourceFailure::Checking(Failure::Const { diagnostic, .. })) =
        SourceUnit::prepare(syntax)
    else {
        panic!("fixed false binding in a recursive conditional component");
    };
    assert!(matches!(diagnostic.rule(), Rule::RuntimeCall(_)));
    for cause in ["runtime", "first(runtime)", "second(action)"] {
        assert!(
            diagnostic
                .source()
                .notes()
                .iter()
                .any(|note| syntax.text_at(note.origin().span().range()) == Some(cause)),
            "{cause}"
        );
    }
}

#[test]
fn covered_conditions_share_closed_calls_and_preserve_unresolved_environment_fallbacks() {
    use nocter_constant_evaluation::state::WorkBudget;
    for (source, root, expected) in [
        (
            "func runtime(value: i32): i32 { value }\nconst func ignore(action: func(i32): i32): i32 { 42 }\nconst func root(): i32 { ignore(runtime) }\nconst ANSWER: i32 = root()",
            "root",
            true,
        ),
        (
            "const func ignore(action: func(i32): i32): i32 { 42 }\nconst func root(): i32 { let captured = 40\nlet action = (captured; value: i32): i32 { value + captured }\nignore(action) }\nconst ANSWER: i32 = root()",
            "root",
            true,
        ),
        (
            "const func identity(value: i32): i32 { value }\nconst func apply(action: func(i32): i32, value: i32): i32 { action(value) }\nconst func root(): i32 { apply(identity, 42) }\nconst ANSWER: i32 = root()",
            "root",
            true,
        ),
        (
            "const func apply(action: func(i32): i32): i32 { action(2) }\nconst func root(): i32 { let captured = 40\nlet action = (captured; value: i32): i32 { value + captured }\napply(action) }\nconst ANSWER: i32 = root()",
            "root",
            true,
        ),
        (
            "const func apply(edit action: mut(i32): i32): i32 { action(2) }\nconst func root(): i32 { let captured = 40\nvar action = (captured; value: i32): i32 { captured += value\ncaptured }\napply(action) }\nconst ANSWER: i32 = root()",
            "root",
            true,
        ),
        (
            "const func apply(take action: once(i32): i32): i32 { action^(2) }\nconst func root(): i32 { let captured = 40\nlet action = (captured; value: i32): i32 { value + captured }\napply(action^) }\nconst ANSWER: i32 = root()",
            "root",
            true,
        ),
        (
            "const func ignore(action: func(i32): i32): i32 { 42 }\nfunc root(take input: error): i32 { let callback = (input^; value: i32): i32 { value }\nignore(callback) }",
            "root",
            false,
        ),
        (
            "func runtime(value: i32): i32 { value }\nconst func ignore(action: func(i32): i32): i32 { 42 }\nconst func root(): i32 { let captured = 40\nlet action = (captured; value: i32): i32 { runtime(value) + captured }\nignore(action) }\nconst ANSWER: i32 = root()",
            "root",
            true,
        ),
        (FORWARDING, "forward", false),
    ] {
        let (sources, tree) = input(source);
        let syntax = bound(&sources, &tree);
        let prepared = SourceUnit::prepare(syntax).unwrap();
        let context = prepared.declarations().bind(syntax).unwrap();
        let unit = prepared.unit();
        let root = named(unit, root).typed().code();
        let verified = unit.result_evidence.verify(&context).unwrap();
        let mut work = WorkBudget::new(std::num::NonZeroU64::new(4).unwrap());
        assert_eq!(
            unit.const_templates
                .prove_closed_root(root, &verified, &mut work)
                .unwrap(),
            expected,
            "{source}"
        );
        if expected {
            let raw = declarations::Query::prepare(syntax).unwrap();
            let raw_context = raw.bind(syntax).unwrap();
            let unmatched = unit.result_evidence.verify(&raw_context).unwrap();
            work.restart();
            assert!(
                unit.const_templates
                    .prove_closed_root(root, &unmatched, &mut work)
                    .is_err()
            );
        } else {
            work.charge(4).unwrap();
        }
    }
}

fn bound_check(
    requirements: &crate::value_access::constant::Requirements,
    key: &nocter_declarations::value_access::CodeInstanceKey,
) -> Result<
    Option<crate::value_access::constant::Violation>,
    crate::value_access::constant::BoundCheckFailure,
> {
    let mut work = nocter_constant_evaluation::state::WorkBudget::new(
        nocter_constant_evaluation::state::Limits::default().steps(),
    );
    requirements
        .check_bound(&|_| true, key, &mut work)
        .map(|check| match check {
            crate::value_access::constant::BoundCheck::Violation(violation) => Some(violation),
            crate::value_access::constant::BoundCheck::Calls { .. } => None,
        })
}
