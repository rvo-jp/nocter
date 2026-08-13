//! Scalar statement and natural-loop CFG construction.

use super::BuildError;
use super::body_builder::ControlFlowBuilder;
use super::coverage::{
    ScalarStatement, binding_scalar_type, known_expression_type, scalar_linear_block_statements,
    scalar_loop_block_statements,
};
use super::expressions::{lower_expression_to_place, lower_operand, mir_comparison_operator};
use crate::ast::Expr;
use crate::mir::{
    BinaryOperator, ComparisonOperator, LocalId, LocalSource, Operand, Origin, Place, Rvalue,
    ScalarType, Statement, Terminator,
};
use crate::resolve::{LocalSymbolId, ResolveOutput};
use crate::typecheck::TypedHir;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
struct LoopTargets {
    break_target: crate::mir::BasicBlockId,
    continue_target: crate::mir::BasicBlockId,
}

pub(super) struct StatementLowerer<'a> {
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
    locals: &'a mut Vec<crate::mir::model::Local>,
    locals_by_symbol: &'a mut HashMap<LocalSymbolId, LocalId>,
    control_flow: &'a mut ControlFlowBuilder,
    loop_regions: &'a mut Vec<crate::mir::LoopRegion>,
}

impl<'a> StatementLowerer<'a> {
    pub(super) fn new(
        resolved: &'a ResolveOutput,
        typed_hir: &'a TypedHir,
        locals: &'a mut Vec<crate::mir::model::Local>,
        locals_by_symbol: &'a mut HashMap<LocalSymbolId, LocalId>,
        control_flow: &'a mut ControlFlowBuilder,
        loop_regions: &'a mut Vec<crate::mir::LoopRegion>,
    ) -> Self {
        Self {
            resolved,
            typed_hir,
            locals,
            locals_by_symbol,
            control_flow,
            loop_regions,
        }
    }

    pub(super) fn lower(&mut self, statements: &[ScalarStatement<'_>]) -> Result<(), BuildError> {
        if self.lower_in_context(statements, None)? {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        Ok(())
    }

    fn lower_in_context(
        &mut self,
        statements: &[ScalarStatement<'_>],
        loop_targets: Option<LoopTargets>,
    ) -> Result<bool, BuildError> {
        for statement in statements {
            let exits_block = match *statement {
                ScalarStatement::Binding(binding) => {
                    let symbol = self
                        .resolved
                        .local_symbol_id_at_name_span(binding.name_span)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    let ty = self
                        .typed_hir
                        .binding_type_expr(symbol)
                        .and_then(|ty| self.typed_hir.type_id(ty))
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let scalar = binding_scalar_type(symbol, self.typed_hir)
                        .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    let local = LocalId::from_index(self.locals.len());
                    self.locals.push(crate::mir::model::Local {
                        ty,
                        scalar,
                        source: LocalSource::Binding(symbol),
                    });
                    self.locals_by_symbol.insert(symbol, local);
                    self.lower_value(local, &binding.initializer, ty, scalar)?;
                    false
                }
                ScalarStatement::Assignment(assignment) => {
                    let Expr::Identifier(identifier) = &assignment.target else {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    };
                    let symbol = self
                        .resolved
                        .local_symbol_for_identifier(identifier)
                        .map(|symbol| symbol.id)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    let local = *self
                        .locals_by_symbol
                        .get(&symbol)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    let declaration = &self.locals[local.index()];
                    let ty = declaration.ty;
                    let scalar = declaration.scalar;
                    self.lower_value(local, &assignment.value, ty, scalar)?;
                    false
                }
                ScalarStatement::While(statement) => {
                    self.lower_while(statement)?;
                    false
                }
                ScalarStatement::If(statement) => self.lower_if(statement, loop_targets)?,
                ScalarStatement::ForRange(statement) => {
                    self.lower_for_range(statement)?;
                    false
                }
                ScalarStatement::Loop(statement) => {
                    self.lower_loop(statement)?;
                    false
                }
                ScalarStatement::Break => {
                    let targets = loop_targets.ok_or(BuildError::UnsupportedClaimedExpression)?;
                    self.control_flow.terminate(Terminator::Goto {
                        target: targets.break_target,
                    })?;
                    true
                }
                ScalarStatement::Continue => {
                    let targets = loop_targets.ok_or(BuildError::UnsupportedClaimedExpression)?;
                    self.control_flow.terminate(Terminator::Goto {
                        target: targets.continue_target,
                    })?;
                    true
                }
            };
            if exits_block {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn lower_value(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        ty: crate::semantic::TyId,
        scalar: ScalarType,
    ) -> Result<(), BuildError> {
        lower_expression_to_place(
            destination,
            expression,
            ty,
            scalar,
            self.resolved,
            self.locals_by_symbol,
            self.typed_hir,
            self.locals,
            self.control_flow,
        )
    }

    fn lower_while(&mut self, statement: &crate::ast::WhileStmt) -> Result<(), BuildError> {
        let condition_target = self.control_flow.reserve_block();
        let body_target = self.control_flow.reserve_block();
        let exit_target = self.control_flow.reserve_block();
        self.control_flow.terminate(Terminator::Goto {
            target: condition_target,
        })?;

        self.control_flow.select_block(condition_target)?;
        let condition_ty = known_expression_type(&statement.condition, self.typed_hir)
            .ok_or(BuildError::MissingTypedExpression)?;
        let condition = lower_operand(
            &statement.condition,
            condition_ty,
            ScalarType::Bool,
            self.resolved,
            self.locals_by_symbol,
            self.typed_hir,
            self.locals,
            self.control_flow,
        )?;
        if matches!(
            &statement.condition,
            Expr::Binary(binary) if mir_comparison_operator(binary.operator).is_some()
        ) && let Operand::Copy(place) = &condition
        {
            self.locals[place.local.index()].source =
                LocalSource::Virtual(statement.condition.span());
        }
        let condition_block = self.control_flow.current_block()?;
        self.control_flow.terminate(Terminator::Switch {
            condition,
            then_target: body_target,
            else_target: exit_target,
        })?;
        self.loop_regions.push(crate::mir::LoopRegion {
            header: condition_target,
            condition: condition_block,
            body: body_target,
            continue_target: condition_target,
            exit: exit_target,
        });

        self.control_flow.select_block(body_target)?;
        let body_statements =
            scalar_loop_block_statements(&statement.body, self.resolved, self.typed_hir)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let exits_body = self.lower_in_context(
            &body_statements,
            Some(LoopTargets {
                break_target: exit_target,
                continue_target: condition_target,
            }),
        )?;
        if !exits_body {
            self.control_flow.terminate(Terminator::Goto {
                target: condition_target,
            })?;
        }
        self.control_flow.select_block(exit_target)
    }

    fn lower_if(
        &mut self,
        statement: &crate::ast::IfStmt,
        loop_targets: Option<LoopTargets>,
    ) -> Result<bool, BuildError> {
        let condition_ty = known_expression_type(&statement.condition, self.typed_hir)
            .ok_or(BuildError::MissingTypedExpression)?;
        let condition = lower_operand(
            &statement.condition,
            condition_ty,
            ScalarType::Bool,
            self.resolved,
            self.locals_by_symbol,
            self.typed_hir,
            self.locals,
            self.control_flow,
        )?;
        let then_target = self.control_flow.reserve_block();
        let else_target = self.control_flow.reserve_block();
        let join_target = self.control_flow.reserve_block();
        self.control_flow.terminate(Terminator::Switch {
            condition,
            then_target,
            else_target,
        })?;

        self.control_flow.select_block(then_target)?;
        let then_statements = scalar_linear_block_statements(
            &statement.then_block,
            self.resolved,
            self.typed_hir,
            loop_targets.is_some(),
        )
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let then_exits = self.lower_in_context(&then_statements, loop_targets)?;
        if !then_exits {
            self.control_flow.terminate(Terminator::Goto {
                target: join_target,
            })?;
        }

        self.control_flow.select_block(else_target)?;
        let else_statements = statement
            .else_block
            .as_ref()
            .map(|block| {
                scalar_linear_block_statements(
                    block,
                    self.resolved,
                    self.typed_hir,
                    loop_targets.is_some(),
                )
                .ok_or(BuildError::UnsupportedClaimedExpression)
            })
            .transpose()?
            .unwrap_or_default();
        let else_exits = self.lower_in_context(&else_statements, loop_targets)?;
        if !else_exits {
            self.control_flow.terminate(Terminator::Goto {
                target: join_target,
            })?;
        }

        if then_exits && else_exits {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        self.control_flow.select_block(join_target)?;
        Ok(false)
    }

    fn lower_for_range(&mut self, statement: &crate::ast::ForRangeStmt) -> Result<(), BuildError> {
        let symbol = self
            .resolved
            .local_symbol_id_at_name_span(statement.name_span)
            .ok_or(BuildError::MissingLocalSymbol)?;
        let ty = self
            .typed_hir
            .binding_type_expr(symbol)
            .and_then(|ty| self.typed_hir.type_id(ty))
            .ok_or(BuildError::MissingTypedExpression)?;
        let scalar = binding_scalar_type(symbol, self.typed_hir)
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let value = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::model::Local {
            ty,
            scalar,
            source: LocalSource::Binding(symbol),
        });
        self.locals_by_symbol.insert(symbol, value);
        self.lower_value(value, &statement.start, ty, scalar)?;

        let end = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::model::Local {
            ty,
            scalar,
            source: LocalSource::Desugared(statement.range_span),
        });
        self.lower_value(end, &statement.end, ty, scalar)?;

        let bool_ty = self
            .typed_hir
            .type_id(&crate::ast::TypeExpr::Reference(
                crate::ast::TypeReference {
                    span: statement.range_span,
                    name: "bool".to_string(),
                },
            ))
            .ok_or(BuildError::MissingTypedExpression)?;
        let condition_local = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::model::Local {
            ty: bool_ty,
            scalar: ScalarType::Bool,
            source: LocalSource::Virtual(statement.range_span),
        });

        let header = self.control_flow.reserve_block();
        let body = self.control_flow.reserve_block();
        let increment = self.control_flow.reserve_block();
        let exit = self.control_flow.reserve_block();
        self.control_flow
            .terminate(Terminator::Goto { target: header })?;
        self.control_flow.select_block(header)?;
        self.control_flow.push_statement(Statement::Assign {
            destination: Place {
                local: condition_local,
            },
            value: Rvalue::Compare {
                operator: ComparisonOperator::Less,
                left: Operand::Copy(Place { local: value }),
                right: Operand::Copy(Place { local: end }),
                operand_ty: ty,
                operand_scalar: scalar,
                result_ty: bool_ty,
            },
            origin: Origin::Desugared(statement.range_span),
        })?;
        self.control_flow.terminate(Terminator::Switch {
            condition: Operand::Copy(Place {
                local: condition_local,
            }),
            then_target: body,
            else_target: exit,
        })?;
        self.loop_regions.push(crate::mir::LoopRegion {
            header,
            condition: header,
            body,
            continue_target: increment,
            exit,
        });

        self.control_flow.select_block(body)?;
        let body_statements =
            scalar_loop_block_statements(&statement.body, self.resolved, self.typed_hir)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let exits_body = self.lower_in_context(
            &body_statements,
            Some(LoopTargets {
                break_target: exit,
                continue_target: increment,
            }),
        )?;
        if !exits_body {
            self.control_flow
                .terminate(Terminator::Goto { target: increment })?;
        }

        self.control_flow.select_block(increment)?;
        self.control_flow.push_statement(Statement::Assign {
            destination: Place { local: value },
            value: Rvalue::Binary {
                operator: BinaryOperator::Add,
                left: Operand::Copy(Place { local: value }),
                right: Operand::Constant(crate::mir::model::Constant {
                    ty,
                    scalar,
                    value: 1,
                }),
                ty,
            },
            origin: Origin::Desugared(statement.range_span),
        })?;
        self.control_flow
            .terminate(Terminator::Goto { target: header })?;
        self.control_flow.select_block(exit)
    }

    fn lower_loop(&mut self, statement: &crate::ast::LoopStmt) -> Result<(), BuildError> {
        let bool_ty = self
            .typed_hir
            .type_id(&crate::ast::TypeExpr::Reference(
                crate::ast::TypeReference {
                    span: statement.span,
                    name: "bool".to_string(),
                },
            ))
            .ok_or(BuildError::MissingTypedExpression)?;
        let header = self.control_flow.reserve_block();
        let body = self.control_flow.reserve_block();
        let exit = self.control_flow.reserve_block();
        self.control_flow
            .terminate(Terminator::Goto { target: header })?;
        self.control_flow.select_block(header)?;
        self.control_flow.terminate(Terminator::Switch {
            condition: Operand::Constant(crate::mir::model::Constant {
                ty: bool_ty,
                scalar: ScalarType::Bool,
                value: 1,
            }),
            then_target: body,
            else_target: exit,
        })?;
        self.loop_regions.push(crate::mir::LoopRegion {
            header,
            condition: header,
            body,
            continue_target: header,
            exit,
        });
        self.control_flow.select_block(body)?;
        let body_statements =
            scalar_loop_block_statements(&statement.body, self.resolved, self.typed_hir)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let exits_body = self.lower_in_context(
            &body_statements,
            Some(LoopTargets {
                break_target: exit,
                continue_target: header,
            }),
        )?;
        if !exits_body {
            self.control_flow
                .terminate(Terminator::Goto { target: header })?;
        }
        self.control_flow.select_block(exit)
    }
}
