use super::*;
use nocter_declarations::value_access::{ArraySchema, PayloadSchema};
use nocter_diagnostics::SourceDiagnostic;

fn array(unit: &Unit, record: &str, position: usize) -> ArraySchema {
    let program = unit.program();
    let record = program
        .records()
        .get(
            program
                .resolve_record(program.symbols().get(record).unwrap())
                .unwrap(),
        )
        .unwrap();
    let data = program
        .fields()
        .get(record.fields()[position])
        .unwrap()
        .data();
    let Some(PayloadSchema::Array(array)) = program.data_schema(data) else {
        panic!("array field");
    };
    array
}
fn diagnostic(failure: &SourceFailure) -> &SourceDiagnostic {
    match failure {
        SourceFailure::Declarations(declarations::Failure::Headers(errors)) => errors[0].source(),
        SourceFailure::Checking(Failure::Body {
            failure: crate::value_access::Failure::Rejected(error),
            ..
        }) => error.source(),
        SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) => diagnostic.source(),
        SourceFailure::Checking(Failure::Const { diagnostic, .. }) => diagnostic.source(),
        other => panic!("expected an authored source rejection: {other:?}"),
    }
}

#[test]
fn computed_array_fields_close_through_the_ordinary_source_unit() {
    let text = "struct Data { first: [i32; width()]\nsecond: [i32; 6 * 7] }\nconst BASE: usize = 40\nconst func width(): usize { let two: usize = 2\nlet pair = (BASE, two)\npair.0 + pair.1 }\nfunc run(): i32 { if width() == 42 { 42 } else { 0 } }";
    let (sources, tree) = input(text);
    let source = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    assert_eq!(
        array(source.unit(), "Data", 0),
        array(source.unit(), "Data", 1)
    );
    assert_eq!(array(source.unit(), "Data", 0).length(), 42);
    let context = source.declarations().bind(bound(&sources, &tree)).unwrap();
    source.unit().bind(&context).unwrap();
}

#[test]
fn computed_zero_breaks_containment_while_maximum_counts_remain_compact() {
    for copy in ["", "copy "] {
        let text = format!(
            "{copy}struct Node {{ next: [Node; zero()] }}\nconst func zero(): usize {{ 0 }}\nfunc run(): i32 {{ 42 }}"
        );
        let (sources, tree) = input(&text);
        let source = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
        let field = array(source.unit(), "Node", 0);
        assert!(field.is_empty());
        assert_eq!(
            source.unit().program().data_copyable(field.element()),
            Some(!copy.is_empty())
        );
    }
    let text = "copy struct Huge { values: [[error; 0]; maximum()] }\nconst func maximum(): usize { 18446744073709551615 }\nfunc run(): i32 { 42 }";
    let (sources, tree) = input(text);
    let source = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    let outer = array(source.unit(), "Huge", 0);
    assert_eq!(outer.length(), u64::MAX);
    let Some(PayloadSchema::Array(inner)) = source.unit().program().data_schema(outer.element())
    else {
        panic!("inner array");
    };
    assert!(inner.is_empty());
}

#[test]
fn header_reuse_guards_transitive_suppliers_without_requiring_unrelated_bodies() {
    let text = "struct Data { values: [i32; width()] }\nconst BASE: usize = 40\nconst func width(): usize { BASE + 2 }\nconst UNUSED: usize = 7\nfunc other(): i32 { 9 }\nfunc run(): i32 { 42 }";
    let (sources, tree) = input(text);
    let source = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    for (before, after, valid) in [
        ("BASE: usize = 40", "BASE: usize = 41", false),
        ("BASE + 2", "BASE + 3", false),
        ("UNUSED: usize = 7", "UNUSED: usize = 8", true),
        ("other(): i32 { 9 }", "other(): i32 { 10 }", true),
    ] {
        let (next_sources, next_tree) = input(&text.replace(before, after));
        assert_eq!(
            source
                .declarations()
                .bind(bound(&next_sources, &next_tree))
                .is_ok(),
            valid,
            "{before}"
        );
    }
    let (next_sources, next_tree) = input(&text.replace("BASE: usize = 40", "BASE: usize = 41"));
    let fresh = SourceUnit::prepare(bound(&next_sources, &next_tree)).unwrap();
    assert_eq!(array(fresh.unit(), "Data", 0).length(), 43);
}

#[test]
fn array_length_rejections_keep_ordinary_typing_const_and_arithmetic_causes() {
    for (length, helper, code) in [
        ("true", "", "E0370"),
        ("missing", "", "E0340"),
        ("1 / 0", "", "E0325"),
        (
            "if false { runtime() } else { 0 }",
            "\nfunc runtime(): usize { 0 }",
            "E0421",
        ),
    ] {
        let text =
            format!("struct Data {{ values: [i32; {length}] }}{helper}\nfunc run(): i32 {{ 42 }}");
        let (sources, tree) = input(&text);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert_eq!(diagnostic(&failure).code(), code, "{text}: {failure:?}");
        assert!(!diagnostic(&failure).primary().span().range().is_empty());
    }
}

#[test]
fn positive_containment_and_independent_invalid_elements_still_reject() {
    for (text, code) in [
        (
            "struct Data { values: [Data; one()] }\nconst func one(): usize { 1 }",
            "E0291",
        ),
        ("struct Data { values: [Missing; 0] }", "E0290"),
        (
            "struct Data { values: [Bad; 0] }\nstruct Bad { value: Missing }",
            "E0290",
        ),
        (
            "struct Item {}\ncopy struct Data { values: [Item; 1] }",
            "E0291",
        ),
    ] {
        let (sources, tree) = input(text);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert_eq!(diagnostic(&failure).code(), code, "{text}: {failure:?}");
    }
}

#[test]
fn type_value_cycles_report_real_declarations_and_source_sites() {
    for use_site in ["let _ = inspect()", "if false { let _ = inspect() }"] {
        let text = format!(
            "struct Loop {{ values: [i32; count()] }}\nconst func count(): usize {{ {use_site}\n0 }}\nconst func inspect(): Loop {{ loop {{}} }}"
        );
        let (sources, tree) = input(&text);
        let syntax = bound(&sources, &tree);
        let failure = SourceUnit::prepare(syntax).unwrap_err();
        let error = diagnostic(&failure);
        assert_eq!(error.code(), "E0324", "{failure:?}");
        assert!(error.message().contains("header Loop"));
        assert!(error.message().contains("type expression in Loop"));
        assert_eq!(
            syntax.text_at(error.primary().span().range()),
            Some("inspect")
        );
    }
}

#[test]
fn whole_array_contracts_close_without_claiming_element_selection_or_storage() {
    for text in [
        "func f(take values: [i32; 2]): void {}",
        "func f(): [i32; 0] { loop {} }",
        "struct Data { values: [i32; 2] }\nfunc f(take value: Data): void {}",
        "func f(action: func(edit [i32; 2]): void): void {}",
    ] {
        let (sources, tree) = input(text);
        let source = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
        let context = source.declarations().bind(bound(&sources, &tree)).unwrap();
        source.unit().bind(&context).unwrap();
        let code = source
            .unit()
            .program()
            .resolve_code(source.unit().program().symbols().get("f").unwrap())
            .unwrap();
        assert!(source.unit().body(code).is_some());
    }
}

#[test]
fn count_computation_preserves_edit_input_writes_and_unsigned_result_typing() {
    let text = "struct Data { values: [i32; width()] }\nconst func increment(edit value: usize): usize { value += 1\nvalue }\nconst func width(): usize { var value: usize = 20\nlet first = increment(value)\nfirst + value }\nfunc run(): i32 { 42 }";
    let (sources, tree) = input(text);
    let source = SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    assert_eq!(array(source.unit(), "Data", 0).length(), 42);
    for (length, helper) in [
        ("COUNT", "\nconst COUNT: i32 = 1"),
        ("count()", "\nconst func count(): i32 { 1 }"),
        ("-1", ""),
        ("18446744073709551616", ""),
    ] {
        let text = format!("struct Data {{ values: [i32; {length}] }}{helper}");
        let (sources, tree) = input(&text);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        let SourceFailure::Checking(Failure::Body {
            failure: crate::value_access::Failure::Rejected(error),
            ..
        }) = failure
        else {
            panic!("ordinary usize rejection: {failure:?}");
        };
        assert!(
            matches!(
                error.rule(),
                crate::value_access::Rule::TypeMismatch | crate::value_access::Rule::IntegerRange
            ),
            "{text}: {error:?}"
        );
    }
}

#[test]
fn recursive_const_calls_and_nontermination_keep_execution_budget_errors() {
    use std::num::{NonZeroU32, NonZeroU64};
    let limits = nocter_constant_evaluation::CompileTimeEvaluationLimits::new(
        NonZeroU64::new(64).unwrap(),
        NonZeroU32::new(4).unwrap(),
    );
    for body in ["count()", "loop {}"] {
        let text = format!(
            "struct Data {{ values: [i32; count()] }}\nconst func count(): usize {{ {body} }}"
        );
        let (sources, tree) = input(&text);
        let failure = SourceUnit::prepare_with_limits(bound(&sources, &tree), limits).unwrap_err();
        assert_eq!(diagnostic(&failure).code(), "E0322", "{failure:?}");
    }
}

#[test]
fn runtime_parameters_do_not_become_implicit_inputs_to_header_lengths() {
    let (sources, tree) = input("func f(size: usize): [i32; size] { loop {} }");
    let syntax = bound(&sources, &tree);
    let failure = SourceUnit::prepare(syntax).unwrap_err();
    let error = diagnostic(&failure);
    assert_eq!(error.code(), "E0340");
    assert_eq!(syntax.text_at(error.primary().span().range()), Some("size"));
}
