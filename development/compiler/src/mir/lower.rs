//! First vertical typed-HIR-to-MIR route: a scalar integer literal returned by
//! an otherwise source-empty body.

use super::ids::{BasicBlockId, LocalId};
use super::model::{
    BasicBlock, Body, Constant, Local, Operand, Place, Rvalue, Statement, Terminator,
};
use super::validate;
use super::validate::ValidationError;
use crate::ast::{Block, Expr, Stmt};
use crate::literals::decode_integer_literal_value;
use crate::semantic::SemanticDb;
use crate::typecheck::{PartialSemantic, TypedHir};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildError {
    MissingSourceBody,
    MissingTypedExpression,
    ErroneousTypedExpression,
    InvalidIntegerConstant,
    InvalidMir(Vec<ValidationError>),
}

pub(crate) fn try_build_scalar_literal_body(
    block: &Block,
    semantic_db: &SemanticDb,
    typed_hir: &TypedHir,
) -> Option<Result<Body, BuildError>> {
    let expression = scalar_literal_result(block)?;
    let literal = ungroup_integer_literal(expression)?;

    Some((|| {
        let source_body = semantic_db
            .body_at(block.span)
            .ok_or(BuildError::MissingSourceBody)?;
        let typed_expression = typed_hir
            .expression(expression.span())
            .ok_or(BuildError::MissingTypedExpression)?;
        let PartialSemantic::Known(ty) = typed_expression.ty else {
            return Err(BuildError::ErroneousTypedExpression);
        };
        let value = decode_integer_literal_value(&literal.value)
            .ok_or(BuildError::InvalidIntegerConstant)?;
        let return_local = LocalId::from_index(0);
        let body = Body {
            source_body,
            source_span: block.span,
            return_local,
            locals: vec![Local {
                ty,
                source: Some(expression.span()),
            }],
            entry: BasicBlockId::from_index(0),
            blocks: vec![
                BasicBlock {
                    statements: vec![Statement::Assign {
                        destination: Place {
                            local: return_local,
                        },
                        value: Rvalue::Use(Operand::Constant(Constant { ty, value })),
                        source: typed_expression.id,
                    }],
                    terminator: Terminator::Goto {
                        target: BasicBlockId::from_index(1),
                    },
                },
                BasicBlock {
                    statements: Vec::new(),
                    terminator: Terminator::Return,
                },
            ],
        };
        validate(&body).map_err(BuildError::InvalidMir)?;
        Ok(body)
    })())
}

fn scalar_literal_result(block: &Block) -> Option<&Expr> {
    let mut runtime_statements = block
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)));
    match (
        runtime_statements.next(),
        runtime_statements.next(),
        &block.result,
    ) {
        (None, None, Some(result)) => Some(result),
        (Some(Stmt::Return(statement)), None, None) => statement.expression.as_ref(),
        _ => None,
    }
}

fn ungroup_integer_literal(expression: &Expr) -> Option<&crate::ast::LiteralExpr> {
    match expression {
        Expr::IntegerLiteral(literal) => Some(literal),
        Expr::Group(group) => ungroup_integer_literal(&group.expression),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;
    use crate::ast::Item;

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
        let body = try_build_scalar_literal_body(block, &analysis.semantic_db, &file.typed_hir)
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
    let value = 42
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
            try_build_scalar_literal_body(block, &analysis.semantic_db, &file.typed_hir).is_none()
        );
    }
}
