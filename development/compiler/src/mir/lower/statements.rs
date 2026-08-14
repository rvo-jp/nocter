//! Scalar statement and natural-loop CFG construction.

use super::BuildError;
use super::context::LoweringContext;
use super::coverage::{
    ScalarStatement, binding_scalar_type, known_expression_type, scalar_body_parts,
    scalar_linear_block_statements, scalar_loop_block_statements,
};
use super::expressions::mir_assignment_operator;
use crate::ast::Expr;
use crate::mir::{
    BinaryOperator, ComparisonOperator, LocalId, LocalOrigin, LocalStorage, Operand, Origin, Place,
    Rvalue, ScalarType, ScopeId, Statement, Terminator,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct LoopTargets {
    pub(super) break_target: crate::mir::BasicBlockId,
    pub(super) continue_target: crate::mir::BasicBlockId,
}

pub(super) struct StatementLowerer<'context, 'semantic> {
    context: &'context mut LoweringContext<'semantic>,
}

impl<'context, 'semantic> StatementLowerer<'context, 'semantic> {
    pub(super) fn new(context: &'context mut LoweringContext<'semantic>) -> Self {
        Self { context }
    }

    pub(super) fn lower(
        &mut self,
        statements: &[ScalarStatement<'_>],
        scope: ScopeId,
    ) -> Result<bool, BuildError> {
        self.lower_in_context(statements, None, scope)
    }

    pub(super) fn lower_in_context(
        &mut self,
        statements: &[ScalarStatement<'_>],
        loop_targets: Option<LoopTargets>,
        scope: ScopeId,
    ) -> Result<bool, BuildError> {
        for statement in statements {
            let exits_block = match *statement {
                ScalarStatement::Binding(binding) => {
                    let symbol = self
                        .context
                        .semantic
                        .resolved
                        .local_symbol_id_at_name_span(binding.name_span)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    let binding_type_expr = self
                        .context
                        .semantic
                        .typed_hir
                        .binding_type_expr(symbol)
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let ty = self
                        .context
                        .semantic
                        .typed_hir
                        .type_id(binding_type_expr)
                        .or_else(|| {
                            known_expression_type(
                                &binding.initializer,
                                self.context.semantic.typed_hir,
                            )
                        })
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let local = LocalId::from_index(self.context.locals.len());
                    if let Some(scalar) =
                        binding_scalar_type(symbol, self.context.semantic.typed_hir)
                    {
                        self.context.locals.push(crate::mir::locals::Local::scalar(
                            ty,
                            scalar,
                            LocalStorage::Local,
                            LocalOrigin::Binding(symbol),
                            scope,
                        ));
                        self.context
                            .places_by_symbol
                            .insert(symbol, Place::local(local));
                        self.lower_value(local, &binding.initializer, ty, scalar, scope)?;
                    } else if let Some(borrow_ty) = self
                        .context
                        .semantic
                        .typed_hir
                        .binding_type_expr(symbol)
                        .and_then(|ty| match ty {
                            crate::ast::TypeExpr::Borrow(borrow) => Some(borrow),
                            _ => None,
                        })
                    {
                        self.context.locals.push(crate::mir::locals::Local::borrow(
                            ty,
                            borrow_ty.is_readwrite,
                            LocalStorage::Local,
                            LocalOrigin::Binding(symbol),
                            scope,
                        ));
                        self.context
                            .places_by_symbol
                            .insert(symbol, Place::local(local));
                        self.lower_borrow_binding(
                            local,
                            &binding.initializer,
                            borrow_ty.is_readwrite,
                            scope,
                        )?;
                    } else {
                        let ownership = if crate::typecheck::type_expr_is_copy(
                            binding_type_expr,
                            self.context.semantic.resolved,
                        ) == Some(true)
                        {
                            crate::mir::OwnershipKind::Copy
                        } else {
                            crate::mir::OwnershipKind::Move
                        };
                        let mut aggregate = crate::mir::locals::Local::aggregate(
                            ty,
                            ownership,
                            LocalStorage::Local,
                            LocalOrigin::Binding(symbol),
                            scope,
                        );
                        if ownership == crate::mir::OwnershipKind::Move {
                            aggregate.drop_plan = Some(
                                super::super::drop_plans::build(
                                    binding_type_expr,
                                    self.context.semantic.resolved,
                                    self.context.semantic.resolved_sources,
                                    self.context.semantic.typed_hir,
                                    &mut self.context.drop_plans,
                                )
                                .ok_or(BuildError::UnsupportedClaimedExpression)?,
                            );
                        }
                        self.context.locals.push(aggregate);
                        self.context
                            .places_by_symbol
                            .insert(symbol, Place::local(local));
                        match binding.initializer.without_groups() {
                            Expr::Call(_)
                                if super::aggregates::literal_is_supported(
                                    &binding.initializer,
                                    self.context.semantic,
                                ) =>
                            {
                                super::aggregates::lower_literal(
                                    self.context,
                                    local,
                                    &binding.initializer,
                                    scope,
                                )?;
                            }
                            Expr::Call(call) => {
                                let source = self
                                    .context
                                    .semantic
                                    .typed_hir
                                    .expression(call.span)
                                    .ok_or(BuildError::MissingTypedExpression)?
                                    .id;
                                let (callee, arguments, returns_never) =
                                    self.context.lower_call(call, scope)?;
                                if returns_never {
                                    return Err(BuildError::UnsupportedClaimedExpression);
                                }
                                self.context
                                    .control_flow
                                    .emit_returning_call(source, callee, arguments, local)?;
                            }
                            Expr::StructLiteral(_)
                            | Expr::ArrayLiteral(_)
                            | Expr::Member(_)
                            | Expr::Closure(_) => {
                                super::aggregates::lower_literal(
                                    self.context,
                                    local,
                                    &binding.initializer,
                                    scope,
                                )?;
                            }
                            Expr::TypedSequenceLiteral(_)
                            | Expr::TypedStringLiteral(_)
                            | Expr::InterpolatedString(_) => {
                                self.context.lower_value_to_place(
                                    local,
                                    &binding.initializer,
                                    ty,
                                    crate::mir::ValueRepresentation::Aggregate,
                                    scope,
                                )?;
                            }
                            Expr::Force(_)
                            | Expr::Propagate(_)
                            | Expr::Otherwise(_)
                            | Expr::Catch(_) => {
                                self.context.lower_value_to_place(
                                    local,
                                    &binding.initializer,
                                    ty,
                                    crate::mir::ValueRepresentation::Aggregate,
                                    scope,
                                )?;
                            }
                            _ => return Err(BuildError::UnsupportedClaimedExpression),
                        }
                    }
                    false
                }
                ScalarStatement::Assignment(assignment) => {
                    if assignment.operator == crate::ast::AssignmentOperator::Assign
                        && self.assignment_target_representation(&assignment.target, scope)?
                            == crate::mir::ValueRepresentation::Aggregate
                    {
                        self.lower_aggregate_assignment(assignment, scope)?;
                        continue;
                    }
                    let (destination, ty, scalar) =
                        self.lower_assignment_target(&assignment.target, scope)?;
                    if assignment.operator == crate::ast::AssignmentOperator::Assign {
                        if destination.projection.is_none() {
                            self.lower_value(
                                destination.local,
                                &assignment.value,
                                ty,
                                scalar,
                                scope,
                            )?;
                        } else {
                            let value =
                                self.context
                                    .lower_operand(&assignment.value, ty, scalar, scope)?;
                            self.context
                                .control_flow
                                .push_statement(Statement::Assign {
                                    destination,
                                    value: Rvalue::Use(value),
                                    origin: Origin::Desugared(assignment.operator_span),
                                })?;
                        }
                    } else {
                        let operator = mir_assignment_operator(assignment.operator)
                            .ok_or(BuildError::UnsupportedClaimedExpression)?;
                        let right =
                            self.context
                                .lower_operand(&assignment.value, ty, scalar, scope)?;
                        let current = if destination.projection.is_some() {
                            let current = self.scalar_temporary(
                                ty,
                                scalar,
                                LocalOrigin::Desugared(assignment.operator_span),
                                scope,
                            );
                            self.context
                                .control_flow
                                .push_statement(Statement::Assign {
                                    destination: Place::local(current),
                                    value: Rvalue::Use(Operand::Copy(destination)),
                                    origin: Origin::Desugared(assignment.operator_span),
                                })?;
                            Place::local(current)
                        } else {
                            destination
                        };
                        let result = if destination.projection.is_some() {
                            self.scalar_temporary(
                                ty,
                                scalar,
                                LocalOrigin::Desugared(assignment.operator_span),
                                scope,
                            )
                        } else {
                            destination.local
                        };
                        self.context
                            .control_flow
                            .push_statement(Statement::Assign {
                                destination: Place::local(result),
                                value: Rvalue::Binary {
                                    operator,
                                    left: Operand::Copy(current),
                                    right,
                                    ty,
                                },
                                origin: Origin::Desugared(assignment.operator_span),
                            })?;
                        if destination.projection.is_some() {
                            self.context
                                .control_flow
                                .push_statement(Statement::Assign {
                                    destination,
                                    value: Rvalue::Use(Operand::Copy(Place::local(result))),
                                    origin: Origin::Desugared(assignment.operator_span),
                                })?;
                        }
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
                ScalarStatement::CollectionFor(statement) => {
                    super::iteration::lower(self.context, statement, scope)?;
                    false
                }
                ScalarStatement::Loop(statement) => self.lower_loop(statement, scope)?,
                ScalarStatement::Region(statement) => {
                    let entered = super::regions::enter(self.context, statement, scope)?;
                    let body = scalar_linear_block_statements(
                        &statement.body,
                        self.context.semantic.resolved,
                        self.context.semantic.resolved_sources,
                        self.context.semantic.typed_hir,
                        loop_targets.is_some(),
                    )
                    .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    let exits = self.lower_in_context(&body, loop_targets, entered.scope)?;
                    if !exits {
                        self.context.control_flow.terminate(Terminator::Goto {
                            target: entered.exit,
                        })?;
                    }
                    self.context.control_flow.select_block(entered.exit)?;
                    exits
                }
                ScalarStatement::Expression(expression) => {
                    enum EffectKind {
                        Plain,
                        Trap,
                        Propagate,
                    }
                    let (call, kind) = match expression.without_groups() {
                        Expr::Call(call) => (call, EffectKind::Plain),
                        Expr::Force(force) => {
                            let Expr::Call(call) = force.expression.without_groups() else {
                                return Err(BuildError::UnsupportedClaimedExpression);
                            };
                            (call, EffectKind::Trap)
                        }
                        Expr::Propagate(propagate) => {
                            let Expr::Call(call) = propagate.expression.without_groups() else {
                                return Err(BuildError::UnsupportedClaimedExpression);
                            };
                            (call, EffectKind::Propagate)
                        }
                        _ => return Err(BuildError::UnsupportedClaimedExpression),
                    };
                    let source = self
                        .context
                        .semantic
                        .typed_hir
                        .expression(expression.span())
                        .ok_or(BuildError::MissingTypedExpression)?
                        .id;
                    let (callee, arguments, returns_never) =
                        self.context.lower_call(call, scope)?;
                    if returns_never {
                        self.context
                            .control_flow
                            .emit_never_call(source, callee, arguments)?;
                        return Ok(true);
                    }
                    match kind {
                        EffectKind::Plain => {
                            let ty = super::coverage::intrinsic_expression_type(
                                call.span,
                                self.context.semantic.typed_hir,
                            )
                            .ok_or(BuildError::MissingTypedExpression)?;
                            let representation =
                                super::coverage::value_representation(ty, self.context.semantic)
                                    .ok_or(BuildError::UnsupportedClaimedExpression)?;
                            if representation == crate::mir::ValueRepresentation::Unit {
                                self.context
                                    .control_flow
                                    .emit_effect_call(source, callee, arguments)?;
                            } else {
                                let destination = self.context.local_for_type(
                                    ty,
                                    LocalOrigin::Temporary(source),
                                    scope,
                                )?;
                                self.context.control_flow.emit_returning_call(
                                    source,
                                    callee,
                                    arguments,
                                    destination,
                                )?;
                                if let Some(plan) =
                                    self.context.locals[destination.index()].drop_plan
                                {
                                    self.context
                                        .control_flow
                                        .emit_drop(Place::local(destination), plan)?;
                                }
                            }
                        }
                        EffectKind::Trap => self
                            .context
                            .control_flow
                            .emit_trapping_outcome_effect(source, callee, arguments)?,
                        EffectKind::Propagate => self
                            .context
                            .control_flow
                            .emit_propagating_outcome_effect(source, callee, arguments)?,
                    }
                    false
                }
                ScalarStatement::Return(statement) => {
                    let Some(expression) = statement.expression.as_ref() else {
                        if self.context.locals[self.context.return_local().index()].representation
                            != crate::mir::ValueRepresentation::Unit
                        {
                            return Err(BuildError::UnsupportedClaimedExpression);
                        }
                        self.context.control_flow.terminate(Terminator::Return)?;
                        return Ok(true);
                    };
                    if self
                        .context
                        .semantic
                        .typed_hir
                        .expression(expression.span())
                        .is_some_and(|expression| expression.diverges)
                    {
                        let Expr::Call(call) = expression.without_groups() else {
                            return Err(BuildError::UnsupportedClaimedExpression);
                        };
                        let source = self
                            .context
                            .semantic
                            .typed_hir
                            .expression(expression.span())
                            .ok_or(BuildError::MissingTypedExpression)?
                            .id;
                        let (callee, arguments, returns_never) =
                            self.context.lower_call(call, scope)?;
                        if !returns_never {
                            return Err(BuildError::UnsupportedClaimedExpression);
                        }
                        self.context
                            .control_flow
                            .emit_never_call(source, callee, arguments)?;
                        return Ok(true);
                    }
                    if super::coverage::failure_value_is_supported(
                        expression,
                        self.context.semantic,
                    ) {
                        self.context.lower_failure_return(expression, scope)?;
                        return Ok(true);
                    }
                    let return_local = self.context.return_local();
                    let declaration = self.context.locals[return_local.index()].clone();
                    self.context.lower_value_to_place(
                        return_local,
                        expression,
                        declaration.ty,
                        declaration.representation,
                        scope,
                    )?;
                    self.context.control_flow.terminate(Terminator::Return)?;
                    return Ok(true);
                }
                ScalarStatement::Break => {
                    let targets = loop_targets.ok_or(BuildError::UnsupportedClaimedExpression)?;
                    self.context.control_flow.terminate(Terminator::Goto {
                        target: targets.break_target,
                    })?;
                    true
                }
                ScalarStatement::Continue => {
                    let targets = loop_targets.ok_or(BuildError::UnsupportedClaimedExpression)?;
                    self.context.control_flow.terminate(Terminator::Goto {
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
        self.context
            .lower_expression_to_place(destination, expression, ty, scalar, scope)
    }

    fn lower_assignment_target(
        &mut self,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<(Place, crate::semantic::TyId, ScalarType), BuildError> {
        match expression {
            Expr::Identifier(identifier) => {
                let symbol = self
                    .context
                    .semantic
                    .resolved
                    .local_symbol_for_identifier(identifier)
                    .map(|symbol| symbol.id)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let place = *self
                    .context
                    .places_by_symbol
                    .get(&symbol)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let declaration = if let Some(projection) = place.projection {
                    let path = &self.context.projections[projection.index()];
                    let crate::mir::ValueRepresentation::Scalar(scalar) = path.representation
                    else {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    };
                    return Ok((place, path.ty, scalar));
                } else {
                    &self.context.locals[place.local.index()]
                };
                let scalar = declaration
                    .scalar_type()
                    .ok_or(BuildError::UnsupportedClaimedExpression)?;
                Ok((place, declaration.ty, scalar))
            }
            Expr::Index(index) => {
                let (place, representation) =
                    super::indexes::lower_place(self.context, index, scope)?;
                let crate::mir::ValueRepresentation::Scalar(scalar) = representation else {
                    return Err(BuildError::UnsupportedClaimedExpression);
                };
                let ty = self.context.projections[place
                    .projection
                    .ok_or(BuildError::UnsupportedClaimedExpression)?
                    .index()]
                .ty;
                Ok((place, ty, scalar))
            }
            Expr::Member(member) => {
                let (place, representation) = super::projections::lower_borrow_field_place(
                    member,
                    self.context.semantic,
                    &self.context.places_by_symbol,
                    &mut self.context.projections,
                    &mut self.context.drop_plans,
                )?;
                let crate::mir::ValueRepresentation::Scalar(scalar) = representation else {
                    return Err(BuildError::UnsupportedClaimedExpression);
                };
                let ty = self.context.projections[place
                    .projection
                    .ok_or(BuildError::UnsupportedClaimedExpression)?
                    .index()]
                .ty;
                Ok((place, ty, scalar))
            }
            _ => Err(BuildError::UnsupportedClaimedExpression),
        }
    }

    fn assignment_target_representation(
        &mut self,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<crate::mir::ValueRepresentation, BuildError> {
        let (_, _, representation) = self.lower_value_assignment_target(expression, scope)?;
        Ok(representation)
    }

    fn lower_aggregate_assignment(
        &mut self,
        assignment: &crate::ast::AssignmentStmt,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        let (destination, ty, representation) =
            self.lower_value_assignment_target(&assignment.target, scope)?;
        if representation != crate::mir::ValueRepresentation::Aggregate {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        let drop_plan = destination
            .projection
            .and_then(|projection| self.context.projections[projection.index()].drop_plan)
            .or(self.context.locals[destination.local.index()].drop_plan);
        if super::coverage::aggregate_operand_is_supported(
            &assignment.value,
            self.context.semantic.resolved,
            self.context.semantic.resolved_sources,
            self.context.semantic.typed_hir,
        ) {
            let operand = self.context.lower_aggregate_operand(&assignment.value)?;
            if let Some(plan) = drop_plan {
                self.context.control_flow.emit_drop(destination, plan)?;
            }
            return self.context.control_flow.push_statement(Statement::Assign {
                destination,
                value: Rvalue::Use(operand),
                origin: Origin::Desugared(assignment.operator_span),
            });
        }
        let source = self
            .context
            .semantic
            .typed_hir
            .expression(assignment.value.span())
            .ok_or(BuildError::MissingTypedExpression)?
            .id;
        let temporary = self
            .context
            .local_for_type(ty, LocalOrigin::Temporary(source), scope)?;
        self.context.lower_value_to_place(
            temporary,
            &assignment.value,
            ty,
            representation,
            scope,
        )?;
        if let Some(plan) = drop_plan {
            self.context.control_flow.emit_drop(destination, plan)?;
        }
        let operand = match self.context.locals[temporary.index()].ownership {
            crate::mir::OwnershipKind::Move => Operand::Move(Place::local(temporary)),
            crate::mir::OwnershipKind::Copy => Operand::Copy(Place::local(temporary)),
            crate::mir::OwnershipKind::Borrowed { .. } => {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
        };
        self.context.control_flow.push_statement(Statement::Assign {
            destination,
            value: Rvalue::Use(operand),
            origin: Origin::Desugared(assignment.operator_span),
        })
    }

    fn lower_value_assignment_target(
        &mut self,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<
        (
            Place,
            crate::semantic::TyId,
            crate::mir::ValueRepresentation,
        ),
        BuildError,
    > {
        let (place, representation) = match expression.without_groups() {
            Expr::Identifier(identifier) => {
                let symbol = self
                    .context
                    .semantic
                    .resolved
                    .local_symbol_for_identifier(identifier)
                    .map(|symbol| symbol.id)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let place = *self
                    .context
                    .places_by_symbol
                    .get(&symbol)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let declaration = &self.context.locals[place.local.index()];
                (place, declaration.representation)
            }
            Expr::Member(member) => super::projections::lower_borrow_field_place(
                member,
                self.context.semantic,
                &self.context.places_by_symbol,
                &mut self.context.projections,
                &mut self.context.drop_plans,
            )?,
            Expr::Index(index) => super::indexes::lower_place(self.context, index, scope)?,
            _ => return Err(BuildError::UnsupportedClaimedExpression),
        };
        let ty = match place.projection {
            Some(projection) => self.context.projections[projection.index()].ty,
            None => self.context.locals[place.local.index()].ty,
        };
        Ok((place, ty, representation))
    }

    fn scalar_temporary(
        &mut self,
        ty: crate::semantic::TyId,
        scalar: ScalarType,
        origin: LocalOrigin,
        scope: ScopeId,
    ) -> LocalId {
        let local = LocalId::from_index(self.context.locals.len());
        self.context.locals.push(crate::mir::locals::Local::scalar(
            ty,
            scalar,
            LocalStorage::Local,
            origin,
            scope,
        ));
        local
    }

    fn lower_borrow_binding(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        readwrite: bool,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        super::borrows::lower_to_local(self.context, destination, expression, readwrite, scope)
    }

    fn lower_while(
        &mut self,
        statement: &crate::ast::WhileStmt,
        parent_scope: ScopeId,
    ) -> Result<(), BuildError> {
        let body_scope = self.context.child_scope(parent_scope, statement.body.span);
        let condition_target = self.context.control_flow.reserve_block(parent_scope);
        let body_target = self.context.control_flow.reserve_block(body_scope);
        let exit_target = self.context.control_flow.reserve_block(parent_scope);
        self.context.control_flow.terminate(Terminator::Goto {
            target: condition_target,
        })?;

        self.context.control_flow.select_block(condition_target)?;
        let condition_ty =
            known_expression_type(&statement.condition, self.context.semantic.typed_hir)
                .ok_or(BuildError::MissingTypedExpression)?;
        let condition = self.context.lower_operand(
            &statement.condition,
            condition_ty,
            ScalarType::Bool,
            parent_scope,
        )?;
        let condition_block = self.context.control_flow.current_block()?;
        self.context.control_flow.terminate(Terminator::Switch {
            condition,
            then_target: body_target,
            else_target: exit_target,
            join_target: None,
        })?;
        self.context.loop_regions.push(crate::mir::LoopRegion {
            header: condition_target,
            condition: condition_block,
            body: body_target,
            continue_target: condition_target,
            exit: exit_target,
        });

        self.context.control_flow.select_block(body_target)?;
        let body_statements = scalar_loop_block_statements(
            &statement.body,
            self.context.semantic.resolved,
            self.context.semantic.resolved_sources,
            self.context.semantic.typed_hir,
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
            self.context.control_flow.terminate(Terminator::Goto {
                target: condition_target,
            })?;
        }
        self.context.control_flow.select_block(exit_target)
    }

    fn lower_if(
        &mut self,
        statement: &crate::ast::IfStmt,
        loop_targets: Option<LoopTargets>,
        parent_scope: ScopeId,
    ) -> Result<bool, BuildError> {
        let condition_ty =
            known_expression_type(&statement.condition, self.context.semantic.typed_hir)
                .ok_or(BuildError::MissingTypedExpression)?;
        let condition = self.context.lower_operand(
            &statement.condition,
            condition_ty,
            ScalarType::Bool,
            parent_scope,
        )?;
        let then_scope = self
            .context
            .child_scope(parent_scope, statement.then_block.span);
        let else_span = statement
            .else_block
            .as_ref()
            .map_or(statement.span, |block| block.span);
        let else_scope = self.context.child_scope(parent_scope, else_span);
        let then_target = self.context.control_flow.reserve_block(then_scope);
        let else_target = self.context.control_flow.reserve_block(else_scope);
        let join_target = self.context.control_flow.reserve_block(parent_scope);
        let switch_block = self.context.control_flow.current_block()?;
        self.context.control_flow.terminate(Terminator::Switch {
            condition,
            then_target,
            else_target,
            join_target: Some(join_target),
        })?;

        self.context.control_flow.select_block(then_target)?;
        let then_statements = scalar_linear_block_statements(
            &statement.then_block,
            self.context.semantic.resolved,
            self.context.semantic.resolved_sources,
            self.context.semantic.typed_hir,
            loop_targets.is_some(),
        )
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let then_exits = self.lower_in_context(&then_statements, loop_targets, then_scope)?;
        if !then_exits {
            self.context.control_flow.terminate(Terminator::Goto {
                target: join_target,
            })?;
        }

        self.context.control_flow.select_block(else_target)?;
        let else_statements = statement
            .else_block
            .as_ref()
            .map(|block| {
                scalar_linear_block_statements(
                    block,
                    self.context.semantic.resolved,
                    self.context.semantic.resolved_sources,
                    self.context.semantic.typed_hir,
                    loop_targets.is_some(),
                )
                .ok_or(BuildError::UnsupportedClaimedExpression)
            })
            .transpose()?
            .unwrap_or_default();
        let else_exits = self.lower_in_context(&else_statements, loop_targets, else_scope)?;
        if !else_exits {
            self.context.control_flow.terminate(Terminator::Goto {
                target: join_target,
            })?;
        }

        if then_exits && else_exits {
            self.context
                .control_flow
                .set_switch_join(switch_block, None)?;
            self.context
                .control_flow
                .discard_last_reserved_block(join_target)?;
            return Ok(true);
        }
        self.context.control_flow.select_block(join_target)?;
        Ok(false)
    }

    fn lower_for_range(
        &mut self,
        statement: &crate::ast::ForRangeStmt,
        parent_scope: ScopeId,
    ) -> Result<(), BuildError> {
        let loop_scope = self.context.child_scope(parent_scope, statement.span);
        let body_scope = self.context.child_scope(loop_scope, statement.body.span);
        let preheader = self.context.control_flow.reserve_block(loop_scope);
        self.context
            .control_flow
            .terminate(Terminator::Goto { target: preheader })?;
        self.context.control_flow.select_block(preheader)?;
        let symbol = self
            .context
            .semantic
            .resolved
            .local_symbol_id_at_name_span(statement.name_span)
            .ok_or(BuildError::MissingLocalSymbol)?;
        let ty = self
            .context
            .semantic
            .typed_hir
            .binding_type_expr(symbol)
            .and_then(|ty| self.context.semantic.typed_hir.type_id(ty))
            .ok_or(BuildError::MissingTypedExpression)?;
        let scalar = binding_scalar_type(symbol, self.context.semantic.typed_hir)
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let value = LocalId::from_index(self.context.locals.len());
        self.context.locals.push(crate::mir::locals::Local::scalar(
            ty,
            scalar,
            LocalStorage::Local,
            LocalOrigin::Binding(symbol),
            loop_scope,
        ));
        self.context
            .places_by_symbol
            .insert(symbol, Place::local(value));
        self.lower_value(value, &statement.start, ty, scalar, loop_scope)?;

        let end = LocalId::from_index(self.context.locals.len());
        self.context.locals.push(crate::mir::locals::Local::scalar(
            ty,
            scalar,
            LocalStorage::Local,
            LocalOrigin::Desugared(statement.range_span),
            loop_scope,
        ));
        self.lower_value(end, &statement.end, ty, scalar, loop_scope)?;

        let bool_ty = self
            .context
            .semantic
            .typed_hir
            .type_id(&crate::ast::TypeExpr::Reference(
                crate::ast::TypeReference {
                    span: statement.range_span,
                    name: "bool".to_string(),
                },
            ))
            .ok_or(BuildError::MissingTypedExpression)?;
        let condition_local = LocalId::from_index(self.context.locals.len());
        self.context.locals.push(crate::mir::locals::Local::scalar(
            bool_ty,
            ScalarType::Bool,
            LocalStorage::Local,
            LocalOrigin::Desugared(statement.range_span),
            loop_scope,
        ));

        let header = self.context.control_flow.reserve_block(loop_scope);
        let body = self.context.control_flow.reserve_block(body_scope);
        let increment = self.context.control_flow.reserve_block(loop_scope);
        let exit = self.context.control_flow.reserve_block(parent_scope);
        self.context
            .control_flow
            .terminate(Terminator::Goto { target: header })?;
        self.context.control_flow.select_block(header)?;
        self.context
            .control_flow
            .push_statement(Statement::Assign {
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
        self.context.control_flow.terminate(Terminator::Switch {
            condition: Operand::Copy(Place::local(condition_local)),
            then_target: body,
            else_target: exit,
            join_target: None,
        })?;
        self.context.loop_regions.push(crate::mir::LoopRegion {
            header,
            condition: header,
            body,
            continue_target: increment,
            exit,
        });

        self.context.control_flow.select_block(body)?;
        let body_statements = scalar_loop_block_statements(
            &statement.body,
            self.context.semantic.resolved,
            self.context.semantic.resolved_sources,
            self.context.semantic.typed_hir,
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
            self.context
                .control_flow
                .terminate(Terminator::Goto { target: increment })?;
        }

        self.context.control_flow.select_block(increment)?;
        self.context
            .control_flow
            .push_statement(Statement::Assign {
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
        self.context
            .control_flow
            .terminate(Terminator::Goto { target: header })?;
        self.context.control_flow.select_block(exit)
    }

    fn lower_loop(
        &mut self,
        statement: &crate::ast::LoopStmt,
        parent_scope: ScopeId,
    ) -> Result<bool, BuildError> {
        let loop_scope = self.context.child_scope(parent_scope, statement.span);
        let body_scope = self.context.child_scope(loop_scope, statement.body.span);
        let bool_ty = self
            .context
            .semantic
            .typed_hir
            .type_id(&crate::ast::TypeExpr::Reference(
                crate::ast::TypeReference {
                    span: statement.span,
                    name: "bool".to_string(),
                },
            ))
            .ok_or(BuildError::MissingTypedExpression)?;
        let header = self.context.control_flow.reserve_block(loop_scope);
        let body = self.context.control_flow.reserve_block(body_scope);
        let exit = self.context.control_flow.reserve_block(parent_scope);
        self.context
            .control_flow
            .terminate(Terminator::Goto { target: header })?;
        self.context.control_flow.select_block(header)?;
        self.context.control_flow.terminate(Terminator::Switch {
            condition: Operand::Constant(crate::mir::model::Constant {
                ty: bool_ty,
                scalar: ScalarType::Bool,
                value: 1,
            }),
            then_target: body,
            else_target: exit,
            join_target: None,
        })?;
        self.context.loop_regions.push(crate::mir::LoopRegion {
            header,
            condition: header,
            body,
            continue_target: header,
            exit,
        });
        self.context.control_flow.select_block(body)?;
        let body_statements = scalar_loop_block_statements(
            &statement.body,
            self.context.semantic.resolved,
            self.context.semantic.resolved_sources,
            self.context.semantic.typed_hir,
        )
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let body_returns = matches!(body_statements.last(), Some(ScalarStatement::Return(_)));
        let exits_body = self.lower_in_context(
            &body_statements,
            Some(LoopTargets {
                break_target: exit,
                continue_target: header,
            }),
            body_scope,
        )?;
        if !exits_body {
            self.context
                .control_flow
                .terminate(Terminator::Goto { target: header })?;
        }
        self.context.control_flow.select_block(exit)?;
        if body_returns {
            self.context.control_flow.terminate(Terminator::Trap)?;
        }
        Ok(body_returns)
    }
}

pub(super) fn lower_value_block(
    context: &mut LoweringContext<'_>,
    block: &crate::ast::Block,
    destination: LocalId,
    ty: crate::semantic::TyId,
    representation: crate::mir::ValueRepresentation,
    scope: ScopeId,
    preserve_explicit_return: bool,
) -> Result<bool, BuildError> {
    let (statements, tail) =
        scalar_body_parts(block).ok_or(BuildError::UnsupportedClaimedExpression)?;
    if StatementLowerer::new(context).lower(&statements, scope)? {
        return Ok(true);
    }
    if let Some(expression) = tail.expression()
        && context
            .semantic
            .typed_hir
            .expression(expression.span())
            .is_some_and(|expression| expression.diverges)
    {
        let Expr::Call(call) = expression.without_groups() else {
            return Err(BuildError::UnsupportedClaimedExpression);
        };
        let source = context
            .semantic
            .typed_hir
            .expression(expression.span())
            .ok_or(BuildError::MissingTypedExpression)?
            .id;
        let (callee, arguments, returns_never) = context.lower_call(call, scope)?;
        if !returns_never {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        context
            .control_flow
            .emit_never_call(source, callee, arguments)?;
        return Ok(true);
    }
    let returns = preserve_explicit_return && tail.is_explicit_return();
    if returns
        && tail.expression().is_some_and(|expression| {
            super::coverage::failure_value_is_supported(expression, context.semantic)
        })
    {
        context.lower_failure_return(
            tail.expression()
                .ok_or(BuildError::UnsupportedClaimedExpression)?,
            scope,
        )?;
        return Ok(true);
    }
    let (destination, ty, representation) = if returns {
        let destination = context.return_local();
        let declaration = context
            .locals
            .get(destination.index())
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        (destination, declaration.ty, declaration.representation)
    } else {
        (destination, ty, representation)
    };
    if representation == crate::mir::ValueRepresentation::Unit && tail.expression().is_none() {
        if returns {
            context.control_flow.terminate(Terminator::Return)?;
        }
        return Ok(returns);
    }
    if let Some(conditional) = tail.conditional() {
        super::expressions::lower_conditional_to_place(
            context,
            destination,
            conditional,
            ty,
            representation,
            scope,
        )?;
    } else {
        context.lower_value_to_place(
            destination,
            tail.expression()
                .ok_or(BuildError::UnsupportedClaimedExpression)?,
            ty,
            representation,
            scope,
        )?;
    }
    if returns {
        context.control_flow.terminate(Terminator::Return)?;
    }
    Ok(returns)
}
