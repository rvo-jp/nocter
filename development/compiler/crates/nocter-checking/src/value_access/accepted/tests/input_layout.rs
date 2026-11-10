//! Invalid input channels cannot cross typed/control admission, including equal-data slots.
use super::*;
use nocter_model::{ValueBindingKind, ValueCode, ValueInputOperation};

fn rejects(typed: crate::value_access::Query) {
    assert!(matches!(
        flow::Query::prepare(Arc::new(typed)),
        Err(flow::IntegrityError::InconsistentTypedBody)
    ));
}

#[test]
fn named_input_order_and_exact_slot_coverage_are_closed_before_flow() {
    let unit = accepted("func run(first: i32, second: i32): i32 { first + second }");
    let query = named(&unit, "run").typed();
    assert_eq!(query.body().inputs().explicit().len(), 2);
    let mut swapped = query.clone();
    swapped.body.inputs.explicit.swap(0, 1);
    rejects(swapped);
    let mut duplicate = query.clone();
    duplicate.body.inputs.explicit[1] = duplicate.body.inputs.explicit[0];
    rejects(duplicate);
    let mut missing = query.clone();
    missing.body.inputs.explicit = Box::new([]);
    rejects(missing);
}

#[test]
fn changing_a_local_kind_and_input_operation_cannot_change_a_named_contract() {
    let unit = accepted("func run(value: i32): i32 { value }");
    let mut typed = named(&unit, "run").typed().clone();
    let input = &mut typed.body.inputs.explicit[0];
    input.operation = ValueInputOperation::Edit;
    let selected = input.local;
    typed.body.locals = typed.body.locals.map(|id, local| {
        let mut local = local.clone();
        if id == selected {
            local.kind = ValueBindingKind::EditAlias;
        }
        local
    });
    rejects(typed);
}

#[test]
fn closure_environment_is_not_an_explicit_input_or_its_source_owners_body() {
    let unit = accepted(
        "func run(): i32 {\nlet value = 1\nlet action = (value; number: i32): i32 { value + number }\naction(2)\n}",
    );
    let (&code, body) = unit.closures().iter().next().unwrap();
    assert_eq!(body.typed().code(), ValueCode::Closure(code));
    assert_eq!(body.typed().source_owner(), code.owner.source_owner());
    let mut switched = body.typed().clone();
    switched.body.inputs.environment = Some(switched.body.inputs.explicit[0]);
    rejects(switched);
    let mut source_owner = body.typed().clone();
    source_owner.code = code.owner.code();
    rejects(source_owner);
}

#[test]
fn a_coherent_slot_edit_cannot_replace_the_inferred_environment_contract() {
    let unit = accepted(
        "func run(): i32 {\nlet value = 1\nvar action = (value;): i32 { value += 1\nvalue }\naction()\n}",
    );
    let mut typed = unit.closures().values().next().unwrap().typed().clone();
    let environment = typed.body.inputs.environment.as_mut().unwrap();
    environment.operation = ValueInputOperation::Read;
    let selected = environment.local;
    typed.body.locals = typed.body.locals.map(|id, local| {
        let mut local = local.clone();
        if id == selected {
            local.kind = ValueBindingKind::ReadAlias;
        }
        local
    });
    rejects(typed);
}

#[test]
fn equal_input_layout_cannot_relabel_a_body_as_another_code_owner() {
    let (sources, tree) = input("func first(): i32 { 1 }\nfunc second(): i32 { 2 }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let code = |name| {
        context
            .program()
            .resolve(context.program().symbols().get(name).unwrap())
            .unwrap()
    };
    let mut typed = crate::value_access::Query::prepare(&context, code("first")).unwrap();
    typed.code = ValueCode::Declaration(code("second"));
    assert!(matches!(
        typed.bind(&context),
        Err(crate::value_access::Failure::InconsistentSyntax)
    ));
    rejects(typed);
}

#[test]
fn a_required_expression_cannot_masquerade_as_its_declaring_body() {
    let (sources, tree) = input("func run(): i32 { 42 }");
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let code = context.program().codes().iter().next().unwrap().0;
    let mut typed = crate::value_access::Query::prepare(&context, code).unwrap();
    typed.code = ValueCode::Expression(nocter_model::ValueExpression {
        owner: code,
        slot: 0,
    });
    assert!(matches!(
        typed.bind(&context),
        Err(crate::value_access::Failure::InconsistentSyntax)
    ));
    rejects(typed);
}
