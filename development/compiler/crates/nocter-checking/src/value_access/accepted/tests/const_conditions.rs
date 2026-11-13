//! Definition-owned const products, symbolic call edges and concrete closure boundaries.
use super::*;
use crate::value_access::constant::Rule;
use nocter_model::CompileTimeGuarantee;

const FORWARDING: &str = "const func identity(value: i32): i32 { value }\nconst func apply(action: func(i32): i32, value: i32): i32 { action(value) }\nconst func forward(action: func(i32): i32, value: i32): i32 { apply(action, value) }\nconst func anonymous(): i32 { let pass = (action: func(i32): i32, value: i32): i32 { forward(action, value) }\npass(identity, 42) }\nconst func interrupted(): i32 { forward(identity, if true { return 42 } else { return 0 }) }\nconst ANSWER: i32 = if anonymous() == 42 && interrupted() == 42 { 42 } else { 0 }";

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
    assert!(requirements.check(&|_| true, None).unwrap().is_none());
    assert_eq!(
        requirements
            .check(&|_| true, Some(&apply_key))
            .unwrap()
            .unwrap()
            .rule(),
        Rule::RuntimeCall(runtime)
    );
    assert!(requirements.check(&|_| true, Some(&ignore_key)).is_err());
    assert!(
        unit.const_requirements(ignore)
            .unwrap()
            .check(&|_| true, Some(&ignore_key))
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
    use crate::value_access::{ExpressionKind, instantiate_call};
    let unit = accepted(
        "const func discard(take action: once(i32): i32): void {}\nfunc run(take input: error): void { let action = (input^; value: i32): i32 { let moved = input^\nvalue }\ndiscard(action^) }",
    );
    let program = unit.program();
    let run = named(&unit, "run");
    let caller = program.bind_code(run.typed().code(), []).unwrap();
    let (_, expression) = run
        .typed()
        .body()
        .expressions()
        .iter()
        .find(|(_, expression)| matches!(expression.kind, ExpressionKind::Call { .. }))
        .unwrap();
    let ExpressionKind::Call {
        target,
        kind,
        arguments,
    } = &expression.kind
    else {
        unreachable!();
    };
    let key = instantiate_call(program, &caller, *target, *kind, arguments).unwrap();
    let requirements = unit.const_requirements(key.code()).unwrap();
    assert!(requirements.check(&|_| true, None).unwrap().is_none());
    let violation = requirements.check(&|_| true, Some(&key)).unwrap().unwrap();
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
fn stored_condition_edges_close_to_the_same_frozen_instance_as_native_call_binding() {
    use crate::value_access::instantiate_call;
    use nocter_declarations::value_access::FunctionWitness;
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
        for call in unit.const_requirements(code).unwrap().completed_calls() {
            let crate::value_access::ExpressionKind::Call {
                target,
                kind,
                arguments,
            } = &body
                .typed()
                .body()
                .expressions()
                .get(call.expression())
                .unwrap()
                .kind
            else {
                unreachable!();
            };
            assert_eq!(
                call.close(&caller).unwrap(),
                instantiate_call(program, &caller, *target, *kind, arguments).unwrap()
            );
            let wrong = program.bind_code(identity.into(), []).unwrap();
            assert!(call.close(&wrong).is_err());
        }
    }
}
