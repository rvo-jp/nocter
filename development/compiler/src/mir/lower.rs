//! First vertical typed-HIR-to-MIR route: a scalar integer literal returned by
//! an otherwise source-empty body.

use super::ids::{BasicBlockId, LocalId};
use super::model::{
    BasicBlock, BinaryOperator, Body, Constant, Local, LocalSource, Operand, Place, Rvalue,
    Statement, Terminator,
};
use super::validate;
use super::validate::ValidationError;
use crate::ast::{AssignmentOperator, AssignmentStmt, BindingStmt, Block, Expr, Parameter, Stmt};
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
    MissingParameterType,
    UnsupportedClaimedExpression,
    InvalidMir(Vec<ValidationError>),
}

pub(crate) fn try_build_scalar_body(
    block: &Block,
    parameters: &[Parameter],
    semantic_db: &SemanticDb,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> Option<Result<Body, BuildError>> {
    let (source_statements, expression) = scalar_body_parts(block)?;
    if !source_statements
        .iter()
        .all(|statement| statement.is_supported(resolved))
        || !scalar_expression_is_supported(expression, resolved)
    {
        return None;
    }

    let return_ty = known_expression_type(expression, typed_hir)?;
    if source_statements
        .iter()
        .any(|statement| known_expression_type(statement.value(), typed_hir) != Some(return_ty))
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
        for (index, parameter) in parameters.iter().enumerate() {
            let ty = typed_hir
                .type_id(&parameter.ty)
                .ok_or(BuildError::MissingParameterType)?;
            if ty != return_ty {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            let symbol = resolved
                .local_symbol_id_at_name_span(parameter.name_span)
                .ok_or(BuildError::MissingLocalSymbol)?;
            let local = LocalId::from_index(locals.len());
            locals.push(Local {
                ty,
                source: LocalSource::Parameter { symbol, index },
            });
            locals_by_symbol.insert(symbol, local);
        }
        let mut mir_statements = Vec::new();
        for source_statement in source_statements {
            let (local, value) = match source_statement {
                ScalarStatement::Binding(binding) => {
                    let symbol = resolved
                        .local_symbol_id_at_name_span(binding.name_span)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    let local = LocalId::from_index(locals.len());
                    locals.push(Local {
                        ty: return_ty,
                        source: LocalSource::Binding(symbol),
                    });
                    locals_by_symbol.insert(symbol, local);
                    (local, &binding.initializer)
                }
                ScalarStatement::Assignment(assignment) => {
                    let Expr::Identifier(identifier) = &assignment.target else {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    };
                    let symbol = resolved
                        .local_symbol_for_identifier(identifier)
                        .map(|symbol| symbol.id)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    (
                        *locals_by_symbol
                            .get(&symbol)
                            .ok_or(BuildError::MissingLocalSymbol)?,
                        &assignment.value,
                    )
                }
            };
            lower_expression_to_place(
                local,
                value,
                return_ty,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut mir_statements,
            )?;
        }
        lower_expression_to_place(
            return_local,
            expression,
            return_ty,
            resolved,
            &locals_by_symbol,
            typed_hir,
            &mut locals,
            &mut mir_statements,
        )?;
        let body = Body {
            source_body,
            source_span: block.span,
            return_local,
            locals,
            entry: BasicBlockId::from_index(0),
            blocks: vec![
                BasicBlock {
                    statements: mir_statements,
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

#[derive(Debug, Clone, Copy)]
enum ScalarStatement<'a> {
    Binding(&'a BindingStmt),
    Assignment(&'a AssignmentStmt),
}

impl<'a> ScalarStatement<'a> {
    fn value(self) -> &'a Expr {
        match self {
            Self::Binding(binding) => &binding.initializer,
            Self::Assignment(assignment) => &assignment.value,
        }
    }

    fn is_supported(self, resolved: &ResolveOutput) -> bool {
        match self {
            Self::Binding(binding) => {
                scalar_expression_is_supported(&binding.initializer, resolved)
            }
            Self::Assignment(assignment) => {
                assignment.operator == AssignmentOperator::Assign
                    && matches!(&assignment.target, Expr::Identifier(identifier) if resolved.local_symbol_for_identifier(identifier).is_some())
                    && scalar_expression_is_supported(&assignment.value, resolved)
            }
        }
    }
}

fn scalar_body_parts(block: &Block) -> Option<(Vec<ScalarStatement<'_>>, &Expr)> {
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
            Stmt::Binding(binding) => Some(ScalarStatement::Binding(binding)),
            Stmt::Assignment(assignment) => Some(ScalarStatement::Assignment(assignment)),
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
        Expr::Binary(binary) => {
            mir_binary_operator(binary.operator).is_some()
                && scalar_expression_is_supported(&binary.left, resolved)
                && scalar_expression_is_supported(&binary.right, resolved)
        }
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

fn lower_expression_to_place(
    destination: LocalId,
    expression: &Expr,
    ty: crate::semantic::TyId,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
    typed_hir: &TypedHir,
    local_declarations: &mut Vec<Local>,
    statements: &mut Vec<Statement>,
) -> Result<(), BuildError> {
    let source = typed_hir
        .expression(expression.span())
        .ok_or(BuildError::MissingTypedExpression)?
        .id;
    let value = match expression {
        Expr::Binary(binary) => Rvalue::Binary {
            operator: mir_binary_operator(binary.operator)
                .ok_or(BuildError::UnsupportedClaimedExpression)?,
            left: lower_operand(
                &binary.left,
                ty,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                statements,
            )?,
            right: lower_operand(
                &binary.right,
                ty,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                statements,
            )?,
            ty,
        },
        Expr::Group(group) => {
            return lower_expression_to_place(
                destination,
                &group.expression,
                ty,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                statements,
            );
        }
        _ => Rvalue::Use(lower_simple_operand(expression, ty, resolved, locals)?),
    };
    statements.push(Statement::Assign {
        destination: Place { local: destination },
        value,
        source,
    });
    Ok(())
}

fn lower_operand(
    expression: &Expr,
    ty: crate::semantic::TyId,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
    typed_hir: &TypedHir,
    local_declarations: &mut Vec<Local>,
    statements: &mut Vec<Statement>,
) -> Result<Operand, BuildError> {
    if !matches!(expression, Expr::Binary(_)) {
        return match expression {
            Expr::Group(group) => lower_operand(
                &group.expression,
                ty,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                statements,
            ),
            _ => lower_simple_operand(expression, ty, resolved, locals),
        };
    }

    let typed_expression = typed_hir
        .expression(expression.span())
        .ok_or(BuildError::MissingTypedExpression)?;
    let temporary = LocalId::from_index(local_declarations.len());
    local_declarations.push(Local {
        ty,
        source: LocalSource::Temporary(typed_expression.id),
    });
    lower_expression_to_place(
        temporary,
        expression,
        ty,
        resolved,
        locals,
        typed_hir,
        local_declarations,
        statements,
    )?;
    Ok(Operand::Copy(Place { local: temporary }))
}

fn lower_simple_operand(
    expression: &Expr,
    ty: crate::semantic::TyId,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
) -> Result<Operand, BuildError> {
    match expression {
        Expr::IntegerLiteral(literal) => Ok(Operand::Constant(Constant {
            ty,
            value: decode_integer_literal_value(&literal.value)
                .ok_or(BuildError::InvalidIntegerConstant)?,
        })),
        Expr::Identifier(identifier) => {
            let symbol = resolved
                .local_symbol_for_identifier(identifier)
                .map(|symbol| symbol.id)
                .ok_or(BuildError::MissingLocalSymbol)?;
            Ok(Operand::Copy(Place {
                local: *locals.get(&symbol).ok_or(BuildError::MissingLocalSymbol)?,
            }))
        }
        _ => Err(BuildError::UnsupportedClaimedExpression),
    }
}

fn mir_binary_operator(operator: crate::ast::BinaryOperator) -> Option<BinaryOperator> {
    match operator {
        crate::ast::BinaryOperator::Add => Some(BinaryOperator::Add),
        crate::ast::BinaryOperator::Subtract => Some(BinaryOperator::Subtract),
        crate::ast::BinaryOperator::Multiply => Some(BinaryOperator::Multiply),
        crate::ast::BinaryOperator::Divide => Some(BinaryOperator::Divide),
        crate::ast::BinaryOperator::Remainder => Some(BinaryOperator::Remainder),
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
        let body = try_build_scalar_body(
            block,
            &[],
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
}
