use super::*;

#[test]
fn grouped_flat_callable_annotations_share_ordinary_witness_checking() {
    let source = "func identity(value: i32): i32 { value }\nfunc apply(action: ((func(i32): i32)), value: i32): i32 { action(value) }\nfunc f(): i32 { let action: (func(i32): i32) = identity\napply(action, 42) }";
    assert!(queries(source).is_ok());
}

#[test]
fn nested_callable_data_reports_missing_type_evidence_at_the_current_header() {
    for source in [
        "func f(input: func(func(): i32): i32): void {}",
        "func f(): func(): func(): i32 { loop {} }",
    ] {
        let fixture = nocter_test_support::ValueSourceFixture::new(source);
        let Err(accepted::SourceFailure::Declarations(declarations::Failure::Headers(diagnostics))) =
            accepted::SourceUnit::prepare(fixture.syntax())
        else {
            panic!("nested callable evidence: {source}");
        };
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.source().code() == "E0291" && diagnostic.violation().rule == nocter_declarations::value_access::DeclarationRule::UnsupportedCallableData));
        for diagnostic in &diagnostics {
            assert_eq!(
                diagnostic.source().primary().source(),
                fixture.syntax().tree().source()
            );
            let range = diagnostic.source().primary().span().range();
            assert!(
                source[range.start().get() as usize..range.end().get() as usize].contains("func")
            );
        }
    }
    reject(
        "func f(): void { let action: (func(): i32)? = none }",
        Rule::UnsupportedData,
    );
}

#[test]
fn callable_record_field_headers_supply_contracts_without_concrete_payload_evidence() {
    let fixture =
        nocter_test_support::ValueSourceFixture::new("struct Data { action: func(): i32 }");
    let unit = accepted::SourceUnit::prepare(fixture.syntax())
        .unwrap()
        .into_unit();
    let program = unit.program();
    let (_, record) = program.records().iter().next().unwrap();
    let field = record.fields()[0];
    let data = program.fields().get(field).unwrap().data();
    let Some(nocter_declarations::value_access::PayloadSchema::Function(selected)) =
        program.data_schema(data)
    else {
        panic!("field signature")
    };
    assert!(
        matches!(selected.origin, nocter_model::ValueFunctionOrigin::Contract(contract) if contract.owner == nocter_model::ValueContractOwner::Field(field))
    );
    assert!(program.function_contract(selected).is_some());
    assert!(program.concrete_witness(selected).is_err());
    assert!(program.instantiate_closed_data(record.data()).is_err());
}

#[test]
fn compound_result_headers_do_not_establish_a_callable_payload_or_returned_value() {
    use crate::value_access::InstantiationFailure;
    use nocter_constant_evaluation::state::WorkBudget;
    use std::num::NonZeroU64;
    let fixture =
        nocter_test_support::ValueSourceFixture::new("func f(): (func(): i32)? { loop {} }");
    let unit = accepted::SourceUnit::prepare(fixture.syntax())
        .unwrap()
        .into_unit();
    let program = unit.program();
    let id = program
        .resolve(program.symbols().get("f").unwrap())
        .unwrap();
    let code = program.codes().get(id).unwrap();
    assert_eq!(code.result_contracts().len(), 1);
    assert!(!program.owned_payload_supported(code.result()));
    assert!(program.instantiate_closed_data(code.result()).is_err());
    let frame = program.bind_code(id.into(), []).unwrap();
    let mut work = WorkBudget::new(NonZeroU64::new(100_000).unwrap());
    assert_eq!(
        unit.instantiate_result_data(&frame, code.result(), &mut work),
        Err(InstantiationFailure::Unresolved)
    );
}

#[test]
fn compound_result_values_must_match_each_declared_callable_contract() {
    let fixture = nocter_test_support::ValueSourceFixture::new(
        "func identity(value: i32): i32 { value }\nfunc f(): (func(bool): i32, i32) { (identity, 0) }",
    );
    let failure = accepted::SourceUnit::prepare(fixture.syntax()).unwrap_err();
    assert!(
        matches!(
            failure,
            accepted::SourceFailure::Checking(accepted::Failure::Body { .. })
        ),
        "{failure:?}"
    );
}

#[test]
fn callable_initializer_annotations_reach_the_final_value_boundary_instead_of_syntax_failure() {
    for storage in ["const", "static"] {
        let source = format!(
            "func identity(value: i32): i32 {{ value }}\n{storage} ACTION: (func(i32): i32) = identity"
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let Err(accepted::SourceFailure::Declarations(declarations::Failure::Headers(diagnostics))) =
            accepted::SourceUnit::prepare(fixture.syntax())
        else {
            panic!("final callable eligibility: {source}");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source().code() == "E0323")
        );
    }
}

#[test]
fn nested_callable_annotations_match_existing_input_and_result_witnesses_without_replacing_identity()
 {
    use nocter_declarations::value_access::FunctionWitness;
    use nocter_model::{ValueFunctionOrigin, ValueInvocation};
    let source = "func identity(value: i32): i32 { value }\nfunc apply(action: func(i32): i32, value: i32): i32 { action(value) }\nfunc make(): func(i32): i32 { identity }\nfunc run(): i32 { let invoke: func(func(i32): i32, i32): i32 = apply\nlet factory: func(): func(i32): i32 = make\nlet value = factory()\ninvoke(value, 42) }";
    let all = queries(source).unwrap();
    let run = named(&all, "run");
    let program = run.program();
    for (local, name) in [(0, "apply"), (1, "make")] {
        let code = program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap();
        assert_eq!(
            run.body().locals().iter().nth(local).unwrap().1.data,
            program
                .function_data(FunctionWitness {
                    origin: ValueFunctionOrigin::Declaration(code),
                    invocation: ValueInvocation::Read
                })
                .unwrap()
        );
    }
    let fixture = nocter_test_support::ValueSourceFixture::new(source);
    accepted::SourceUnit::prepare(fixture.syntax()).unwrap();
}

#[test]
fn nested_callable_annotation_failures_report_the_contract_path_at_the_actual_value() {
    for (annotation, reason) in [
        (
            "func(func(bool): i32, i32): i32",
            "callable input 1 -> callable input 1",
        ),
        (
            "func(func(i32): bool, i32): i32",
            "callable input 1 -> callable result",
        ),
        (
            "func(mut(i32): i32, i32): i32",
            "nested callable contract requires mut",
        ),
        (
            "func(once(i32): i32, i32): i32",
            "nested callable contract requires once",
        ),
        (
            "func(func(): i32, i32): i32",
            "callable input 1 -> callable annotation requires 0 inputs",
        ),
        (
            "func(func(i32): i32, func(): i32): i32",
            "callable input 2 -> this annotation needs",
        ),
        (
            "func(take func(i32): i32, i32): i32",
            "callable input 1 requires Take",
        ),
        ("func(func(i32): i32): i32", "requires 1 inputs"),
    ] {
        let source = format!(
            "func apply(action: func(i32): i32, value: i32): i32 {{ action(value) }}\nfunc run(): void {{ let invoke: {annotation} = apply }}"
        );
        let Err(Failure::Rejected(diagnostic)) = queries(&source) else {
            panic!("{source}");
        };
        assert_eq!(diagnostic.rule(), Rule::TypeMismatch);
        assert!(
            diagnostic.source().message().contains(reason),
            "{}",
            diagnostic.source().message()
        );
        let range = diagnostic.source().primary().span().range();
        assert_eq!(
            &source[range.start().get() as usize..range.end().get() as usize],
            "apply"
        );
    }
}

#[test]
fn nested_annotation_adapts_only_the_outer_view_and_keeps_alias_and_identity_rules() {
    let prefix = "func apply(action: func(i32): i32, value: i32): i32 { action(value) }\nfunc other(action: func(i32): i32, value: i32): i32 { action(value) }\nfunc identity(value: i32): i32 { value }\n";
    assert!(queries(&format!("{prefix}func run(): i32 {{ var invoke: mut(func(i32): i32, i32): i32 = apply\ninvoke(identity, 42) }}")).is_ok());
    for body in [
        "let value = apply\nread alias: mut(func(i32): i32, i32): i32 = value",
        "var invoke: mut(func(i32): i32, i32): i32 = apply\nlet upgrade: func(func(i32): i32, i32): i32 = invoke",
        "var invoke: func(func(i32): i32, i32): i32 = apply\ninvoke = other",
    ] {
        reject(
            &format!("{prefix}func run(): void {{ {body} }}"),
            Rule::TypeMismatch,
        );
    }
}

#[test]
fn deeper_callable_signatures_do_not_invent_unqualified_anonymous_formal_trees() {
    reject(
        "func run(): void { let invoke: func(func(func(i32): i32): i32): i32 = (action) { 42 } }",
        Rule::UnknownType,
    );
}
