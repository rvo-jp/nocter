use super::*;
use crate::analysis::test_support::analyze_text;
use crate::ast::{Item, Stmt};

#[test]
fn builds_typed_control_flow_for_a_scalar_literal_body() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return 42
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let block = function.body.as_ref().unwrap();
    let body = try_build_scalar_body(
        block,
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("the source shape must select MIR")
    .unwrap();

    assert_eq!(body.source_span, block.span);
    assert_eq!(body.blocks.len(), 2);
    assert_eq!(body.blocks[0].statements.len(), 1);
    assert_eq!(
        body.blocks[0].terminator,
        Terminator::Goto {
            target: BasicBlockId::from_index(1),
        }
    );
    assert_eq!(body.blocks[1].terminator, Terminator::Return);
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn does_not_claim_a_body_with_runtime_statements() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    var value = 42
    value += 7
    return value
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let block = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => function.body.as_ref(),
            _ => None,
        })
        .unwrap();

    assert!(
        try_build_scalar_body(
            block,
            &[],
            ScalarType::I32,
            &analysis.semantic_db,
            &file.resolved,
            &file.typed_hir,
        )
        .is_none()
    );
}

#[test]
fn keys_straight_line_bindings_by_resolved_local_identity() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    let value = 42
    return value
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let block = function.body.as_ref().unwrap();
    let body = try_build_scalar_body(
        block,
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("straight-line scalar bindings must select MIR")
    .unwrap();

    let Stmt::Binding(binding) = &block.statements[0] else {
        panic!("expected binding");
    };
    let symbol = file
        .resolved
        .local_symbol_id_at_name_span(binding.name_span)
        .unwrap();
    assert_eq!(body.locals.len(), 2);
    assert_eq!(body.locals[1].source, LocalSource::Binding(symbol));
    assert_eq!(body.blocks[0].statements.len(), 2);
    assert!(matches!(
        body.blocks[0].statements[1],
        Statement::Assign {
            value: Rvalue::Use(Operand::Copy(_)),
            ..
        }
    ));
}

#[test]
fn makes_nested_scalar_evaluation_order_explicit_with_a_temporary() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return (1 + 2) * 3
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let block = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => function.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        block,
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("scalar arithmetic must select MIR")
    .unwrap();

    assert_eq!(body.locals.len(), 2);
    assert!(matches!(body.locals[1].source, LocalSource::Temporary(_)));
    assert_eq!(body.blocks[0].statements.len(), 2);
    assert!(body.blocks[0].statements.iter().all(|statement| matches!(
        statement,
        Statement::Assign {
            value: Rvalue::Binary { .. },
            ..
        }
    )));
    assert_eq!(validate(&body), Ok(()));
}
