use super::*;

#[test]
fn grouped_flat_callable_annotations_share_ordinary_witness_checking() {
    let source = "func identity(value: i32): i32 { value }\nfunc apply(action: ((func(i32): i32)), value: i32): i32 { action(value) }\nfunc f(): i32 { let action: (func(i32): i32) = identity\napply(action, 42) }";
    assert!(queries(source).is_ok());
}

#[test]
fn nested_callable_data_reports_missing_type_evidence_at_the_current_header() {
    for source in [
        "struct Data { action: func(): i32 }",
        "func f(input: (func(): i32)?): void {}",
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
