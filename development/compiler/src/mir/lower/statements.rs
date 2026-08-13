//! Scalar statement and natural-loop CFG construction.

use super::body_builder::ControlFlowBuilder;
use super::coverage::{
    ScalarStatement, binding_scalar_type, known_expression_type, scalar_linear_block_statements,
    scalar_loop_block_statements,
};
use super::expressions::{lower_expression_to_place, lower_operand, mir_assignment_operator};
use super::{BuildError, SemanticInputs};
use crate::ast::Expr;
use crate::mir::{
    BinaryOperator, ComparisonOperator, LocalId, LocalOrigin, LocalStorage, Operand, Origin, Place,
    Rvalue, ScalarType, Scope, ScopeId, Statement, Terminator,
};
use crate::resolve::LocalSymbolId;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
struct LoopTargets {
    break_target: crate::mir::BasicBlockId,
    continue_target: crate::mir::BasicBlockId,
}

pub(super) struct StatementLowerer<'a> {
    semantic: SemanticInputs<'a>,
    locals: &'a mut Vec<crate::mir::Local>,
    locals_by_symbol: &'a mut HashMap<LocalSymbolId, LocalId>,
    projections: &'a mut Vec<crate::mir::ProjectionPath>,
    control_flow: &'a mut ControlFlowBuilder,
    loop_regions: &'a mut Vec<crate::mir::LoopRegion>,
    scopes: &'a mut Vec<Scope>,
}

impl<'a> StatementLowerer<'a> {
    pub(super) fn new(
        semantic: SemanticInputs<'a>,
        locals: &'a mut Vec<crate::mir::Local>,
        locals_by_symbol: &'a mut HashMap<LocalSymbolId, LocalId>,
        projections: &'a mut Vec<crate::mir::ProjectionPath>,
        control_flow: &'a mut ControlFlowBuilder,
        loop_regions: &'a mut Vec<crate::mir::LoopRegion>,
        scopes: &'a mut Vec<Scope>,
    ) -> Self {
        Self {
            semantic,
            locals,
            locals_by_symbol,
            projections,
            control_flow,
            loop_regions,
            scopes,
        }
    }

    pub(super) fn lower(
        &mut self,
        statements: &[ScalarStatement<'_>],
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        if self.lower_in_context(statements, None, scope)? {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        Ok(())
    }

    fn lower_in_context(
        &mut self,
        statements: &[ScalarStatement<'_>],
        loop_targets: Option<LoopTargets>,
        scope: ScopeId,
    ) -> Result<bool, BuildError> {
        for statement in statements {
            let exits_block = match *statement {
                ScalarStatement::Binding(binding) => {
                    let symbol = self
                        .semantic
                        .resolved
                        .local_symbol_id_at_name_span(binding.name_span)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    let ty = self
                        .semantic
                        .typed_hir
                        .binding_type_expr(symbol)
                        .and_then(|ty| self.semantic.typed_hir.type_id(ty))
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let scalar = binding_scalar_type(symbol, self.semantic.typed_hir)
                        .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    let local = LocalId::from_index(self.locals.len());
                    self.locals.push(crate::mir::locals::Local::scalar(
                        ty,
                        scalar,
                        LocalStorage::Local,
                        LocalOrigin::Binding(symbol),
                        scope,
                    ));
                    self.locals_by_symbol.insert(symbol, local);
                    self.lower_value(local, &binding.initializer, ty, scalar, scope)?;
                    false
                }
                ScalarStatement::Assignment(assignment) => {
                    let Expr::Identifier(identifier) = &assignment.target else {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    };
                    let symbol = self
                        .semantic
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
                    let scalar = declaration
                        .scalar_type()
                        .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    if assignment.operator == crate::ast::AssignmentOperator::Assign {
                        self.lower_value(local, &assignment.value, ty, scalar, scope)?;
                    } else {
                        let operator = mir_assignment_operator(assignment.operator)
                            .ok_or(BuildError::UnsupportedClaimedExpression)?;
                        let right = lower_operand(
                            &assignment.value,
                            ty,
                            scalar,
                            self.semantic,
                            self.locals_by_symbol,
                            self.locals,
                            self.projections,
                            self.control_flow,
                            self.scopes,
                            scope,
                        )?;
                        self.control_flow.push_statement(Statement::Assign {
                            destination: Place::local(local),
                            value: Rvalue::Binary {
                                operator,
                                left: Operand::Copy(Place::local(local)),
                                right,
                                ty,
                            },
                            origin: Origin::Desugared(assignment.operator_span),
                        })?;
                    }
                    false
                }
                ScalarStatement::While(statement) => {
                    self.lower_while(statement, scope)?;
                    false
                }
                ScalarStatement::If(statement) => self.lower_if(statement, loop_targets, scope)?,
                ScalarStatement::ForRange(statement) => {
                    self.lower_for_range(statement, scope)?;
                    false
                }
                ScalarStatement::Loop(statement) => {
                    self.lower_loop(statement, scope)?;
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
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        lower_expression_to_place(
            destination,
            expression,
            ty,
            scalar,
            self.semantic,
            self.locals_by_symbol,
            self.locals,
            self.projections,
            self.control_flow,
            self.scopes,
            scope,
        )
    }

    fn lower_while(
        &mut self,
        statement: &crate::ast::WhileStmt,
        parent_scope: ScopeId,
    ) -> Result<(), BuildError> {
        let body_scope = self.child_scope(parent_scope, statement.body.span);
        let condition_target = self.control_flow.reserve_block(parent_scope);
        let body_target = self.control_flow.reserve_block(body_scope);
        let exit_target = self.control_flow.reserve_block(parent_scope);
        self.control_flow.terminate(Terminator::Goto {
            target: condition_target,
        })?;

        self.control_flow.select_block(condition_target)?;
        let condition_ty = known_expression_type(&statement.condition, self.semantic.typed_hir)
            .ok_or(BuildError::MissingTypedExpression)?;
        let condition = lower_operand(
            &statement.condition,
            condition_ty,
            ScalarType::Bool,
            self.semantic,
            self.locals_by_symbol,
            self.locals,
            self.projections,
            self.control_flow,
            self.scopes,
            parent_scope,
        )?;
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
        let body_statements = scalar_loop_block_statements(
            &statement.body,
            self.semantic.resolved,
            self.semantic.resolved_sources,
            self.semantic.typed_hir,
        )
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let exits_body = self.lower_in_context(
            &body_statements,
            Some(LoopTargets {
                break_target: exit_target,
                continue_target: condition_target,
            }),
            body_scope,
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
        parent_scope: ScopeId,
    ) -> Result<bool, BuildError> {
        let condition_ty = known_expression_type(&statement.condition, self.semantic.typed_hir)
            .ok_or(BuildError::MissingTypedExpression)?;
        let condition = lower_operand(
            &statement.condition,
            condition_ty,
            ScalarType::Bool,
            self.semantic,
            self.locals_by_symbol,
            self.locals,
            self.projections,
            self.control_flow,
            self.scopes,
            parent_scope,
        )?;
        let then_scope = self.child_scope(parent_scope, statement.then_block.span);
        let else_span = statement
            .else_block
            .as_ref()
            .map_or(statement.span, |block| block.span);
        let else_scope = self.child_scope(parent_scope, else_span);
        let then_target = self.control_flow.reserve_block(then_scope);
        let else_target = self.control_flow.reserve_block(else_scope);
        let join_target = self.control_flow.reserve_block(parent_scope);
        self.control_flow.terminate(Terminator::Switch {
            condition,
            then_target,
            else_target,
        })?;

        self.control_flow.select_block(then_target)?;
        let then_statements = scalar_linear_block_statements(
            &statement.then_block,
            self.semantic.resolved,
            self.semantic.resolved_sources,
            self.semantic.typed_hir,
            loop_targets.is_some(),
        )
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let then_exits = self.lower_in_context(&then_statements, loop_targets, then_scope)?;
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
                    self.semantic.resolved,
                    self.semantic.resolved_sources,
                    self.semantic.typed_hir,
                    loop_targets.is_some(),
                )
                .ok_or(BuildError::UnsupportedClaimedExpression)
            })
            .transpose()?
            .unwrap_or_default();
        let else_exits = self.lower_in_context(&else_statements, loop_targets, else_scope)?;
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

    fn lower_for_range(
        &mut self,
        statement: &crate::ast::ForRangeStmt,
        parent_scope: ScopeId,
    ) -> Result<(), BuildError> {
        let loop_scope = self.child_scope(parent_scope, statement.span);
        let body_scope = self.child_scope(loop_scope, statement.body.span);
        let preheader = self.control_flow.reserve_block(loop_scope);
        self.control_flow
            .terminate(Terminator::Goto { target: preheader })?;
        self.control_flow.select_block(preheader)?;
        let symbol = self
            .semantic
            .resolved
            .local_symbol_id_at_name_span(statement.name_span)
            .ok_or(BuildError::MissingLocalSymbol)?;
        let ty = self
            .semantic
            .typed_hir
            .binding_type_expr(symbol)
            .and_then(|ty| self.semantic.typed_hir.type_id(ty))
            .ok_or(BuildError::MissingTypedExpression)?;
        let scalar = binding_scalar_type(symbol, self.semantic.typed_hir)
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let value = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::locals::Local::scalar(
            ty,
            scalar,
            LocalStorage::Local,
            LocalOrigin::Binding(symbol),
            loop_scope,
        ));
        self.locals_by_symbol.insert(symbol, value);
        self.lower_value(value, &statement.start, ty, scalar, loop_scope)?;

        let end = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::locals::Local::scalar(
            ty,
            scalar,
            LocalStorage::Local,
            LocalOrigin::Desugared(statement.range_span),
            loop_scope,
        ));
        self.lower_value(end, &statement.end, ty, scalar, loop_scope)?;

        let bool_ty = self
            .semantic
            .typed_hir
            .type_id(&crate::ast::TypeExpr::Reference(
                crate::ast::TypeReference {
                    span: statement.range_span,
                    name: "bool".to_string(),
                },
            ))
            .ok_or(BuildError::MissingTypedExpression)?;
        let condition_local = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::locals::Local::scalar(
            bool_ty,
            ScalarType::Bool,
            LocalStorage::Local,
            LocalOrigin::Desugared(statement.range_span),
            loop_scope,
        ));

        let header = self.control_flow.reserve_block(loop_scope);
        let body = self.control_flow.reserve_block(body_scope);
        let increment = self.control_flow.reserve_block(loop_scope);
        let exit = self.control_flow.reserve_block(parent_scope);
        self.control_flow
            .terminate(Terminator::Goto { target: header })?;
        self.control_flow.select_block(header)?;
        self.control_flow.push_statement(Statement::Assign {
            destination: Place::local(condition_local),
            value: Rvalue::Compare {
                operator: ComparisonOperator::Less,
                left: Operand::Copy(Place::local(value)),
                right: Operand::Copy(Place::local(end)),
                operand_ty: ty,
                operand_scalar: scalar,
                result_ty: bool_ty,
            },
            origin: Origin::Desugared(statement.range_span),
        })?;
        self.control_flow.terminate(Terminator::Switch {
            condition: Operand::Copy(Place::local(condition_local)),
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
        let body_statements = scalar_loop_block_statements(
            &statement.body,
            self.semantic.resolved,
            self.semantic.resolved_sources,
            self.semantic.typed_hir,
        )
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let exits_body = self.lower_in_context(
            &body_statements,
            Some(LoopTargets {
                break_target: exit,
                continue_target: increment,
            }),
            body_scope,
        )?;
        if !exits_body {
            self.control_flow
                .terminate(Terminator::Goto { target: increment })?;
        }

        self.control_flow.select_block(increment)?;
        self.control_flow.push_statement(Statement::Assign {
            destination: Place::local(value),
            value: Rvalue::Binary {
                operator: BinaryOperator::Add,
                left: Operand::Copy(Place::local(value)),
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

    fn lower_loop(
        &mut self,
        statement: &crate::ast::LoopStmt,
        parent_scope: ScopeId,
    ) -> Result<(), BuildError> {
        let loop_scope = self.child_scope(parent_scope, statement.span);
        let body_scope = self.child_scope(loop_scope, statement.body.span);
        let bool_ty = self
            .semantic
            .typed_hir
            .type_id(&crate::ast::TypeExpr::Reference(
                crate::ast::TypeReference {
                    span: statement.span,
                    name: "bool".to_string(),
                },
            ))
            .ok_or(BuildError::MissingTypedExpression)?;
        let header = self.control_flow.reserve_block(loop_scope);
        let body = self.control_flow.reserve_block(body_scope);
        let exit = self.control_flow.reserve_block(parent_scope);
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
        let body_statements = scalar_loop_block_statements(
            &statement.body,
            self.semantic.resolved,
            self.semantic.resolved_sources,
            self.semantic.typed_hir,
        )
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let exits_body = self.lower_in_context(
            &body_statements,
            Some(LoopTargets {
                break_target: exit,
                continue_target: header,
            }),
            body_scope,
        )?;
        if !exits_body {
            self.control_flow
                .terminate(Terminator::Goto { target: header })?;
        }
        self.control_flow.select_block(exit)
    }

    fn child_scope(&mut self, parent: ScopeId, span: crate::source::ByteSpan) -> ScopeId {
        let scope = ScopeId::from_index(self.scopes.len());
        self.scopes.push(Scope::child(parent, span));
        scope
    }
}
