use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{BuiltinType, CallableCapability, TypeKind};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CallTarget, CheckedOperation, TypeSubstitution, is_concrete_type, prepare_program_checking,
};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn annotated_closure_has_concrete_identity_and_direct_static_dispatch() {
    let output = check(
        "func main(): i32 {\n    let double = (value: i32): i32 { value * 2 }\n    double(3)\n}\n",
    )
    .unwrap();
    let program = output.program();
    assert_eq!(program.closures().definitions().len(), 1);
    let (closure, definition) = program.closures().definitions().iter().next().unwrap();
    assert_eq!(
        definition.signature().capability(),
        CallableCapability::Readonly
    );
    assert!(matches!(
        program.types().get(definition.ty()),
        Some(TypeKind::Closure { definition: actual, .. }) if *actual == closure
    ));
    assert!(program.bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Call(call)
                    if matches!(call.target(), CallTarget::ClosureValue { closure: actual, .. } if *actual == closure)
            )
        })
    }));
}

#[test]
fn generic_closure_identity_specializes_its_enclosing_generic_domain() {
    let output = check(
        "func generic<T>(value: &T): void {\n\
             let inspect = (&value;): void { return }\n\
             inspect()\n\
             return\n\
         }\n",
    )
    .unwrap();
    let program = output.program();
    let (_, definition) = program.closures().definitions().iter().next().unwrap();
    let TypeKind::Closure {
        definition: closure,
        arguments,
    } = program.types().get(definition.ty()).unwrap()
    else {
        panic!("checked closure must have a structural closure type")
    };
    assert_eq!(arguments.len(), 1);
    assert!(matches!(
        program.types().get(arguments[0]),
        Some(TypeKind::GenericParameter(_))
    ));
    assert!(!is_concrete_type(program.types(), definition.ty()).unwrap());

    let parameter = match program.types().get(arguments[0]).unwrap() {
        TypeKind::GenericParameter(parameter) => *parameter,
        _ => unreachable!(),
    };
    let mut types = program.types().clone();
    let concrete = types.builtin(BuiltinType::I32);
    let mut substitution = TypeSubstitution::default();
    substitution.bind_generic(parameter, concrete);
    let specialized = substitution
        .apply_type(&mut types, definition.ty())
        .unwrap();

    assert!(is_concrete_type(&types, specialized).unwrap());
    assert!(matches!(
        types.get(specialized),
        Some(TypeKind::Closure {
            definition: actual,
            arguments,
        }) if actual == closure && arguments.as_ref() == [concrete]
    ));
}

#[test]
fn capture_access_derives_invocation_capability_independently_of_copyability() {
    let output = check(
        "func main(): i32 {\n    var total = 0\n    var add = (&+total; value: i32): i32 {\n        total += value\n        total\n    }\n    add(2)\n}\n",
    )
    .unwrap();
    let definition = output
        .program()
        .closures()
        .definitions()
        .iter()
        .next()
        .unwrap()
        .1;
    assert_eq!(
        definition.signature().capability(),
        CallableCapability::ReadWrite
    );
    assert_eq!(
        output.program().copyabilities().get(definition.ty()),
        Some(crate::Copyability::MoveOnly)
    );
}

#[test]
fn readonly_capture_loan_lives_through_the_last_closure_use() {
    let error = check(
        "func invalid(): i32 {\n    var value = 1\n    let read = (&value;): i32 { value }\n    value = 2\n    read()\n}\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0397");

    check(
        "func valid(): i32 {\n    var value = 1\n    let read = (&value;): i32 { value }\n    let result = read()\n    value = 2\n    result\n}\n",
    )
    .unwrap();
}

#[test]
fn consuming_a_moved_capture_makes_the_closure_one_shot() {
    let error = check(
        "struct Box { value: i32 }\nfunc consume(value: Box): i32 { value.value }\nfunc invalid(value: Box): i32 {\n    let take = (move value;): i32 { consume(move value) }\n    let first = take()\n    take()\n}\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn closure_result_provenance_maps_capture_and_parameter_origins() {
    let output = check(
        "func main(text: &str): &str from text {\n    let choose = (&text; value: &str): &str {\n        if true { text } else { value }\n    }\n    choose(text)\n}\n",
    )
    .unwrap();
    let (closure, definition) = output
        .program()
        .closures()
        .definitions()
        .iter()
        .next()
        .unwrap();
    let provenance = output
        .program()
        .provenance()
        .closures()
        .get(closure)
        .unwrap();
    assert_eq!(provenance.parameters().origins().len(), 1);
    assert_eq!(provenance.captures(), definition.captures());
}

#[test]
fn unannotated_tail_result_is_inferred_without_crossing_callable_control() {
    let output =
        check("func main(): i32 {\n    let inferred = () { 42 }\n    inferred()\n}\n").unwrap();
    assert_eq!(output.program().closures().definitions().len(), 1);
}

#[test]
fn generic_callable_contract_infers_closure_parameters_and_result() {
    let output = check(
        "func accept<T, U, F>(callback: F, value: T): i32 where F: &func(value: T): U {\n    1\n}\nfunc main(): i32 {\n    accept((value) { value * 2 }, 4)\n}\n",
    )
    .unwrap();

    let definition = output
        .program()
        .closures()
        .definitions()
        .iter()
        .next()
        .unwrap()
        .1;
    let i32_type = output
        .program()
        .types()
        .builtin(nocter_model::BuiltinType::I32);
    assert_eq!(definition.signature().parameters(), &[i32_type]);
    assert_eq!(definition.signature().result(), i32_type);
    assert_eq!(definition.callable_requirements().len(), 1);
}

#[test]
fn closure_dependencies_are_inferred_independently_of_argument_order() {
    check(
        "func accept_chain<T, U, V, F, G>(second: G, first: F, value: T): i32 where F: &func(value: T): U, G: &func(value: U): V {\n    1\n}\nfunc main(): i32 {\n    accept_chain((value) { value + 1 }, (value) { value * 2 }, 4)\n}\n",
    )
    .unwrap();
}

#[test]
fn unannotated_result_joins_early_returns_and_tail_outcomes() {
    let output = check(
        "func main(flag: bool): i32? {\n    let choose = (value: bool) {\n        if value {\n            return none\n        }\n        42\n    }\n    choose(flag)\n}\n",
    )
    .unwrap();
    let definition = output
        .program()
        .closures()
        .definitions()
        .iter()
        .next()
        .unwrap()
        .1;
    assert!(matches!(
        output.program().types().get(definition.signature().result()),
        Some(TypeKind::Optional(payload))
            if *payload == output.program().types().builtin(nocter_model::BuiltinType::I32)
    ));
}

#[test]
fn unannotated_result_is_preserved_when_every_path_returns() {
    let output = check(
        "func main(flag: bool): i32 {\n    let choose = (value: bool) {\n        if value {\n            return 1\n        }\n        return 2\n    }\n    choose(flag)\n}\n",
    )
    .unwrap();
    let result = output
        .program()
        .closures()
        .definitions()
        .iter()
        .next()
        .unwrap()
        .1
        .signature()
        .result();
    assert_eq!(
        result,
        output
            .program()
            .types()
            .builtin(nocter_model::BuiltinType::I32)
    );
}

#[test]
fn propagation_uses_the_inferred_closure_result_boundary() {
    check(
        "func main(value: i32?): i32? {\n    let double = (input: i32?) {\n        let present = input?\n        present * 2\n    }\n    double(value)\n}\n",
    )
    .unwrap();
}

#[test]
fn structural_callable_provenance_bounds_closure_parameter_results() {
    let error = check(
        "func accept<F>(callback: F): i32 where F: &func(left: &str, right: &str): &str from left {\n    1\n}\nfunc main(left: &str, right: &str): i32 {\n    accept((first, second) { second })\n}\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
}

#[test]
fn closure_identity_follows_lexical_source_order_even_when_bodies_are_nested() {
    let output = check(
        "func main(): i32 {\n    let outer = (flag: bool): i32 {\n        let inner = (value: i32): i32 { value * 2 }\n        if flag { inner(1) } else { 0 }\n    }\n    outer(true)\n}\n",
    )
    .unwrap();
    let mut definitions = output.program().closures().definitions().iter();
    let (_, outer) = definitions.next().unwrap();
    let (_, inner) = definitions.next().unwrap();
    assert_eq!(
        outer.signature().parameters(),
        &[output
            .program()
            .types()
            .builtin(nocter_model::BuiltinType::Bool)]
    );
    assert_eq!(
        inner.signature().parameters(),
        &[output
            .program()
            .types()
            .builtin(nocter_model::BuiltinType::I32)]
    );
}

#[test]
fn closure_copyability_depends_only_on_stored_capture_types() {
    check(
        "func main(): i32 {\n    let original = (value: i32): i32 { value * 2 }\n    let copied = original\n    original(2) + copied(3)\n}\n",
    )
    .unwrap();

    let error = check(
        "func main(): i32 {\n    var total = 0\n    let original = (&+total; value: i32): i32 {\n        total += value\n        total\n    }\n    let copied = original\n    0\n}\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
}

#[test]
fn callable_requirement_checks_invocation_capability_not_copyability() {
    let error = check(
        "func inspect<F>(callback: F): i32 where F: &func(value: i32): i32 {\n    1\n}\nfunc main(): i32 {\n    var total = 0\n    inspect((&+total; value) {\n        total += value\n        total\n    })\n}\n",
    )
    .unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0390");
}

#[test]
fn calling_a_captured_concrete_closure_contributes_its_capability() {
    let output = check(
        "func main(): i32 {\n    var total = 0\n    var inner = (&+total;): i32 {\n        total += 1\n        total\n    }\n    var outer = (&+inner;): i32 { inner() }\n    outer()\n}\n",
    )
    .unwrap();
    let definitions = output
        .program()
        .closures()
        .definitions()
        .iter()
        .map(|(_, definition)| definition)
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), 2);
    assert_eq!(
        definitions[1].signature().capability(),
        CallableCapability::ReadWrite
    );
}

#[test]
fn a_diverging_closure_infers_never_as_its_result() {
    let output =
        check("func main(): i32 {\n    let stop = () { loop {} }\n    stop()\n}\n").unwrap();
    let result = output
        .program()
        .closures()
        .definitions()
        .iter()
        .next()
        .unwrap()
        .1
        .signature()
        .result();
    assert_eq!(
        result,
        output
            .program()
            .types()
            .builtin(nocter_model::BuiltinType::Never)
    );
}
