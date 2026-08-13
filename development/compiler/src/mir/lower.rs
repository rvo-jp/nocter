//! First vertical typed-HIR-to-MIR route: a scalar integer literal returned by
//! an otherwise source-empty body.

use super::ids::{BasicBlockId, LocalId};
use super::model::{
    BasicBlock, Body, Constant, Local, LocalSource, Operand, Place, Rvalue, Statement, Terminator,
};
use super::validate;
use super::validate::ValidationError;
use crate::ast::{BindingStmt, Block, Expr, Stmt};
use crate::literals::decode_integer_literal_value;
use crate::resolve::{LocalSymbolId, ResolveOutput};
use crate::semantic::SemanticDb;
use crate::typecheck::{PartialSemantic, TypedHir};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildError {
    MissingSourceBody,
    MissingTypedExpression,
    InvalidIntegerConstant,
    MissingLocalSymbol,
    UnsupportedClaimedExpression,
    InvalidMir(Vec<ValidationError>),
}

pub(crate) fn try_build_scalar_body(
    block: &Block,
    semantic_db: &SemanticDb,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> Option<Result<Body, BuildError>> {
    let (bindings, expression) = scalar_body_parts(block)?;
    if !bindings
        .iter()
        .all(|binding| scalar_expression_is_supported(&binding.initializer, resolved))
        || !scalar_expression_is_supported(expression, resolved)
    {
        return None;
    }

    let return_ty = known_expression_type(expression, typed_hir)?;
    if bindings
        .iter()
        .any(|binding| known_expression_type(&binding.initializer, typed_hir) != Some(return_ty))
    {
        return None;
    }

    Some((|| {
        let source_body = semantic_db
            .body_at(block.span)
            .ok_or(BuildError::MissingSourceBody)?;
        let return_local = LocalId::from_index(0);
        let mut locals = vec![Local {
            ty: return_ty,
            source: LocalSource::Return,
        }];
        let mut locals_by_symbol = HashMap::new();
        let mut statements = Vec::new();
        for binding in bindings {
            let symbol = resolved
                .local_symbol_id_at_name_span(binding.name_span)
                .ok_or(BuildError::MissingLocalSymbol)?;
            let local = LocalId::from_index(locals.len());
            locals.push(Local {
                ty: return_ty,
                source: LocalSource::Binding(symbol),
            });
            locals_by_symbol.insert(symbol, local);
            statements.push(assign_expression(
                local,
                &binding.initializer,
                return_ty,
                resolved,
                &locals_by_symbol,
                typed_hir,
            )?);
        }
        statements.push(assign_expression(
            return_local,
            expression,
            return_ty,
            resolved,
            &locals_by_symbol,
            typed_hir,
        )?);
        let body = Body {
            source_body,
            source_span: block.span,
            return_local,
            locals,
            entry: BasicBlockId::from_index(0),
            blocks: vec![
                BasicBlock {
                    statements,
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

fn scalar_body_parts(block: &Block) -> Option<(Vec<&BindingStmt>, &Expr)> {
    let runtime_statements = block
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)))
        .collect::<Vec<_>>();
    let (binding_statements, result) = if let Some(result) = block.result.as_deref() {
        (runtime_statements.as_slice(), result)
    } else {
        let (last, leading) = runtime_statements.split_last()?;
        let Stmt::Return(statement) = last else {
            return None;
        };
        (leading, statement.expression.as_ref()?)
    };
    let bindings = binding_statements
        .iter()
        .map(|statement| match statement {
            Stmt::Binding(binding) => Some(binding),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((bindings, result))
}

fn scalar_expression_is_supported(expression: &Expr, resolved: &ResolveOutput) -> bool {
    match expression {
        Expr::IntegerLiteral(literal) => decode_integer_literal_value(&literal.value).is_some(),
        Expr::Identifier(identifier) => resolved.local_symbol_for_identifier(identifier).is_some(),
        Expr::Group(group) => scalar_expression_is_supported(&group.expression, resolved),
        _ => false,
    }
}

fn known_expression_type(expression: &Expr, typed_hir: &TypedHir) -> Option<crate::semantic::TyId> {
    let expression = typed_hir.expression(expression.span())?;
    let PartialSemantic::Known(ty) = expression.ty else {
        return None;
    };
    Some(ty)
}

fn assign_expression(
    destination: LocalId,
    expression: &Expr,
    ty: crate::semantic::TyId,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
    typed_hir: &TypedHir,
) -> Result<Statement, BuildError> {
    let source = typed_hir
        .expression(expression.span())
        .ok_or(BuildError::MissingTypedExpression)?
        .id;
    let operand = match expression {
        Expr::IntegerLiteral(literal) => Operand::Constant(Constant {
            ty,
            value: decode_integer_literal_value(&literal.value)
                .ok_or(BuildError::InvalidIntegerConstant)?,
        }),
        Expr::Identifier(identifier) => {
            let symbol = resolved
                .local_symbol_for_identifier(identifier)
                .map(|symbol| symbol.id)
                .ok_or(BuildError::MissingLocalSymbol)?;
            Operand::Copy(Place {
                local: *locals.get(&symbol).ok_or(BuildError::MissingLocalSymbol)?,
            })
        }
        Expr::Group(group) => {
            return assign_expression(
                destination,
                &group.expression,
                ty,
                resolved,
                locals,
                typed_hir,
            );
        }
        _ => return Err(BuildError::UnsupportedClaimedExpression),
    };
    Ok(Statement::Assign {
        destination: Place { local: destination },
        value: Rvalue::Use(operand),
        source,
    })
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
        let body = try_build_scalar_body(
            block,
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
    value = 7
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
}
