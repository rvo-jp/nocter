//! Scalar expression evaluation into MIR places, rvalues, and operands.

use super::BuildError;
use super::context::LoweringContext;
use super::source_model::{known_expression_type, scalar_type, value_representation};
use crate::ast::Expr;
use crate::literals::decode_integer_literal_value;
use crate::mir::{
    BinaryOperator, CallArgument, ComparisonOperator, LocalId, LocalOrigin, LocalStorage, Operand,
    Place, Rvalue, ScalarType, ScopeId, Statement, UnaryOperator,
};

mod control_flow_expressions;
mod outcomes;

enum PlannedReceiver<'a> {
    Method(&'a crate::ast::MemberExpr),
    Callable {
        expression: &'a Expr,
        receiver_ty: crate::semantic::TyId,
        capability: crate::ast::CallableCapability,
    },
}
pub(super) use control_flow_expressions::{
    block_exits_function, lower_conditional_to_place, lower_if_is_statement, lower_if_is_to_place,
    lower_match_statement, lower_match_to_place,
};
pub(super) use outcomes::lower_unit_catch;

impl LoweringContext<'_> {
    pub(super) fn lower_coercion_to_local(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<bool, BuildError> {
        let Some(conversion) = self.semantic.conversion_plan(expression) else {
            return Ok(false);
        };
        self.lower_planned_coercion_to_local(destination, expression, &conversion, scope)?;
        Ok(true)
    }

    pub(super) fn lower_planned_coercion_to_local(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        conversion: &crate::typecheck::TypecheckConversionPlan,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        let crate::typecheck::TypecheckConversionKind::BorrowCoercion(plan) = &conversion.kind
        else {
            return Err(BuildError::UnsupportedClaimedExpression);
        };
        let definition = plan.def_id.ok_or(BuildError::MissingCallTarget)?;
        let definition = self
            .semantic
            .resolved
            .callable_bodies
            .canonical_definition(definition);
        let receiver_ty = self
            .semantic
            .typed_hir
            .type_id(&plan.self_ty)
            .ok_or(BuildError::MissingSpecializedReceiverType)?;
        let source = coercion_source_expression(expression);
        let receiver_is_readwrite =
            plan.receiver_mode == crate::ast::MethodReceiverMode::ReadwriteBorrow;
        let argument = if matches!(conversion.source_ty, crate::ast::TypeExpr::Borrow(_)) {
            let source_ty = self
                .semantic
                .typed_hir
                .type_id(&conversion.source_ty)
                .ok_or(BuildError::MissingTypedExpression)?;
            let operand = if matches!(source.without_groups(), Expr::Identifier(_)) {
                self.lower_stored_identifier(source)?
            } else {
                let source_origin = self
                    .semantic
                    .typed_hir
                    .expression(source.span())
                    .map_or(LocalOrigin::Desugared(source.span()), |expression| {
                        LocalOrigin::Temporary(expression.id)
                    });
                let temporary = LocalId::from_index(self.locals.len());
                self.locals.push(crate::mir::Local::borrow(
                    source_ty,
                    plan.source_is_readwrite,
                    LocalStorage::Local,
                    source_origin,
                    scope,
                ));
                if matches!(source.without_groups(), Expr::Borrow(_)) {
                    super::borrows::lower_to_local_without_coercion(
                        self,
                        temporary,
                        source,
                        plan.source_is_readwrite,
                        scope,
                        crate::mir::LoanLifetime::Call,
                    )?;
                } else {
                    self.lower_borrow_value_to_place_without_coercion(
                        temporary, source, source_ty, scope,
                    )?;
                }
                Operand::Copy(Place::local(temporary))
            };
            CallArgument {
                operand,
                ty: source_ty,
                representation: crate::mir::ValueRepresentation::Borrow,
            }
        } else if super::borrows::source_place_is_supported(source, self.semantic) {
            let source = super::borrows::lower_source_place(self, source, scope)?;
            super::borrows::place_argument(
                self,
                source,
                &plan.self_ty,
                receiver_is_readwrite,
                scope,
                crate::mir::Origin::Desugared(expression.span()),
            )?
        } else {
            super::borrows::expression_argument(
                self,
                source,
                &plan.self_ty,
                receiver_is_readwrite,
                scope,
                crate::mir::Origin::Desugared(expression.span()),
            )?
        };
        let origin = self
            .semantic
            .typed_hir
            .expression(expression.span())
            .ok_or(BuildError::MissingTypedExpression)?
            .id;
        self.control_flow.emit_returning_call(
            origin,
            crate::mir::CallInstance::specialized(definition, Some(receiver_ty), Vec::new()),
            vec![argument],
            destination,
        )?;
        Ok(())
    }

    pub(super) fn lower_intrinsic_effect(
        &mut self,
        call: &crate::ast::CallExpr,
        origin: crate::mir::Origin,
        scope: ScopeId,
    ) -> Result<bool, BuildError> {
        let Some(intrinsic) = super::source_model::intrinsic_for_call(call, self.semantic) else {
            return Ok(false);
        };
        if !super::source_model::effect_intrinsic_is_supported(intrinsic) {
            return Ok(false);
        }
        let first_new_loan = self.loans.len();
        let arguments = call
            .arguments
            .iter()
            .map(|argument| self.lower_call_argument(argument, scope))
            .collect::<Result<Vec<_>, _>>()?;
        let type_arguments = if matches!(
            intrinsic,
            crate::intrinsics::IntrinsicId::StoreValueToPtr
                | crate::intrinsics::IntrinsicId::DropValueAtPtr
        ) {
            self.semantic
                .typed_hir
                .function_call_specialization(call.span)
                .and_then(|specialization| specialization.ordered_type_arguments())
                .map(|arguments| self.call_type_arguments(Some(arguments)))
                .transpose()?
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let call_loans = self.loans[first_new_loan..]
            .iter()
            .filter(|loan| {
                loan.lifetime == crate::mir::LoanLifetime::Call
                    && arguments.iter().any(|argument| {
                        matches!(
                            argument.operand,
                            Operand::Copy(place) | Operand::Move(place)
                                if place == Place::local(loan.destination)
                        )
                    })
            })
            .map(|loan| loan.id)
            .collect::<Vec<_>>();
        if intrinsic == crate::intrinsics::IntrinsicId::DropValueAtPtr {
            let ty = *type_arguments
                .first()
                .ok_or(BuildError::MissingTypedExpression)?;
            let ty = self
                .semantic
                .typed_hir
                .type_expr_by_id(ty)
                .ok_or(BuildError::MissingTypedExpression)?;
            if super::super::drop_plans::is_copy(
                ty,
                self.semantic.resolved,
                self.semantic.resolved_sources,
            ) != Some(true)
            {
                let plan = super::super::drop_plans::build(
                    ty,
                    self.semantic.resolved,
                    self.semantic.resolved_sources,
                    self.semantic.typed_hir,
                    &mut self.drop_plans,
                )
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
                let [pointer, offset] = arguments.as_slice() else {
                    return Err(BuildError::UnsupportedClaimedExpression);
                };
                self.control_flow.push_statement(Statement::DropAtPointer {
                    pointer: pointer.operand.clone(),
                    offset: offset.operand.clone(),
                    ty: type_arguments[0],
                    plan,
                    origin,
                })?;
            }
        } else {
            self.control_flow.push_statement(Statement::Intrinsic {
                intrinsic,
                arguments,
                type_arguments,
                origin,
            })?;
        }
        for loan in call_loans {
            self.control_flow
                .push_statement(Statement::EndLoan { loan })?;
        }
        Ok(true)
    }

    fn lower_intrinsic_rvalue(
        &mut self,
        call: &crate::ast::CallExpr,
        result_ty: crate::semantic::TyId,
        representation: crate::mir::ValueRepresentation,
        scope: ScopeId,
    ) -> Result<Option<Rvalue>, BuildError> {
        let Some(intrinsic) = super::source_model::intrinsic_for_call(call, self.semantic) else {
            return Ok(None);
        };
        if !super::source_model::value_intrinsic_is_supported(intrinsic) {
            return Ok(None);
        }
        let arguments = call
            .arguments
            .iter()
            .map(|argument| self.lower_call_argument(argument, scope))
            .collect::<Result<Vec<_>, _>>()?;
        let type_arguments = if matches!(
            intrinsic,
            crate::intrinsics::IntrinsicId::PointeeAlign
                | crate::intrinsics::IntrinsicId::PointeeSize
        ) {
            self.semantic
                .typed_hir
                .function_call_specialization(call.span)
                .and_then(|specialization| specialization.ordered_type_arguments())
                .map(|arguments| self.call_type_arguments(Some(arguments)))
                .transpose()?
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Ok(Some(Rvalue::Intrinsic {
            intrinsic,
            arguments,
            type_arguments,
            result_ty,
            representation,
        }))
    }

    pub(super) fn lower_direct_outcome_return(
        &mut self,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        let contract = self
            .outcome_contract
            .clone()
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        if matches!(expression.without_groups(), Expr::NoneLiteral(_)) {
            if !contract
                .layers
                .contains(&crate::outcomes::OutcomeLayer::Optional)
            {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            return self
                .control_flow
                .terminate(crate::mir::Terminator::ReturnOptionalNone);
        }
        let source = match contract.payload_representation {
            crate::mir::ValueRepresentation::Scalar(scalar) => {
                self.lower_operand(expression, contract.payload_ty, scalar, scope)?
            }
            crate::mir::ValueRepresentation::View(kind) => self
                .lower_view_operand(expression, contract.payload_ty, kind, scope)
                .map_err(|error| error.context("lower view outcome return payload"))?,
            crate::mir::ValueRepresentation::Aggregate => {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(expression.span())
                    .ok_or(BuildError::MissingTypedExpression)?
                    .id;
                let local = self
                    .aggregate_temporary(contract.payload_ty, LocalOrigin::Temporary(origin), scope)
                    .map_err(|error| {
                        error.context("allocate aggregate outcome return temporary")
                    })?;
                self.lower_value_to_place(
                    local,
                    expression,
                    contract.payload_ty,
                    contract.payload_representation,
                    scope,
                )
                .map_err(|error| error.context("lower aggregate outcome return payload"))?;
                if self.locals[local.index()].ownership == crate::mir::OwnershipKind::Move {
                    Operand::Move(Place::local(local))
                } else {
                    Operand::Copy(Place::local(local))
                }
            }
            crate::mir::ValueRepresentation::Borrow => {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(expression.span())
                    .map_or(LocalOrigin::Desugared(expression.span()), |expression| {
                        LocalOrigin::Temporary(expression.id)
                    });
                let readwrite = match expression.without_groups() {
                    Expr::Borrow(borrow) => borrow.is_readwrite,
                    _ => contract
                        .payload_borrow_readwrite
                        .ok_or(BuildError::MissingTypedExpression)?,
                };
                let local = LocalId::from_index(self.locals.len());
                self.locals.push(crate::mir::Local::borrow(
                    contract.payload_ty,
                    readwrite,
                    LocalStorage::Local,
                    origin,
                    scope,
                ));
                let first_new_loan = self.loans.len();
                self.lower_value_to_place(
                    local,
                    expression,
                    contract.payload_ty,
                    contract.payload_representation,
                    scope,
                )?;
                // A borrow returned inside an outcome is an operand of the
                // terminal edge, not an intermediate value in return storage.
                // Preserve any loan that directly initializes this staging
                // place until that edge has consumed it.
                for loan in &mut self.loans[first_new_loan..] {
                    if loan.destination == local && loan.lifetime == crate::mir::LoanLifetime::Scope
                    {
                        loan.lifetime = crate::mir::LoanLifetime::Return;
                    }
                }
                if self.locals[local.index()].ownership
                    == (crate::mir::OwnershipKind::Borrowed { readwrite: true })
                {
                    Operand::Move(Place::local(local))
                } else {
                    Operand::Copy(Place::local(local))
                }
            }
            crate::mir::ValueRepresentation::Unit | crate::mir::ValueRepresentation::Error => {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
        };
        self.control_flow
            .terminate(crate::mir::Terminator::ReturnOutcomeSuccess { source })
    }

    pub(super) fn lower_failure_return(
        &mut self,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        if !super::source_model::failure_value_is_supported(expression, self.semantic) {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        let (code, message) = self.lower_error_operands(expression, scope)?;
        self.control_flow
            .terminate(crate::mir::Terminator::ReturnFailure { code, message })
    }

    pub(super) fn lower_value_to_place(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        ty: crate::semantic::TyId,
        representation: crate::mir::ValueRepresentation,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        if let Expr::Call(call) = expression.without_groups() {
            let first_new_loan = self.loans.len();
            if let Some(value) = self.lower_intrinsic_rvalue(call, ty, representation, scope)? {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(expression.span())
                    .map_or(
                        crate::mir::Origin::Desugared(expression.span()),
                        |expression| crate::mir::Origin::Expression(expression.id),
                    );
                let call_loans = match &value {
                    Rvalue::Intrinsic { arguments, .. } => self.loans[first_new_loan..]
                        .iter()
                        .filter(|loan| {
                            loan.lifetime == crate::mir::LoanLifetime::Call
                                && arguments.iter().any(|argument| {
                                    matches!(
                                        argument.operand,
                                        Operand::Copy(place) | Operand::Move(place)
                                            if place == Place::local(loan.destination)
                                    )
                                })
                        })
                        .map(|loan| loan.id)
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                self.control_flow.push_statement(Statement::Assign {
                    destination: Place::local(destination),
                    value,
                    origin,
                })?;
                for loan in call_loans {
                    self.control_flow
                        .push_statement(Statement::EndLoan { loan })?;
                }
                return Ok(());
            }
        }
        match representation {
            crate::mir::ValueRepresentation::Unit => Err(BuildError::UnsupportedClaimedExpression),
            crate::mir::ValueRepresentation::Scalar(scalar) => self
                .lower_expression_to_place(destination, expression, ty, scalar, scope)
                .map_err(|error| error.context("lower scalar value to place")),
            crate::mir::ValueRepresentation::View(kind) => {
                self.lower_view_expression_to_place(destination, expression, ty, kind, scope)
            }
            crate::mir::ValueRepresentation::Aggregate => match expression.without_groups() {
                Expr::NoneLiteral(_) if self.type_has_outcome_layers(ty) => {
                    self.control_flow.push_statement(Statement::Assign {
                        destination: Place::local(destination),
                        value: Rvalue::OutcomeNone,
                        origin: crate::mir::Origin::Desugared(expression.span()),
                    })
                }
                _ if self.type_has_outcome_layers(ty)
                    && super::source_model::failure_value_is_supported(
                        expression,
                        self.semantic,
                    ) =>
                {
                    let (code, message) = self.lower_error_operands(expression, scope)?;
                    self.control_flow.push_statement(Statement::Assign {
                        destination: Place::local(destination),
                        value: Rvalue::OutcomeFailure { code, message },
                        origin: crate::mir::Origin::Desugared(expression.span()),
                    })
                }
                _ if self.type_has_outcome_layers(ty)
                    && !self.expression_has_outcome_value(expression) =>
                {
                    let value = self.lower_outcome_success_argument(ty, expression, scope)?;
                    self.control_flow.push_statement(Statement::Assign {
                        destination: Place::local(destination),
                        value: Rvalue::OutcomeSuccess { value },
                        origin: crate::mir::Origin::Desugared(expression.span()),
                    })
                }
                Expr::Call(call)
                    if matches!(call.callee.without_groups(), Expr::Member(member)
                        if self.semantic.typed_hir.enum_variant_target(member.member_span).is_some()) =>
                {
                    super::aggregates::lower_literal(self, destination, expression, scope)
                }
                Expr::Call(call) => {
                    let source = self
                        .semantic
                        .typed_hir
                        .expression(expression.span())
                        .ok_or(BuildError::MissingTypedExpression)?
                        .id;
                    let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                    if returns_never {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    self.control_flow
                        .emit_returning_call(source, callee, arguments, destination)
                }
                Expr::TypedSequenceLiteral(_) | Expr::TypedStringLiteral(_) => {
                    super::literals::lower_to_place(self, destination, expression, ty, scope)
                }
                Expr::InterpolatedString(interpolated) => {
                    super::interpolation::lower_to_place(self, destination, interpolated, ty, scope)
                }
                Expr::Member(member)
                    if self
                        .semantic
                        .typed_hir
                        .enum_variant_target(member.member_span)
                        .is_some() =>
                {
                    super::aggregates::lower_literal(self, destination, expression, scope)
                }
                Expr::Member(member) => {
                    let source = if super::source_model::aggregate_operand_is_supported(
                        expression,
                        self.semantic.resolved,
                        self.semantic.resolved_sources,
                        self.semantic.typed_hir,
                    ) {
                        let Operand::Copy(source) =
                            self.lower_aggregate_operand(expression, scope)?
                        else {
                            return Err(BuildError::UnsupportedClaimedExpression);
                        };
                        source
                    } else {
                        self.lower_value_member_source(
                            member,
                            ty,
                            crate::mir::ValueRepresentation::Aggregate,
                            scope,
                        )?
                    };
                    self.control_flow.push_statement(Statement::Assign {
                        destination: Place::local(destination),
                        value: Rvalue::Use(Operand::Copy(source)),
                        origin: crate::mir::Origin::Expression(
                            self.semantic
                                .typed_hir
                                .expression(member.span)
                                .ok_or(BuildError::MissingTypedExpression)?
                                .id,
                        ),
                    })
                }
                Expr::Identifier(_) | Expr::Unary(_) | Expr::Index(_) => {
                    let source = self
                        .semantic
                        .typed_hir
                        .expression(expression.span())
                        .ok_or(BuildError::MissingTypedExpression)?
                        .id;
                    let operand = self.lower_aggregate_operand(expression, scope)?;
                    self.control_flow.push_statement(Statement::Assign {
                        destination: Place::local(destination),
                        value: Rvalue::Use(operand),
                        origin: crate::mir::Origin::Expression(source),
                    })
                }
                Expr::Force(force) => {
                    if let Expr::Call(call) = force.expression.without_groups()
                        && super::source_model::call_has_single_outcome_layer(call, self.semantic)
                    {
                        let source = self
                            .semantic
                            .typed_hir
                            .expression(call.span)
                            .ok_or(BuildError::MissingTypedExpression)?
                            .id;
                        let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                        if returns_never {
                            return Err(BuildError::UnsupportedClaimedExpression);
                        }
                        self.control_flow.emit_trapping_outcome_call(
                            source,
                            callee,
                            arguments,
                            destination,
                        )
                    } else {
                        outcomes::lower_terminal_stored_outcome_to_place(
                            self,
                            destination,
                            &force.expression,
                            crate::mir::Terminator::Trap,
                            scope,
                        )
                    }
                }
                Expr::Propagate(propagate) => {
                    if let Expr::Call(call) = propagate.expression.without_groups()
                        && super::source_model::call_has_single_outcome_layer(call, self.semantic)
                    {
                        let source = self
                            .semantic
                            .typed_hir
                            .expression(call.span)
                            .ok_or(BuildError::MissingTypedExpression)?
                            .id;
                        let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                        if returns_never {
                            return Err(BuildError::UnsupportedClaimedExpression);
                        }
                        self.control_flow.emit_propagating_outcome_call(
                            source,
                            callee,
                            arguments,
                            destination,
                        )
                    } else {
                        outcomes::lower_terminal_stored_outcome_to_place(
                            self,
                            destination,
                            &propagate.expression,
                            crate::mir::Terminator::PropagateFailure,
                            scope,
                        )
                    }
                }
                Expr::Otherwise(otherwise) => outcomes::lower_aggregate_otherwise_to_place(
                    self,
                    destination,
                    otherwise,
                    ty,
                    scope,
                ),
                Expr::Catch(catch) => {
                    outcomes::lower_aggregate_catch_to_place(self, destination, catch, ty, scope)
                }
                Expr::If(conditional) => control_flow_expressions::lower_conditional_to_place(
                    self,
                    destination,
                    conditional,
                    ty,
                    crate::mir::ValueRepresentation::Aggregate,
                    scope,
                ),
                Expr::IfIs(if_is) => control_flow_expressions::lower_if_is_to_place(
                    self,
                    if_is,
                    destination,
                    ty,
                    crate::mir::ValueRepresentation::Aggregate,
                    scope,
                    false,
                ),
                Expr::Match(match_) => control_flow_expressions::lower_match_to_place(
                    self,
                    match_,
                    destination,
                    ty,
                    crate::mir::ValueRepresentation::Aggregate,
                    scope,
                    false,
                ),
                _ => super::aggregates::lower_literal(self, destination, expression, scope)
                    .map_err(|error| error.context("lower aggregate literal")),
            },
            crate::mir::ValueRepresentation::Borrow => {
                if self.lower_coercion_to_local(destination, expression, scope)? {
                    return Ok(());
                }
                self.lower_borrow_value_to_place_without_coercion(
                    destination,
                    expression,
                    ty,
                    scope,
                )
            }
            crate::mir::ValueRepresentation::Error => {
                self.lower_error_value_to_place(destination, expression, scope)
            }
        }
    }

    fn lower_error_value_to_place(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        let value = if super::source_model::failure_value_is_supported(expression, self.semantic) {
            let (code, message) = self.lower_error_operands(expression, scope)?;
            Rvalue::Error { code, message }
        } else {
            match expression.without_groups() {
                Expr::Identifier(identifier) => {
                    let symbol = self
                        .semantic
                        .resolved
                        .local_symbol_for_identifier(identifier)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    let source = *self
                        .places_by_symbol
                        .get(&symbol.id)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    if source.projection.is_some()
                        || self.locals[source.local.index()].representation
                            != crate::mir::ValueRepresentation::Error
                    {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    Rvalue::Use(Operand::Copy(source))
                }
                _ => return Err(BuildError::UnsupportedClaimedExpression),
            }
        };
        self.control_flow.push_statement(Statement::Assign {
            destination: Place::local(destination),
            value,
            origin: crate::mir::Origin::Desugared(expression.span()),
        })
    }

    pub(super) fn type_has_outcome_layers(&self, ty: crate::semantic::TyId) -> bool {
        self.semantic
            .typed_hir
            .type_expr_by_id(ty)
            .is_some_and(|ty| {
                !crate::outcomes::outcome_shape_with_resolver(
                    ty,
                    self.semantic.resolved,
                    |source| self.semantic.resolver_for(source),
                )
                .layers
                .is_empty()
            })
    }

    fn expression_has_outcome_value(&self, expression: &Expr) -> bool {
        super::source_model::expression_has_outcome_value(expression, self.semantic)
    }

    fn lower_outcome_success_argument(
        &mut self,
        outcome_ty: crate::semantic::TyId,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<crate::mir::CallArgument, BuildError> {
        let outcome_type = self
            .semantic
            .typed_hir
            .type_expr_by_id(outcome_ty)
            .ok_or(BuildError::MissingTypedExpression)?;
        let shape = crate::outcomes::outcome_shape_with_resolver(
            outcome_type,
            self.semantic.resolved,
            |source| self.semantic.resolver_for(source),
        );
        let payload_ty = self
            .semantic
            .typed_hir
            .type_id(&shape.payload)
            .ok_or(BuildError::MissingTypedExpression)?;
        let representation = super::source_model::value_representation(payload_ty, self.semantic)
            .ok_or(BuildError::MissingTypedExpression)?;
        let operand = match representation {
            crate::mir::ValueRepresentation::Scalar(scalar) => {
                self.lower_operand(expression, payload_ty, scalar, scope)?
            }
            crate::mir::ValueRepresentation::View(kind) => {
                self.lower_view_operand(expression, payload_ty, kind, scope)?
            }
            crate::mir::ValueRepresentation::Aggregate => {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(expression.span())
                    .map_or(
                        crate::mir::LocalOrigin::Desugared(expression.span()),
                        |expression| crate::mir::LocalOrigin::Temporary(expression.id),
                    );
                let temporary = self.aggregate_temporary(payload_ty, origin, scope)?;
                self.lower_value_to_place(
                    temporary,
                    expression,
                    payload_ty,
                    representation,
                    scope,
                )?;
                if self.locals[temporary.index()].ownership == crate::mir::OwnershipKind::Move {
                    Operand::Move(Place::local(temporary))
                } else {
                    Operand::Copy(Place::local(temporary))
                }
            }
            crate::mir::ValueRepresentation::Borrow => {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(expression.span())
                    .map_or(
                        crate::mir::LocalOrigin::Desugared(expression.span()),
                        |expression| crate::mir::LocalOrigin::Temporary(expression.id),
                    );
                let temporary = self.local_for_type(payload_ty, origin, scope)?;
                self.lower_value_to_place(
                    temporary,
                    expression,
                    payload_ty,
                    representation,
                    scope,
                )?;
                Operand::Copy(Place::local(temporary))
            }
            crate::mir::ValueRepresentation::Unit => {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(expression.span())
                    .map_or(
                        crate::mir::LocalOrigin::Desugared(expression.span()),
                        |expression| crate::mir::LocalOrigin::Temporary(expression.id),
                    );
                let temporary = LocalId::from_index(self.locals.len());
                self.locals.push(crate::mir::Local::unit(
                    payload_ty,
                    crate::mir::LocalStorage::Local,
                    origin,
                    scope,
                ));
                Operand::Copy(Place::local(temporary))
            }
            crate::mir::ValueRepresentation::Error => {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
        };
        Ok(crate::mir::CallArgument {
            operand,
            ty: payload_ty,
            representation,
        })
    }

    fn lower_error_operands(
        &mut self,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<(Operand, Operand), BuildError> {
        match expression.without_groups() {
            Expr::Call(call) => {
                if let Some([code, message]) =
                    super::source_model::error_constructor_arguments(call, self.semantic)
                {
                    let code_ty = known_expression_type(code, self.semantic.typed_hir)
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let message_ty = known_expression_type(message, self.semantic.typed_hir)
                        .ok_or(BuildError::MissingTypedExpression)?;
                    if value_representation(code_ty, self.semantic)
                        != Some(crate::mir::ValueRepresentation::View(
                            crate::mir::ViewKind::Str,
                        ))
                        || value_representation(message_ty, self.semantic)
                            != Some(crate::mir::ValueRepresentation::View(
                                crate::mir::ViewKind::Str,
                            ))
                    {
                        return Err(BuildError::UnsupportedSource {
                            span: call.span,
                            construct: "error constructors with non-`&str` payloads",
                            help: "pass `&str` code and message values to an error constructor",
                        });
                    }
                    return Ok((
                        self.lower_view_operand(code, code_ty, crate::mir::ViewKind::Str, scope)
                            .map_err(|error| error.context("lower error code"))?,
                        self.lower_view_operand(
                            message,
                            message_ty,
                            crate::mir::ViewKind::Str,
                            scope,
                        )
                        .map_err(|error| error.context("lower error message"))?,
                    ));
                }
                if matches!(call.callee.without_groups(), Expr::Member(member)
                    if self.semantic.typed_hir.method_call_target(member.member_span).is_some())
                {
                    return Err(BuildError::UnsupportedSource {
                        span: call.span,
                        construct: "error-returning method calls in failure positions",
                        help: "return an `error` value from a free zero-argument helper, or construct the error directly at the failure site",
                    });
                }
                let expression_fact = self
                    .semantic
                    .typed_hir
                    .expression(expression.span())
                    .ok_or(BuildError::MissingTypedExpression)?;
                let return_ty = self
                    .semantic
                    .resolved
                    .function_signature_for_call(call)
                    .or_else(|| {
                        self.semantic
                            .resolved
                            .associated_function_signature_for_call(call)
                    })
                    .map(|signature| &signature.return_type)
                    .ok_or(BuildError::MissingTypedExpression)?;
                let ty = self
                    .semantic
                    .typed_hir
                    .type_id(return_ty)
                    .ok_or(BuildError::MissingTypedExpression)?;
                let temporary =
                    self.local_for_type(ty, LocalOrigin::Temporary(expression_fact.id), scope)?;
                if self.locals[temporary.index()].representation
                    != crate::mir::ValueRepresentation::Error
                {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                if returns_never {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                if !arguments.is_empty() {
                    return Err(BuildError::UnsupportedSource {
                        span: call.span,
                        construct: "error-returning helpers with runtime arguments",
                        help: "construct the error directly at the failure site until runtime error values have a complete ABI",
                    });
                }
                self.control_flow.emit_returning_call(
                    expression_fact.id,
                    callee,
                    arguments,
                    temporary,
                )?;
                self.error_place_operands(Place::local(temporary), expression.span())
            }
            Expr::Identifier(identifier) => {
                let symbol = self
                    .semantic
                    .resolved
                    .local_symbol_for_identifier(identifier)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let source = *self
                    .places_by_symbol
                    .get(&symbol.id)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                self.error_place_operands(source, expression.span())
            }
            _ => Err(BuildError::UnsupportedClaimedExpression),
        }
    }

    fn error_place_operands(
        &mut self,
        source: Place,
        span: crate::source::ByteSpan,
    ) -> Result<(Operand, Operand), BuildError> {
        if source.projection.is_some()
            || self.locals[source.local.index()].representation
                != crate::mir::ValueRepresentation::Error
        {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        let str_ty = self
            .semantic
            .typed_hir
            .type_id(&crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
                span,
                is_readwrite: false,
                inner: Box::new(crate::ast::TypeExpr::Reference(crate::ast::TypeReference {
                    span,
                    name: "str".to_string(),
                })),
            }))
            .ok_or(BuildError::MissingTypedExpression)?;
        Ok((
            Operand::Copy(super::projections::push_error_field_place(
                source.local,
                crate::builtin_types::BuiltinErrorField::Code,
                str_ty,
                &mut self.projections,
            )),
            Operand::Copy(super::projections::push_error_field_place(
                source.local,
                crate::builtin_types::BuiltinErrorField::Message,
                str_ty,
                &mut self.projections,
            )),
        ))
    }

    fn lower_borrow_value_to_place_without_coercion(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        ty: crate::semantic::TyId,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        match expression.without_groups() {
            Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => self
                .lower_borrow_value_to_place_without_coercion(
                    destination,
                    &unary.operand,
                    ty,
                    scope,
                ),
            Expr::Borrow(_) => super::borrows::lower_to_local_without_coercion(
                self,
                destination,
                expression,
                self.locals[destination.index()].ownership
                    == crate::mir::OwnershipKind::Borrowed { readwrite: true },
                scope,
                crate::mir::LoanLifetime::Scope,
            ),
            Expr::Call(call) => {
                let source = self
                    .semantic
                    .typed_hir
                    .expression(call.span)
                    .ok_or(BuildError::MissingTypedExpression)?
                    .id;
                let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                if returns_never {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                self.control_flow
                    .emit_returning_call(source, callee, arguments, destination)
            }
            Expr::If(if_) => lower_conditional_to_place(
                self,
                destination,
                if_,
                ty,
                crate::mir::ValueRepresentation::Borrow,
                scope,
            ),
            Expr::IfIs(if_is) => lower_if_is_to_place(
                self,
                if_is,
                destination,
                ty,
                crate::mir::ValueRepresentation::Borrow,
                scope,
                false,
            ),
            Expr::Match(match_) => lower_match_to_place(
                self,
                match_,
                destination,
                ty,
                crate::mir::ValueRepresentation::Borrow,
                scope,
                false,
            ),
            Expr::Force(force) => {
                if let Expr::Call(call) = force.expression.without_groups()
                    && super::source_model::call_has_single_outcome_layer(call, self.semantic)
                {
                    let source = self
                        .semantic
                        .typed_hir
                        .expression(call.span)
                        .ok_or(BuildError::MissingTypedExpression)?
                        .id;
                    let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                    if returns_never {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    self.control_flow.emit_trapping_outcome_call(
                        source,
                        callee,
                        arguments,
                        destination,
                    )
                } else {
                    outcomes::lower_terminal_stored_outcome_to_place(
                        self,
                        destination,
                        &force.expression,
                        crate::mir::Terminator::Trap,
                        scope,
                    )
                }
            }
            Expr::Propagate(propagate) => {
                if let Expr::Call(call) = propagate.expression.without_groups()
                    && super::source_model::call_has_single_outcome_layer(call, self.semantic)
                {
                    let source = self
                        .semantic
                        .typed_hir
                        .expression(call.span)
                        .ok_or(BuildError::MissingTypedExpression)?
                        .id;
                    let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                    if returns_never {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    self.control_flow.emit_propagating_outcome_call(
                        source,
                        callee,
                        arguments,
                        destination,
                    )
                } else {
                    outcomes::lower_terminal_stored_outcome_to_place(
                        self,
                        destination,
                        &propagate.expression,
                        crate::mir::Terminator::PropagateFailure,
                        scope,
                    )
                }
            }
            Expr::Otherwise(otherwise) => {
                outcomes::lower_borrow_otherwise_to_place(self, destination, otherwise, ty, scope)
            }
            Expr::Catch(catch) => {
                outcomes::lower_borrow_catch_to_place(self, destination, catch, ty, scope)
            }
            Expr::Identifier(_) => {
                let source = self.lower_stored_identifier(expression)?;
                self.control_flow.push_statement(Statement::Assign {
                    destination: Place::local(destination),
                    value: Rvalue::Use(source),
                    origin: crate::mir::Origin::Desugared(expression.span()),
                })
            }
            _ => Err(BuildError::UnsupportedClaimedExpression),
        }
    }

    pub(super) fn lower_value_member_source(
        &mut self,
        member: &crate::ast::MemberExpr,
        result_ty: crate::semantic::TyId,
        result_representation: crate::mir::ValueRepresentation,
        scope: ScopeId,
    ) -> Result<Place, BuildError> {
        let root = super::projections::member_chain_root(member);
        let root_expression = self
            .semantic
            .typed_hir
            .expression(root.span())
            .ok_or(BuildError::MissingTypedExpression)?;
        let crate::typecheck::PartialSemantic::Known(root_ty) = root_expression.ty else {
            return Err(BuildError::MissingTypedExpression);
        };
        if super::source_model::value_representation(root_ty, self.semantic)
            != Some(crate::mir::ValueRepresentation::Aggregate)
        {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        let root_place = if let Expr::Index(index) = root.without_groups()
            && super::indexes::is_supported(index, self.semantic)
        {
            let (place, representation) = super::indexes::lower_place(self, index, scope)?;
            if representation != crate::mir::ValueRepresentation::Aggregate {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            place
        } else {
            let temporary = self.aggregate_temporary(
                root_ty,
                LocalOrigin::Temporary(root_expression.id),
                scope,
            )?;
            self.lower_value_to_place(
                temporary,
                root,
                root_ty,
                crate::mir::ValueRepresentation::Aggregate,
                scope,
            )?;
            Place::local(temporary)
        };
        let (source, representation) = super::projections::lower_field_place_from_value_root(
            member,
            root_ty,
            root_place,
            self.semantic,
            &mut self.projections,
            &mut self.drop_plans,
        )?;
        if representation != result_representation
            || self.projections[source
                .projection
                .ok_or(BuildError::UnsupportedClaimedExpression)?
                .index()]
            .ty != result_ty
        {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        Ok(source)
    }

    pub(super) fn lower_call(
        &mut self,
        call: &crate::ast::CallExpr,
        scope: ScopeId,
    ) -> Result<(crate::mir::CallInstance, Vec<CallArgument>, bool), BuildError> {
        if let Some(symbol) = self.semantic.resolved.symbol_for_call(call)
            && let crate::resolve::SymbolKind::Imported(imported) = &symbol.kind
            && imported.kind == crate::resolve::ImportedSymbolKind::UnloadedName
        {
            return Err(BuildError::UnloadedImportedCall {
                span: call.span,
                path: imported.path.clone(),
            });
        }
        if self
            .semantic
            .typed_hir
            .generic_function_call_target(call.span)
            .is_some()
            && self
                .semantic
                .typed_hir
                .function_call_specialization(call.span)
                .is_none()
        {
            return Err(BuildError::UnspecializedGenericCall { span: call.span });
        }
        let returns_never = self
            .semantic
            .typed_hir
            .expression(call.span)
            .is_some_and(|expression| expression.diverges);
        if let Some(intrinsic) = super::source_model::intrinsic_for_call(call, self.semantic)
            .filter(|intrinsic| {
                super::source_model::outcome_intrinsic_is_supported(*intrinsic)
                    || super::source_model::never_intrinsic_is_supported(*intrinsic)
            })
        {
            let arguments = call
                .arguments
                .iter()
                .map(|argument| self.lower_call_argument(argument, scope))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok((
                crate::mir::CallInstance::intrinsic(intrinsic),
                arguments,
                returns_never,
            ));
        }
        let (callee, receiver) = match call.callee.without_groups() {
            _ if self.semantic.typed_hir.callable_call(call.span).is_some() => {
                let fact = self
                    .semantic
                    .typed_hir
                    .callable_call(call.span)
                    .ok_or(BuildError::MissingCallTarget)?;
                let callable_ty = self
                    .semantic
                    .typed_hir
                    .type_id(&fact.specialization.callable_ty)
                    .ok_or(BuildError::MissingTypedExpression)?;
                let receiver_ty = self
                    .semantic
                    .typed_hir
                    .type_id(&fact.receiver_ty)
                    .ok_or(BuildError::MissingTypedExpression)?;
                (
                    crate::mir::CallInstance::value(callable_ty, fact.specialization.capability),
                    Some(PlannedReceiver::Callable {
                        expression: &call.callee,
                        receiver_ty,
                        capability: fact.specialization.capability,
                    }),
                )
            }
            Expr::Identifier(identifier) => {
                let definition = self
                    .semantic
                    .typed_hir
                    .function_call_target(identifier.span)
                    .map(|definition| {
                        self.semantic
                            .resolved
                            .callable_bodies
                            .canonical_definition(definition)
                    })
                    .ok_or(BuildError::MissingCallTarget)?;
                let instance = if let Some(specialization) = self
                    .semantic
                    .typed_hir
                    .function_call_specialization(call.span)
                {
                    crate::mir::CallInstance::specialized(
                        definition,
                        None,
                        self.call_type_arguments(specialization.ordered_type_arguments())?,
                    )
                } else {
                    crate::mir::CallInstance::direct(definition)
                };
                (instance, None)
            }
            Expr::Member(member) => {
                if let Some(definition) = self
                    .semantic
                    .typed_hir
                    .function_call_target(member.member_span)
                {
                    let definition = self
                        .semantic
                        .resolved
                        .callable_bodies
                        .canonical_definition(definition);
                    let instance = if let Some(specialization) = self
                        .semantic
                        .typed_hir
                        .function_call_specialization(call.span)
                    {
                        crate::mir::CallInstance::specialized(
                            definition,
                            None,
                            self.call_type_arguments(specialization.ordered_type_arguments())?,
                        )
                    } else {
                        crate::mir::CallInstance::direct(definition)
                    };
                    (instance, None)
                } else if let Some(definition) = self
                    .semantic
                    .typed_hir
                    .associated_function_target(member.member_span)
                {
                    let definition = self
                        .semantic
                        .resolved
                        .callable_bodies
                        .canonical_definition(definition);
                    let instance = if let Some(specialization) = self
                        .semantic
                        .typed_hir
                        .function_call_specialization(call.span)
                    {
                        crate::mir::CallInstance::specialized(
                            definition,
                            None,
                            self.call_type_arguments(specialization.ordered_type_arguments())?,
                        )
                    } else {
                        crate::mir::CallInstance::direct(definition)
                    };
                    (instance, None)
                } else {
                    let definition = self
                        .semantic
                        .typed_hir
                        .method_call_target(member.member_span)
                        .map(|definition| {
                            self.semantic
                                .resolved
                                .callable_bodies
                                .canonical_definition(definition)
                        })
                        .ok_or(BuildError::MissingCallTarget)?;
                    let instance = if let Some(specialization) = self
                        .semantic
                        .typed_hir
                        .method_call_specialization(member.member_span)
                    {
                        let receiver = super::storage_types::runtime_type_id_for_type_expr(
                            &specialization.self_ty,
                            self.semantic,
                        )
                        .ok_or(BuildError::MissingSpecializedReceiverType)?;
                        crate::mir::CallInstance::specialized(
                            definition,
                            Some(receiver),
                            self.call_type_arguments(specialization.ordered_type_arguments())?,
                        )
                    } else {
                        crate::mir::CallInstance::direct(definition)
                    };
                    (instance, Some(PlannedReceiver::Method(member)))
                }
            }
            _ => return Err(BuildError::UnsupportedClaimedExpression),
        };
        let mut arguments = Vec::new();
        if let Some(receiver) = receiver {
            arguments.push(match receiver {
                PlannedReceiver::Method(member) => self
                    .lower_method_receiver(call, member, scope)
                    .map_err(|error| error.context("lower method receiver"))?,
                PlannedReceiver::Callable {
                    expression,
                    receiver_ty,
                    capability,
                } => {
                    self.lower_callable_receiver(call, expression, receiver_ty, capability, scope)?
                }
            });
        }
        for argument in &call.arguments {
            arguments.push(
                self.lower_call_argument(argument, scope)
                    .map_err(|error| error.context("lower call argument"))?,
            );
        }
        Ok((callee, arguments, returns_never))
    }

    fn call_type_arguments(
        &self,
        arguments: Option<Vec<&crate::ast::TypeExpr>>,
    ) -> Result<Vec<crate::semantic::TyId>, BuildError> {
        arguments
            .ok_or(BuildError::MissingCallTarget)?
            .into_iter()
            .map(|ty| {
                self.semantic
                    .typed_hir
                    .type_id(ty)
                    .ok_or(BuildError::MissingTypedExpression)
            })
            .collect()
    }

    pub(super) fn lower_call_argument(
        &mut self,
        argument: &Expr,
        scope: ScopeId,
    ) -> Result<CallArgument, BuildError> {
        let ty =
            super::source_model::conversion_plan_for_expression(argument, self.semantic.typed_hir)
                .and_then(|conversion| self.semantic.typed_hir.type_id(&conversion.target_ty))
                .or_else(|| {
                    super::source_model::handled_outcome_success_type(argument, self.semantic)
                })
                .or_else(|| match argument.without_groups() {
                    Expr::Call(call) => super::source_model::call_result_type(call, self.semantic),
                    _ => super::source_model::expression_value_type(argument, self.semantic),
                })
                .ok_or(BuildError::MissingTypedExpression)?;
        let ty = super::storage_types::normalized_storage_type(ty, self.semantic);
        if super::source_model::failure_value_is_supported(argument, self.semantic) {
            let error_ty = self
                .semantic
                .typed_hir
                .type_id(&crate::ast::TypeExpr::Reference(
                    crate::ast::TypeReference {
                        span: argument.span(),
                        name: "error".to_string(),
                    },
                ))
                .unwrap_or(ty);
            let operand = if matches!(argument.without_groups(), Expr::Identifier(_)) {
                self.lower_stored_identifier(argument)?
            } else {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(argument.span())
                    .map_or(LocalOrigin::Desugared(argument.span()), |expression| {
                        LocalOrigin::Temporary(expression.id)
                    });
                let local = self.local_for_type(error_ty, origin, scope)?;
                self.lower_error_value_to_place(local, argument, scope)
                    .map_err(|error| error.context("materialize error argument"))?;
                Operand::Copy(Place::local(local))
            };
            return Ok(CallArgument {
                operand,
                ty: error_ty,
                representation: crate::mir::ValueRepresentation::Error,
            });
        }
        if let Some(scalar) = super::source_model::value_scalar_type(ty, self.semantic) {
            let operand = self.lower_operand(argument, ty, scalar, scope)?;
            return Ok(CallArgument {
                operand,
                ty,
                representation: crate::mir::ValueRepresentation::Scalar(scalar),
            });
        }
        if let Some(representation @ crate::mir::ValueRepresentation::View(kind)) =
            value_representation(ty, self.semantic)
        {
            let ty = super::source_model::intrinsic_expression_type(
                argument.span(),
                self.semantic.typed_hir,
            )
            .filter(|ty| value_representation(*ty, self.semantic) == Some(representation))
            .unwrap_or(ty);
            if matches!(
                argument.without_groups(),
                Expr::If(_) | Expr::IfIs(_) | Expr::Match(_)
            ) {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(argument.span())
                    .map_or(LocalOrigin::Desugared(argument.span()), |expression| {
                        LocalOrigin::Temporary(expression.id)
                    });
                let local = LocalId::from_index(self.locals.len());
                self.locals.push(crate::mir::Local::view(
                    ty,
                    kind,
                    LocalStorage::Local,
                    origin,
                    scope,
                ));
                self.lower_view_expression_to_place(local, argument, ty, kind, scope)?;
                return Ok(CallArgument {
                    operand: Operand::Copy(Place::local(local)),
                    ty,
                    representation,
                });
            }
            return Ok(CallArgument {
                operand: self.lower_view_operand(argument, ty, kind, scope)?,
                ty,
                representation,
            });
        }
        if value_representation(ty, self.semantic) == Some(crate::mir::ValueRepresentation::Error) {
            let operand = if matches!(argument.without_groups(), Expr::Identifier(_)) {
                self.lower_stored_identifier(argument)?
            } else {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(argument.span())
                    .map_or(LocalOrigin::Desugared(argument.span()), |expression| {
                        LocalOrigin::Temporary(expression.id)
                    });
                let local = self.local_for_type(ty, origin, scope)?;
                self.lower_error_value_to_place(local, argument, scope)
                    .map_err(|error| error.context("materialize error argument"))?;
                Operand::Copy(Place::local(local))
            };
            return Ok(CallArgument {
                operand,
                ty,
                representation: crate::mir::ValueRepresentation::Error,
            });
        }
        if value_representation(ty, self.semantic) == Some(crate::mir::ValueRepresentation::Borrow)
        {
            let borrow_expression = match argument.without_groups() {
                Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
                    unary.operand.as_ref()
                }
                _ => argument,
            };
            let operand = if super::source_model::conversion_plan_for_expression(
                borrow_expression,
                self.semantic.typed_hir,
            )
            .is_some()
            {
                let typed_expression = self
                    .semantic
                    .typed_hir
                    .expression(borrow_expression.span())
                    .ok_or(BuildError::MissingTypedExpression)?;
                let crate::ast::TypeExpr::Borrow(borrow_ty) = self
                    .semantic
                    .typed_hir
                    .type_expr_by_id(ty)
                    .ok_or(BuildError::MissingTypedExpression)?
                else {
                    return Err(BuildError::UnsupportedClaimedExpression);
                };
                let local = LocalId::from_index(self.locals.len());
                self.locals.push(crate::mir::Local::borrow(
                    ty,
                    borrow_ty.is_readwrite,
                    LocalStorage::Local,
                    LocalOrigin::Temporary(typed_expression.id),
                    scope,
                ));
                super::borrows::lower_to_local(
                    self,
                    local,
                    borrow_expression,
                    borrow_ty.is_readwrite,
                    scope,
                    crate::mir::LoanLifetime::Call,
                )
                .map_err(|error| error.context("lower converted borrow argument"))?;
                if borrow_ty.is_readwrite {
                    Operand::Move(Place::local(local))
                } else {
                    Operand::Copy(Place::local(local))
                }
            } else if super::source_model::borrow_identifier_is_supported(
                borrow_expression,
                self.semantic.resolved,
                self.semantic.typed_hir,
            ) {
                self.lower_stored_identifier(borrow_expression)?
            } else {
                let typed_expression = self
                    .semantic
                    .typed_hir
                    .expression(borrow_expression.span())
                    .ok_or(BuildError::MissingTypedExpression)?;
                let crate::ast::TypeExpr::Borrow(borrow_ty) = self
                    .semantic
                    .typed_hir
                    .type_expr_by_id(ty)
                    .ok_or(BuildError::MissingTypedExpression)?
                else {
                    return Err(BuildError::UnsupportedClaimedExpression);
                };
                let local = LocalId::from_index(self.locals.len());
                self.locals.push(crate::mir::Local::borrow(
                    ty,
                    borrow_ty.is_readwrite,
                    LocalStorage::Local,
                    LocalOrigin::Temporary(typed_expression.id),
                    scope,
                ));
                super::borrows::lower_to_local(
                    self,
                    local,
                    borrow_expression,
                    borrow_ty.is_readwrite,
                    scope,
                    crate::mir::LoanLifetime::Call,
                )
                .map_err(|error| error.context("lower borrow argument"))?;
                if borrow_ty.is_readwrite {
                    Operand::Move(Place::local(local))
                } else {
                    Operand::Copy(Place::local(local))
                }
            };
            let operand_ty = match operand {
                Operand::Copy(place) | Operand::Move(place) => place
                    .projection
                    .and_then(|projection| self.projections.get(projection.index()))
                    .map_or(self.locals[place.local.index()].ty, |projection| {
                        projection.ty
                    }),
                Operand::Constant(_) | Operand::StaticStr { .. } => ty,
            };
            return Ok(CallArgument {
                operand,
                ty: operand_ty,
                representation: crate::mir::ValueRepresentation::Borrow,
            });
        }
        if value_representation(ty, self.semantic)
            == Some(crate::mir::ValueRepresentation::Aggregate)
            && (super::aggregates::literal_is_supported(argument, self.semantic)
                || matches!(
                    argument.without_groups(),
                    Expr::StructLiteral(_)
                        | Expr::ArrayLiteral(_)
                        | Expr::Closure(_)
                        | Expr::InterpolatedString(_)
                        | Expr::If(_)
                        | Expr::IfIs(_)
                        | Expr::Match(_)
                )
                || matches!(argument.without_groups(), Expr::Call(call)
                    if matches!(call.callee.without_groups(), Expr::Member(member)
                        if self.semantic.typed_hir.enum_variant_target(member.member_span).is_some())))
        {
            let expression = self
                .semantic
                .typed_hir
                .expression(argument.span())
                .ok_or(BuildError::MissingTypedExpression)?;
            let local =
                self.aggregate_temporary(ty, LocalOrigin::Temporary(expression.id), scope)?;
            self.lower_value_to_place(
                local,
                argument,
                ty,
                crate::mir::ValueRepresentation::Aggregate,
                scope,
            )?;
            let ownership = self.locals[local.index()].ownership;
            return Ok(CallArgument {
                operand: if ownership == crate::mir::OwnershipKind::Move {
                    Operand::Move(Place::local(local))
                } else {
                    Operand::Copy(Place::local(local))
                },
                ty,
                representation: crate::mir::ValueRepresentation::Aggregate,
            });
        }
        if matches!(
            argument.without_groups(),
            Expr::Call(_)
                | Expr::Force(_)
                | Expr::Propagate(_)
                | Expr::Otherwise(_)
                | Expr::Catch(_)
        ) && value_representation(ty, self.semantic)
            == Some(crate::mir::ValueRepresentation::Aggregate)
        {
            let origin = self
                .semantic
                .typed_hir
                .expression(argument.span())
                .map_or(LocalOrigin::Desugared(argument.span()), |expression| {
                    LocalOrigin::Temporary(expression.id)
                });
            let local = self.aggregate_temporary(ty, origin, scope)?;
            self.lower_value_to_place(
                local,
                argument,
                ty,
                crate::mir::ValueRepresentation::Aggregate,
                scope,
            )?;
            let operand = if self.locals[local.index()].ownership == crate::mir::OwnershipKind::Move
            {
                Operand::Move(Place::local(local))
            } else {
                Operand::Copy(Place::local(local))
            };
            return Ok(CallArgument {
                operand,
                ty,
                representation: crate::mir::ValueRepresentation::Aggregate,
            });
        }
        if matches!(
            argument.without_groups(),
            Expr::TypedSequenceLiteral(_) | Expr::TypedStringLiteral(_)
        ) {
            let expression = self
                .semantic
                .typed_hir
                .expression(argument.span())
                .ok_or(BuildError::MissingTypedExpression)?;
            let local =
                self.aggregate_temporary(ty, LocalOrigin::Temporary(expression.id), scope)?;
            self.lower_value_to_place(
                local,
                argument,
                ty,
                crate::mir::ValueRepresentation::Aggregate,
                scope,
            )?;
            let operand = if self.locals[local.index()].ownership == crate::mir::OwnershipKind::Move
            {
                Operand::Move(Place::local(local))
            } else {
                Operand::Copy(Place::local(local))
            };
            return Ok(CallArgument {
                operand,
                ty,
                representation: crate::mir::ValueRepresentation::Aggregate,
            });
        }
        if let Expr::Member(member) = argument.without_groups()
            && super::projections::aggregate_value_field_is_supported(member, self.semantic)
            && !super::source_model::aggregate_operand_is_supported(
                argument,
                self.semantic.resolved,
                self.semantic.resolved_sources,
                self.semantic.typed_hir,
            )
        {
            let operand = Operand::Copy(self.lower_value_member_source(
                member,
                ty,
                crate::mir::ValueRepresentation::Aggregate,
                scope,
            )?);
            return Ok(CallArgument {
                operand,
                ty,
                representation: crate::mir::ValueRepresentation::Aggregate,
            });
        }
        let operand = self.lower_aggregate_operand(argument, scope)?;
        let operand = match &operand {
            Operand::Copy(place) | Operand::Move(place) if self.place_has_runtime_index(*place) => {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(argument.span())
                    .map_or(LocalOrigin::Desugared(argument.span()), |expression| {
                        LocalOrigin::Temporary(expression.id)
                    });
                let local = self.aggregate_temporary(ty, origin, scope)?;
                self.control_flow.push_statement(Statement::Assign {
                    destination: Place::local(local),
                    value: Rvalue::Use(operand),
                    origin: crate::mir::Origin::Desugared(argument.span()),
                })?;
                if self.locals[local.index()].ownership == crate::mir::OwnershipKind::Move {
                    Operand::Move(Place::local(local))
                } else {
                    Operand::Copy(Place::local(local))
                }
            }
            _ => operand,
        };
        Ok(CallArgument {
            operand,
            ty,
            representation: crate::mir::ValueRepresentation::Aggregate,
        })
    }

    fn place_has_runtime_index(&self, place: Place) -> bool {
        let Some(mut projection) = place.projection else {
            return false;
        };
        loop {
            let Some(path) = self.projections.get(projection.index()) else {
                return false;
            };
            if matches!(
                path.element,
                crate::mir::ProjectionElement::Index { .. }
                    | crate::mir::ProjectionElement::ViewIndex { .. }
            ) {
                return true;
            }
            let Some(parent) = path.parent else {
                return false;
            };
            projection = parent;
        }
    }

    fn lower_method_receiver(
        &mut self,
        call: &crate::ast::CallExpr,
        member: &crate::ast::MemberExpr,
        scope: ScopeId,
    ) -> Result<CallArgument, BuildError> {
        let kind = self
            .semantic
            .typed_hir
            .method_call_receiver_kind(member.member_span)
            .ok_or(BuildError::MissingCallTarget)?;
        if kind == crate::typecheck::TypecheckMethodReceiverKind::Owned {
            let ty = known_expression_type(&member.object, self.semantic.typed_hir)
                .ok_or(BuildError::MissingTypedExpression)?;
            if value_representation(ty, self.semantic)
                == Some(crate::mir::ValueRepresentation::Aggregate)
                && super::borrows::source_place_is_supported(&member.object, self.semantic)
            {
                let source = super::borrows::lower_source_place(self, &member.object, scope)?;
                return Ok(CallArgument {
                    operand: Operand::Move(source),
                    ty,
                    representation: crate::mir::ValueRepresentation::Aggregate,
                });
            }
            return self.lower_call_argument(&member.object, scope);
        }
        let readwrite = kind == crate::typecheck::TypecheckMethodReceiverKind::ReadwriteBorrow;
        let ty = self
            .semantic
            .typed_hir
            .method_call_receiver_type(member.member_span)
            .ok_or(BuildError::MissingMethodReceiverType)?;
        if let Some(plan) = self.semantic.typed_hir.coercion_plan(member.object.span()) {
            let ty = self
                .semantic
                .typed_hir
                .type_id(&plan.target_ty)
                .ok_or(BuildError::MissingMethodReceiverType)?;
            let representation = value_representation(ty, self.semantic)
                .ok_or(BuildError::UnsupportedClaimedExpression)
                .map_err(|error| error.context("resolve coerced method receiver representation"))?;
            let origin = self
                .semantic
                .typed_hir
                .expression(member.object.span())
                .ok_or(BuildError::MissingTypedExpression)?
                .id;
            let local = match representation {
                crate::mir::ValueRepresentation::Borrow => crate::mir::Local::borrow(
                    ty,
                    readwrite,
                    LocalStorage::Local,
                    LocalOrigin::Temporary(origin),
                    scope,
                ),
                crate::mir::ValueRepresentation::View(view) => crate::mir::Local::view(
                    ty,
                    view,
                    LocalStorage::Local,
                    LocalOrigin::Temporary(origin),
                    scope,
                ),
                _ => {
                    return Err(BuildError::UnsupportedClaimedExpression
                        .context("materialize coerced method receiver"));
                }
            };
            let local_id = LocalId::from_index(self.locals.len());
            self.locals.push(local);
            if !self
                .lower_coercion_to_local(local_id, &member.object, scope)
                .map_err(|error| error.context("lower method receiver coercion"))?
            {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            return Ok(CallArgument {
                operand: if readwrite && representation == crate::mir::ValueRepresentation::Borrow {
                    Operand::Move(Place::local(local_id))
                } else {
                    Operand::Copy(Place::local(local_id))
                },
                ty,
                representation,
            });
        }
        if let Some(representation @ crate::mir::ValueRepresentation::View(view)) =
            value_representation(ty, self.semantic)
        {
            let source_ty =
                super::source_model::handled_outcome_success_type(&member.object, self.semantic)
                    .or_else(|| known_expression_type(&member.object, self.semantic.typed_hir))
                    .ok_or(BuildError::MissingTypedExpression)?;
            let source = self
                .lower_view_operand(&member.object, source_ty, view, scope)
                .map_err(|error| error.context("lower view method receiver"))?;
            let operand = if source_ty == ty {
                source
            } else {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(member.object.span())
                    .ok_or(BuildError::MissingTypedExpression)?
                    .id;
                let temporary = LocalId::from_index(self.locals.len());
                self.locals.push(crate::mir::Local::view(
                    ty,
                    view,
                    LocalStorage::Local,
                    LocalOrigin::Temporary(origin),
                    scope,
                ));
                self.control_flow.push_statement(Statement::Assign {
                    destination: Place::local(temporary),
                    value: Rvalue::ViewCast {
                        source,
                        source_ty,
                        target_ty: ty,
                        kind: view,
                    },
                    origin: crate::mir::Origin::Expression(origin),
                })?;
                Operand::Copy(Place::local(temporary))
            };
            return Ok(CallArgument {
                operand,
                ty,
                representation,
            });
        }
        let source = self
            .semantic
            .typed_hir
            .expression(call.span)
            .ok_or(BuildError::MissingCallExpression)?
            .id;
        let local = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::Local::borrow(
            ty,
            readwrite,
            LocalStorage::Local,
            LocalOrigin::Temporary(source),
            scope,
        ));
        super::borrows::lower_implicit_to_local(
            self,
            local,
            &member.object,
            readwrite,
            scope,
            crate::mir::Origin::Expression(source),
        )
        .map_err(|error| error.context("lower implicit borrowed method receiver"))?;
        Ok(CallArgument {
            operand: if readwrite {
                Operand::Move(Place::local(local))
            } else {
                Operand::Copy(Place::local(local))
            },
            ty,
            representation: crate::mir::ValueRepresentation::Borrow,
        })
    }

    pub(super) fn lower_protocol_receiver(
        &mut self,
        method: &crate::typecheck::TypecheckProtocolMethod,
        expression: &Expr,
        scope: ScopeId,
        origin: crate::mir::Origin,
    ) -> Result<CallArgument, BuildError> {
        if method.receiver_mode == crate::ast::MethodReceiverMode::Owned {
            return self.lower_call_argument(expression, scope);
        }
        let readwrite = method.receiver_mode == crate::ast::MethodReceiverMode::ReadwriteBorrow;
        let receiver_type = crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
            span: expression.span(),
            is_readwrite: readwrite,
            inner: Box::new(method.self_ty.clone()),
        });
        let ty = self
            .semantic
            .typed_hir
            .type_id(&receiver_type)
            .ok_or(BuildError::MissingMethodReceiverType)?;
        if super::source_model::expression_value_type(expression, self.semantic)
            .and_then(|source_ty| self.semantic.typed_hir.type_expr_by_id(source_ty))
            .is_some_and(|source_ty| {
                crate::ast::canonical_type_expr(source_ty)
                    == crate::ast::canonical_type_expr(&receiver_type)
            })
        {
            return self.lower_call_argument(expression, scope);
        }
        if let Some(kind) =
            super::source_model::view_kind_for_type_expr(&receiver_type, self.semantic)
        {
            return Ok(CallArgument {
                operand: self.lower_view_operand(expression, ty, kind, scope)?,
                ty,
                representation: crate::mir::ValueRepresentation::View(kind),
            });
        }
        let local = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::Local::borrow(
            ty,
            readwrite,
            LocalStorage::Local,
            LocalOrigin::Desugared(expression.span()),
            scope,
        ));
        if matches!(expression.without_groups(), Expr::Borrow(_)) {
            super::borrows::lower_to_local(
                self,
                local,
                expression,
                readwrite,
                scope,
                crate::mir::LoanLifetime::Call,
            )?;
        } else {
            super::borrows::lower_implicit_to_local(
                self, local, expression, readwrite, scope, origin,
            )?;
        }
        Ok(CallArgument {
            operand: if readwrite {
                Operand::Move(Place::local(local))
            } else {
                Operand::Copy(Place::local(local))
            },
            ty,
            representation: crate::mir::ValueRepresentation::Borrow,
        })
    }

    fn lower_callable_receiver(
        &mut self,
        call: &crate::ast::CallExpr,
        expression: &Expr,
        receiver_ty: crate::semantic::TyId,
        capability: crate::ast::CallableCapability,
        scope: ScopeId,
    ) -> Result<CallArgument, BuildError> {
        if capability == crate::ast::CallableCapability::Consuming {
            let ty = known_expression_type(expression, self.semantic.typed_hir)
                .ok_or(BuildError::MissingTypedExpression)?;
            let Operand::Copy(place) = self.lower_stored_identifier(expression)? else {
                return Err(BuildError::UnsupportedClaimedExpression);
            };
            return Ok(CallArgument {
                operand: Operand::Move(place),
                ty,
                representation: crate::mir::ValueRepresentation::Aggregate,
            });
        }
        let readwrite = capability == crate::ast::CallableCapability::Readwrite;
        let source = self
            .semantic
            .typed_hir
            .expression(call.span)
            .ok_or(BuildError::MissingTypedExpression)?
            .id;
        let local = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::Local::borrow(
            receiver_ty,
            readwrite,
            LocalStorage::Local,
            LocalOrigin::Temporary(source),
            scope,
        ));
        super::borrows::lower_implicit_to_local(
            self,
            local,
            expression,
            readwrite,
            scope,
            crate::mir::Origin::Expression(source),
        )?;
        Ok(CallArgument {
            operand: if readwrite {
                Operand::Move(Place::local(local))
            } else {
                Operand::Copy(Place::local(local))
            },
            ty: receiver_ty,
            representation: crate::mir::ValueRepresentation::Borrow,
        })
    }

    pub(super) fn lower_aggregate_operand(
        &mut self,
        expression: &Expr,
        scope: ScopeId,
    ) -> Result<Operand, BuildError> {
        let authored_expression = expression;
        let (expression, moved) = match expression.without_groups() {
            Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
                (unary.operand.without_groups(), true)
            }
            expression => (expression, false),
        };
        if !super::source_model::aggregate_operand_is_supported(
            authored_expression,
            self.semantic.resolved,
            self.semantic.resolved_sources,
            self.semantic.typed_hir,
        ) && !matches!(expression, Expr::Index(index) if super::indexes::is_supported(index, self.semantic))
        {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        let place = match expression {
            Expr::Identifier(_) => {
                let Operand::Copy(place) = self.lower_stored_identifier(expression)? else {
                    unreachable!("stored identifiers lower to copy places")
                };
                place
            }
            Expr::Member(member) => {
                let (place, representation) = super::projections::lower_field_place(
                    member,
                    self.semantic,
                    &self.places_by_symbol,
                    &mut self.projections,
                    &mut self.drop_plans,
                )?;
                if representation != crate::mir::ValueRepresentation::Aggregate {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                let local = &self.locals[place.local.index()];
                if let Some(plan) = local.drop_plan {
                    super::projections::ensure_owned_drop_projections(
                        place.local,
                        local.ty,
                        plan,
                        self.semantic,
                        &mut self.projections,
                        &self.drop_plans,
                    )?;
                }
                place
            }
            Expr::Index(index) => {
                let (place, representation) = super::indexes::lower_place(self, index, scope)?;
                if representation != crate::mir::ValueRepresentation::Aggregate {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                if place.projection.is_some_and(|projection| {
                    matches!(
                        self.projections[projection.index()].element,
                        crate::mir::ProjectionElement::ViewIndex { .. }
                    ) && self.projections[projection.index()].ownership
                        == crate::mir::OwnershipKind::Move
                }) {
                    return Err(BuildError::UnsupportedSource {
                        span: index.span,
                        construct: "slice indexing outside scalar, `&str`, and copy aggregate elements",
                        help: "add a `copy` element constraint or use an owned iterator to transfer move-only elements",
                    });
                }
                place
            }
            _ => return Err(BuildError::UnsupportedClaimedExpression),
        };
        Ok(
            if moved
                && self.locals[place.local.index()].ownership == crate::mir::OwnershipKind::Move
            {
                Operand::Move(place)
            } else {
                Operand::Copy(place)
            },
        )
    }

    fn lower_stored_identifier(&self, expression: &Expr) -> Result<Operand, BuildError> {
        let Expr::Identifier(identifier) = expression.without_groups() else {
            return Err(BuildError::UnsupportedClaimedExpression);
        };
        let symbol = self
            .semantic
            .resolved
            .local_symbol_for_identifier(identifier)
            .ok_or(BuildError::MissingLocalSymbol)?;
        let place = *self
            .places_by_symbol
            .get(&symbol.id)
            .ok_or(BuildError::MissingLocalSymbol)?;
        Ok(Operand::Copy(place))
    }

    pub(super) fn lower_expression_to_place(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        ty: crate::semantic::TyId,
        scalar: ScalarType,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        if let Expr::Call(call) = expression.without_groups()
            && let Some(operand) = super::literal_packs::length_operand(self, call)
        {
            return self.control_flow.push_statement(Statement::Assign {
                destination: Place::local(destination),
                value: Rvalue::Use(operand),
                origin: crate::mir::Origin::Desugared(call.span),
            });
        }
        let source = self
            .semantic
            .typed_hir
            .expression(expression.span())
            .map(|expression| expression.id);
        if let Expr::Binary(binary) = expression
            && matches!(
                binary.operator,
                crate::ast::BinaryOperator::LogicalAnd | crate::ast::BinaryOperator::LogicalOr
            )
        {
            return self.lower_short_circuit_to_place(
                destination,
                binary,
                ty,
                scope,
                source.ok_or(BuildError::MissingTypedExpression)?,
            );
        }
        if let Expr::Binary(binary) = expression
            && self
                .semantic
                .comparison_plan(binary.span)
                .is_some_and(|plan| plan.method.is_some())
        {
            return self
                .lower_declared_comparison_to_place(
                    destination,
                    binary,
                    ty,
                    scope,
                    source.ok_or(BuildError::MissingTypedExpression)?,
                )
                .map_err(|error| error.context("lower declared comparison"));
        }
        let value = match expression {
            Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
                Rvalue::Use(self.lower_operand(&unary.operand, ty, scalar, scope)?)
            }
            Expr::Unary(unary) => Rvalue::Unary {
                operator: match unary.operator {
                    crate::ast::UnaryOperator::Negate => UnaryOperator::Negate,
                    crate::ast::UnaryOperator::LogicalNot => UnaryOperator::LogicalNot,
                    crate::ast::UnaryOperator::Move => unreachable!("handled above"),
                    crate::ast::UnaryOperator::Spread => {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                },
                operand: self.lower_operand(&unary.operand, ty, scalar, scope)?,
                ty,
            },
            Expr::TypeConversion(conversion) => {
                let checked_literal_conversion = self
                    .semantic
                    .typed_hir
                    .conversion_plan(conversion.span)
                    .is_some_and(|plan| {
                        plan.kind == crate::typecheck::TypecheckConversionKind::LosslessInteger
                    });
                if checked_literal_conversion
                    && let Expr::IntegerLiteral(literal) = conversion.expression.without_groups()
                {
                    Rvalue::Use(Operand::Constant(crate::mir::model::Constant {
                        ty,
                        scalar,
                        value: decode_integer_literal_value(&literal.value)
                            .ok_or(BuildError::InvalidScalarConstant)?,
                    }))
                } else if checked_literal_conversion
                    && let Expr::Unary(unary) = conversion.expression.without_groups()
                    && unary.operator == crate::ast::UnaryOperator::Negate
                    && let Expr::IntegerLiteral(literal) = unary.operand.without_groups()
                {
                    Rvalue::Unary {
                        operator: UnaryOperator::Negate,
                        operand: Operand::Constant(crate::mir::model::Constant {
                            ty,
                            scalar,
                            value: decode_integer_literal_value(&literal.value)
                                .ok_or(BuildError::InvalidScalarConstant)?,
                        }),
                        ty,
                    }
                } else {
                    let source_ty = self
                        .semantic
                        .typed_hir
                        .conversion_plan(conversion.span)
                        .and_then(|plan| self.semantic.typed_hir.type_id(&plan.source_ty))
                        .or_else(|| {
                            super::source_model::expression_value_type(
                                &conversion.expression,
                                self.semantic,
                            )
                        })
                        .ok_or(BuildError::MissingTypedExpression)
                        .map_err(|error| error.context("resolve conversion source type"))?;
                    let source_scalar = scalar_type(source_ty, self.semantic.typed_hir)
                        .ok_or(BuildError::UnsupportedClaimedExpression)
                        .map_err(|error| error.context("resolve conversion source scalar"))?;
                    Rvalue::Cast {
                        operand: self
                            .lower_operand(&conversion.expression, source_ty, source_scalar, scope)
                            .map_err(|error| error.context("lower conversion source"))?,
                        source_ty,
                        source_scalar,
                        target_ty: ty,
                        target_scalar: scalar,
                    }
                }
            }
            Expr::Binary(binary) => {
                if super::source_model::view_comparison_is_supported(binary, self.semantic) {
                    let operator = mir_comparison_operator(binary.operator)
                        .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    let left_ty = known_expression_type(&binary.left, self.semantic.typed_hir)
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let right_ty = known_expression_type(&binary.right, self.semantic.typed_hir)
                        .ok_or(BuildError::MissingTypedExpression)?;
                    Rvalue::ViewCompare {
                        operator,
                        left: self.lower_view_operand(
                            &binary.left,
                            left_ty,
                            crate::mir::ViewKind::Str,
                            scope,
                        )?,
                        right: self.lower_view_operand(
                            &binary.right,
                            right_ty,
                            crate::mir::ViewKind::Str,
                            scope,
                        )?,
                        kind: crate::mir::ViewKind::Str,
                        result_ty: ty,
                    }
                } else if let Some(operator) = mir_binary_operator(binary.operator) {
                    Rvalue::Binary {
                        operator,
                        left: self.lower_operand(&binary.left, ty, scalar, scope)?,
                        right: self.lower_operand(&binary.right, ty, scalar, scope)?,
                        ty,
                    }
                } else {
                    let operator = mir_comparison_operator(binary.operator)
                        .ok_or(BuildError::UnsupportedClaimedExpression)
                        .map_err(|error| error.context("resolve builtin comparison operator"))?;
                    let (operand_ty, operand_scalar) =
                        super::source_model::comparison_operand_type(binary, self.semantic)
                            .ok_or(BuildError::UnsupportedClaimedExpression)
                            .map_err(|error| {
                                error.context("resolve builtin comparison operand type")
                            })?;
                    Rvalue::Compare {
                        operator,
                        left: self
                            .lower_operand(&binary.left, operand_ty, operand_scalar, scope)
                            .map_err(|error| error.context("lower comparison left operand"))?,
                        right: self
                            .lower_operand(&binary.right, operand_ty, operand_scalar, scope)
                            .map_err(|error| error.context("lower comparison right operand"))?,
                        operand_ty,
                        operand_scalar,
                        result_ty: ty,
                    }
                }
            }
            Expr::Group(group) => {
                return self
                    .lower_expression_to_place(destination, &group.expression, ty, scalar, scope)
                    .map_err(|error| error.context("lower grouped scalar expression"));
            }
            Expr::If(if_) => {
                return lower_conditional_to_place(
                    self,
                    destination,
                    if_,
                    ty,
                    crate::mir::ValueRepresentation::Scalar(scalar),
                    scope,
                );
            }
            Expr::IfIs(if_is) => {
                return lower_if_is_to_place(
                    self,
                    if_is,
                    destination,
                    ty,
                    crate::mir::ValueRepresentation::Scalar(scalar),
                    scope,
                    false,
                );
            }
            Expr::Match(match_) => {
                return lower_match_to_place(
                    self,
                    match_,
                    destination,
                    ty,
                    crate::mir::ValueRepresentation::Scalar(scalar),
                    scope,
                    false,
                );
            }
            Expr::Otherwise(otherwise) => {
                return outcomes::lower_otherwise_to_place(
                    self,
                    destination,
                    otherwise,
                    ty,
                    scalar,
                    scope,
                );
            }
            Expr::Catch(catch) => {
                return outcomes::lower_catch_to_place(self, destination, catch, ty, scalar, scope);
            }
            Expr::Call(call) => {
                let (callee, arguments, returns_never) = self
                    .lower_call(call, scope)
                    .map_err(|error| error.context("lower scalar call"))?;
                if returns_never {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                return self.control_flow.emit_returning_call(
                    source.ok_or(BuildError::MissingTypedExpression)?,
                    callee,
                    arguments,
                    destination,
                );
            }
            Expr::Force(force) => {
                if let Expr::Call(call) = force.expression.without_groups()
                    && super::source_model::call_has_single_outcome_layer(call, self.semantic)
                {
                    let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                    if returns_never {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    return self.control_flow.emit_trapping_outcome_call(
                        source.ok_or(BuildError::MissingTypedExpression)?,
                        callee,
                        arguments,
                        destination,
                    );
                }
                return outcomes::lower_terminal_stored_outcome_to_place(
                    self,
                    destination,
                    &force.expression,
                    crate::mir::Terminator::Trap,
                    scope,
                );
            }
            Expr::Propagate(propagate) => {
                if let Expr::Call(call) = propagate.expression.without_groups()
                    && super::source_model::call_has_single_outcome_layer(call, self.semantic)
                {
                    let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                    if returns_never {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    return self.control_flow.emit_propagating_outcome_call(
                        source.ok_or(BuildError::MissingTypedExpression)?,
                        callee,
                        arguments,
                        destination,
                    );
                }
                return outcomes::lower_terminal_stored_outcome_to_place(
                    self,
                    destination,
                    &propagate.expression,
                    crate::mir::Terminator::PropagateFailure,
                    scope,
                );
            }
            Expr::Member(member) => {
                if let Some(tag) =
                    super::source_model::payloadless_enum_variant_tag(member, self.semantic)
                {
                    Rvalue::Use(Operand::Constant(crate::mir::model::Constant {
                        ty,
                        scalar,
                        value: u128::from(tag),
                    }))
                } else {
                    let (place, field_scalar) =
                        if super::projections::scalar_field_is_supported(member, self.semantic) {
                            let (place, representation) =
                                super::projections::lower_borrow_field_place(
                                    member,
                                    self.semantic,
                                    &self.places_by_symbol,
                                    &mut self.projections,
                                    &mut self.drop_plans,
                                )?;
                            let crate::mir::ValueRepresentation::Scalar(field_scalar) =
                                representation
                            else {
                                return Err(BuildError::UnsupportedClaimedExpression);
                            };
                            (place, field_scalar)
                        } else {
                            (
                                self.lower_value_member_source(
                                    member,
                                    ty,
                                    crate::mir::ValueRepresentation::Scalar(scalar),
                                    scope,
                                )?,
                                scalar,
                            )
                        };
                    if field_scalar != scalar {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    Rvalue::Use(Operand::Copy(place))
                }
            }
            Expr::Index(index) => {
                let (place, representation) = super::indexes::lower_place(self, index, scope)
                    .map_err(|error| error.context("lower scalar index expression"))?;
                if representation != crate::mir::ValueRepresentation::Scalar(scalar) {
                    return Err(BuildError::UnsupportedClaimedExpression
                        .context("match scalar index result representation"));
                }
                Rvalue::Use(Operand::Copy(place))
            }
            _ => Rvalue::Use(
                self.lower_simple_operand(expression, ty, scalar)
                    .map_err(|error| error.context("lower simple scalar operand"))?,
            ),
        };
        self.control_flow
            .push_statement(Statement::Assign {
                destination: Place::local(destination),
                value,
                origin: source.map_or(crate::mir::Origin::Desugared(expression.span()), |source| {
                    crate::mir::Origin::Expression(source)
                }),
            })
            .map_err(|error| error.context("assign scalar expression result"))?;
        Ok(())
    }

    fn lower_declared_comparison_to_place(
        &mut self,
        destination: LocalId,
        binary: &crate::ast::BinaryExpr,
        ty: crate::semantic::TyId,
        scope: ScopeId,
        source: crate::semantic::ExprId,
    ) -> Result<(), BuildError> {
        let plan = self
            .semantic
            .comparison_plan(binary.span)
            .ok_or(BuildError::MissingCallTarget)?;
        let method = plan.method.ok_or(BuildError::MissingCallTarget)?;
        let definition = self
            .semantic
            .resolved
            .callable_bodies
            .canonical_definition(method.def_id);
        let receiver = self
            .semantic
            .typed_hir
            .type_id(&method.self_ty)
            .ok_or(BuildError::MissingSpecializedReceiverType)?;
        // Preserve authored left-to-right evaluation even when `>` selects a
        // reversed strict-order implementation.  Operand adjustment happens
        // before the completed arguments are reordered for the call ABI.
        let mut arguments = vec![
            self.lower_comparison_operand(
                &binary.left,
                plan.left_conversion.as_ref(),
                !plan.reverse_operands,
                &method.self_ty,
                scope,
            )
            .map_err(|error| error.context("lower comparison left operand"))?,
            self.lower_comparison_operand(
                &binary.right,
                plan.right_conversion.as_ref(),
                plan.reverse_operands || plan.right_implicit_readonly_borrow,
                &method.self_ty,
                scope,
            )
            .map_err(|error| error.context("lower comparison right operand"))?,
        ];
        if plan.reverse_operands {
            arguments.swap(0, 1);
        }
        let call_destination = if plan.invert_result {
            let temporary = LocalId::from_index(self.locals.len());
            self.locals.push(crate::mir::Local::scalar(
                ty,
                ScalarType::Bool,
                LocalStorage::Local,
                LocalOrigin::Temporary(source),
                scope,
            ));
            temporary
        } else {
            destination
        };
        self.control_flow.emit_returning_call(
            source,
            crate::mir::CallInstance::specialized(definition, Some(receiver), Vec::new()),
            arguments,
            call_destination,
        )?;
        if plan.invert_result {
            self.control_flow.push_statement(Statement::Assign {
                destination: Place::local(destination),
                value: Rvalue::Unary {
                    operator: UnaryOperator::LogicalNot,
                    operand: Operand::Copy(Place::local(call_destination)),
                    ty,
                },
                origin: crate::mir::Origin::Expression(source),
            })?;
        }
        Ok(())
    }

    fn lower_comparison_operand(
        &mut self,
        expression: &Expr,
        conversion: Option<&crate::typecheck::TypecheckConversionPlan>,
        implicit_readonly_borrow: bool,
        receiver_ty: &crate::ast::TypeExpr,
        scope: ScopeId,
    ) -> Result<CallArgument, BuildError> {
        if let Some(conversion) = conversion {
            let ty = self
                .semantic
                .typed_hir
                .type_id(&conversion.target_ty)
                .ok_or(BuildError::MissingTypedExpression)?;
            let local =
                self.local_for_type(ty, LocalOrigin::Desugared(expression.span()), scope)?;
            self.lower_planned_coercion_to_local(local, expression, conversion, scope)?;
            let representation = value_representation(ty, self.semantic)
                .ok_or(BuildError::MissingTypedExpression)?;
            return Ok(CallArgument {
                operand: Operand::Copy(Place::local(local)),
                ty,
                representation,
            });
        }
        let source_representation = match expression.without_groups() {
            Expr::Identifier(identifier) => self
                .semantic
                .resolved
                .local_symbol_for_identifier(identifier)
                .and_then(|symbol| self.semantic.typed_hir.binding_type_expr(symbol.id))
                .and_then(|ty| self.semantic.typed_hir.type_id(ty))
                .and_then(|ty| value_representation(ty, self.semantic)),
            _ => super::source_model::intrinsic_expression_type(
                expression.span(),
                self.semantic.typed_hir,
            )
            .or_else(|| known_expression_type(expression, self.semantic.typed_hir))
            .and_then(|ty| value_representation(ty, self.semantic)),
        };
        if (implicit_readonly_borrow
            || source_representation == Some(crate::mir::ValueRepresentation::Aggregate))
            && source_representation != Some(crate::mir::ValueRepresentation::Borrow)
            && !matches!(
                source_representation,
                Some(crate::mir::ValueRepresentation::View(_))
            )
        {
            let origin = crate::mir::Origin::Desugared(expression.span());
            return if super::borrows::source_place_is_supported(expression, self.semantic) {
                let source = super::borrows::lower_source_place(self, expression, scope)?;
                super::borrows::place_argument(self, source, receiver_ty, false, scope, origin)
            } else {
                super::borrows::expression_argument(
                    self,
                    expression,
                    receiver_ty,
                    false,
                    scope,
                    origin,
                )
            };
        }
        self.lower_call_argument(expression, scope)
    }

    pub(super) fn lower_view_expression_to_place(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        ty: crate::semantic::TyId,
        kind: crate::mir::ViewKind,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        if self.lower_coercion_to_local(destination, expression, scope)? {
            return Ok(());
        }
        let source = self
            .semantic
            .typed_hir
            .expression(expression.span())
            .ok_or(BuildError::MissingTypedExpression)?
            .id;
        let source_ty = super::source_model::intrinsic_expression_type(
            expression.span(),
            self.semantic.typed_hir,
        )
        .filter(|source_ty| {
            super::source_model::value_representation(*source_ty, self.semantic)
                == Some(crate::mir::ValueRepresentation::View(kind))
        })
        .unwrap_or(ty);
        match expression.without_groups() {
            Expr::If(if_) => {
                return lower_conditional_to_place(
                    self,
                    destination,
                    if_,
                    ty,
                    crate::mir::ValueRepresentation::View(kind),
                    scope,
                );
            }
            Expr::IfIs(if_is) => {
                return lower_if_is_to_place(
                    self,
                    if_is,
                    destination,
                    ty,
                    crate::mir::ValueRepresentation::View(kind),
                    scope,
                    false,
                );
            }
            Expr::Match(match_) => {
                return lower_match_to_place(
                    self,
                    match_,
                    destination,
                    ty,
                    crate::mir::ValueRepresentation::View(kind),
                    scope,
                    false,
                );
            }
            Expr::Otherwise(otherwise) => {
                return outcomes::lower_view_otherwise_to_place(
                    self,
                    destination,
                    otherwise,
                    ty,
                    kind,
                    scope,
                );
            }
            Expr::Catch(catch) => {
                return outcomes::lower_view_catch_to_place(
                    self,
                    destination,
                    catch,
                    ty,
                    kind,
                    scope,
                );
            }
            Expr::Force(force) => {
                if let Expr::Call(call) = force.expression.without_groups()
                    && super::source_model::call_has_single_outcome_layer(call, self.semantic)
                {
                    let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                    if returns_never {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    return self.control_flow.emit_trapping_outcome_call(
                        source,
                        callee,
                        arguments,
                        destination,
                    );
                }
                return outcomes::lower_terminal_stored_outcome_to_place(
                    self,
                    destination,
                    &force.expression,
                    crate::mir::Terminator::Trap,
                    scope,
                );
            }
            Expr::Propagate(propagate) => {
                if let Expr::Call(call) = propagate.expression.without_groups()
                    && super::source_model::call_has_single_outcome_layer(call, self.semantic)
                {
                    let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                    if returns_never {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                    return self.control_flow.emit_propagating_outcome_call(
                        source,
                        callee,
                        arguments,
                        destination,
                    );
                }
                return outcomes::lower_terminal_stored_outcome_to_place(
                    self,
                    destination,
                    &propagate.expression,
                    crate::mir::Terminator::PropagateFailure,
                    scope,
                );
            }
            _ => {}
        }
        if let Expr::Call(call) = expression.without_groups() {
            let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
            if returns_never {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            if source_ty == ty {
                return self.control_flow.emit_returning_call(
                    source,
                    callee,
                    arguments,
                    destination,
                );
            }
            let source_local = LocalId::from_index(self.locals.len());
            self.locals.push(crate::mir::Local::view(
                source_ty,
                kind,
                LocalStorage::Local,
                LocalOrigin::Temporary(source),
                scope,
            ));
            self.control_flow
                .emit_returning_call(source, callee, arguments, source_local)?;
            return self.control_flow.push_statement(Statement::Assign {
                destination: Place::local(destination),
                value: Rvalue::ViewCast {
                    source: Operand::Copy(Place::local(source_local)),
                    source_ty,
                    target_ty: ty,
                    kind,
                },
                origin: crate::mir::Origin::Expression(source),
            });
        }
        let operand = self.lower_view_operand(expression, source_ty, kind, scope)?;
        let operand_ty = match &operand {
            Operand::Copy(place) | Operand::Move(place) => place.projection.map_or_else(
                || self.locals.get(place.local.index()).map(|local| local.ty),
                |projection| self.projections.get(projection.index()).map(|path| path.ty),
            ),
            Operand::Constant(constant) => Some(constant.ty),
            Operand::StaticStr { ty, .. } => Some(*ty),
        }
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let value = if operand_ty == ty {
            Rvalue::Use(operand)
        } else {
            Rvalue::ViewCast {
                source: operand,
                source_ty: operand_ty,
                target_ty: ty,
                kind,
            }
        };
        self.control_flow.push_statement(Statement::Assign {
            destination: Place::local(destination),
            value,
            origin: crate::mir::Origin::Expression(source),
        })
    }

    pub(super) fn lower_view_operand(
        &mut self,
        expression: &Expr,
        ty: crate::semantic::TyId,
        kind: crate::mir::ViewKind,
        scope: ScopeId,
    ) -> Result<Operand, BuildError> {
        if self
            .semantic
            .typed_hir
            .coercion_plan(expression.span())
            .is_some()
        {
            let origin = self
                .semantic
                .typed_hir
                .expression(expression.span())
                .ok_or(BuildError::MissingTypedExpression)?
                .id;
            let temporary = LocalId::from_index(self.locals.len());
            self.locals.push(crate::mir::Local::view(
                ty,
                kind,
                LocalStorage::Local,
                LocalOrigin::Temporary(origin),
                scope,
            ));
            if !self.lower_coercion_to_local(temporary, expression, scope)? {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            return Ok(Operand::Copy(Place::local(temporary)));
        }
        match expression {
            Expr::StringLiteral(literal) if kind == crate::mir::ViewKind::Str => {
                let bytes = crate::literals::decode_string_literal_bytes(&literal.value)
                    .map_err(|_| BuildError::InvalidScalarConstant)?;
                Ok(Operand::StaticStr { ty, bytes })
            }
            Expr::Identifier(_) => self.lower_stored_identifier(expression),
            Expr::Member(member)
                if super::projections::field_is_supported(member, self.semantic) =>
            {
                let (place, representation) = super::projections::lower_borrow_field_place(
                    member,
                    self.semantic,
                    &self.places_by_symbol,
                    &mut self.projections,
                    &mut self.drop_plans,
                )?;
                if representation != crate::mir::ValueRepresentation::View(kind) {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(member.span)
                    .ok_or(BuildError::MissingTypedExpression)?
                    .id;
                let temporary = LocalId::from_index(self.locals.len());
                self.locals.push(crate::mir::Local::view(
                    ty,
                    kind,
                    LocalStorage::Local,
                    LocalOrigin::Temporary(origin),
                    scope,
                ));
                self.control_flow.push_statement(Statement::Assign {
                    destination: Place::local(temporary),
                    value: Rvalue::Use(Operand::Copy(place)),
                    origin: crate::mir::Origin::Expression(origin),
                })?;
                Ok(Operand::Copy(Place::local(temporary)))
            }
            Expr::Member(member) if kind == crate::mir::ViewKind::Str => {
                super::projections::lower_error_field_place(
                    member,
                    self.semantic,
                    &self.places_by_symbol,
                    &mut self.projections,
                )
                .map(Operand::Copy)
            }
            Expr::Index(index) if super::indexes::is_supported(index, self.semantic) => {
                let (place, representation) = super::indexes::lower_place(self, index, scope)?;
                if representation != crate::mir::ValueRepresentation::View(kind) {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                Ok(Operand::Copy(place))
            }
            Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
                // Views carry no owned storage. `move` is accepted uniformly
                // by the source language, but remains a transparent copy at
                // the MIR ABI boundary.
                self.lower_view_operand(&unary.operand, ty, kind, scope)
            }
            Expr::Group(group) => self.lower_view_operand(&group.expression, ty, kind, scope),
            Expr::Call(_)
            | Expr::Force(_)
            | Expr::Propagate(_)
            | Expr::Otherwise(_)
            | Expr::Catch(_) => {
                let typed_expression = self
                    .semantic
                    .typed_hir
                    .expression(expression.span())
                    .ok_or(BuildError::MissingTypedExpression)?;
                let temporary = LocalId::from_index(self.locals.len());
                self.locals.push(crate::mir::Local::view(
                    ty,
                    kind,
                    LocalStorage::Local,
                    LocalOrigin::Temporary(typed_expression.id),
                    scope,
                ));
                self.lower_value_to_place(
                    temporary,
                    expression,
                    ty,
                    crate::mir::ValueRepresentation::View(kind),
                    scope,
                )?;
                Ok(Operand::Copy(Place::local(temporary)))
            }
            _ => Err(BuildError::UnsupportedClaimedExpression),
        }
    }

    fn lower_short_circuit_to_place(
        &mut self,
        destination: LocalId,
        binary: &crate::ast::BinaryExpr,
        ty: crate::semantic::TyId,
        scope: ScopeId,
        source: crate::semantic::ExprId,
    ) -> Result<(), BuildError> {
        let left = self
            .lower_operand(&binary.left, ty, ScalarType::Bool, scope)
            .map_err(|error| error.context("lower short-circuit left operand"))?;
        let right_target = self.control_flow.reserve_block(scope);
        let short_target = self.control_flow.reserve_block(scope);
        let join_target = self.control_flow.reserve_block(scope);
        let (then_target, else_target, short_value) = match binary.operator {
            crate::ast::BinaryOperator::LogicalAnd => (right_target, short_target, 0),
            crate::ast::BinaryOperator::LogicalOr => (short_target, right_target, 1),
            _ => return Err(BuildError::UnsupportedClaimedExpression),
        };
        self.control_flow
            .terminate(crate::mir::Terminator::Switch {
                condition: left,
                then_target,
                else_target,
                join_target: Some(join_target),
            })?;

        self.control_flow.select_block(short_target)?;
        self.control_flow.push_statement(Statement::Assign {
            destination: Place::local(destination),
            value: Rvalue::Use(Operand::Constant(crate::mir::Constant {
                ty,
                scalar: ScalarType::Bool,
                value: short_value,
            })),
            origin: crate::mir::Origin::Expression(source),
        })?;
        self.control_flow.terminate(crate::mir::Terminator::Goto {
            target: join_target,
        })?;

        self.control_flow.select_block(right_target)?;
        self.lower_expression_to_place(destination, &binary.right, ty, ScalarType::Bool, scope)
            .map_err(|error| error.context("lower short-circuit right operand"))?;
        self.control_flow.terminate(crate::mir::Terminator::Goto {
            target: join_target,
        })?;
        self.control_flow.select_block(join_target)
    }

    pub(super) fn lower_operand(
        &mut self,
        expression: &Expr,
        ty: crate::semantic::TyId,
        scalar: ScalarType,
        scope: ScopeId,
    ) -> Result<Operand, BuildError> {
        let projected_identifier = match expression.without_groups() {
            Expr::Identifier(identifier) => self
                .semantic
                .resolved
                .local_symbol_for_identifier(identifier)
                .and_then(|symbol| self.places_by_symbol.get(&symbol.id))
                .is_some_and(|place| {
                    place.projection.is_some()
                        || self.locals[place.local.index()].representation
                            == crate::mir::ValueRepresentation::Borrow
                }),
            _ => false,
        };
        if !projected_identifier
            && !matches!(
                expression,
                Expr::Unary(_)
                    | Expr::TypeConversion(_)
                    | Expr::Binary(_)
                    | Expr::Call(_)
                    | Expr::Force(_)
                    | Expr::Propagate(_)
                    | Expr::Member(_)
                    | Expr::Index(_)
                    | Expr::If(_)
                    | Expr::IfIs(_)
                    | Expr::Match(_)
                    | Expr::Otherwise(_)
                    | Expr::Catch(_)
            )
        {
            return match expression {
                Expr::Group(group) => self.lower_operand(&group.expression, ty, scalar, scope),
                _ => self.lower_simple_operand(expression, ty, scalar),
            };
        }

        let origin = self
            .semantic
            .typed_hir
            .expression(expression.span())
            .map_or(LocalOrigin::Desugared(expression.span()), |expression| {
                LocalOrigin::Temporary(expression.id)
            });
        let temporary = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::locals::Local::scalar(
            ty,
            scalar,
            LocalStorage::Local,
            origin,
            scope,
        ));
        self.lower_value_to_place(
            temporary,
            expression,
            ty,
            crate::mir::ValueRepresentation::Scalar(scalar),
            scope,
        )?;
        Ok(Operand::Copy(Place::local(temporary)))
    }

    fn lower_simple_operand(
        &mut self,
        expression: &Expr,
        ty: crate::semantic::TyId,
        scalar: ScalarType,
    ) -> Result<Operand, BuildError> {
        match expression {
            Expr::IntegerLiteral(literal) => Ok(Operand::Constant(crate::mir::model::Constant {
                ty,
                scalar,
                value: decode_integer_literal_value(&literal.value)
                    .ok_or(BuildError::InvalidScalarConstant)?,
            })),
            Expr::ByteLiteral(literal) => Ok(Operand::Constant(crate::mir::model::Constant {
                ty,
                scalar,
                value: u128::from(
                    crate::literals::decode_byte_literal(&literal.value)
                        .map_err(|_| BuildError::InvalidScalarConstant)?,
                ),
            })),
            Expr::BoolLiteral(literal) => Ok(Operand::Constant(crate::mir::model::Constant {
                ty,
                scalar,
                value: match literal.value.as_str() {
                    "false" => 0,
                    "true" => 1,
                    _ => return Err(BuildError::InvalidScalarConstant),
                },
            })),
            Expr::Identifier(identifier) => {
                let symbol = self
                    .semantic
                    .resolved
                    .local_symbol_for_identifier(identifier)
                    .map(|symbol| symbol.id)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let place = *self
                    .places_by_symbol
                    .get(&symbol)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                if place.projection.is_none()
                    && self.locals[place.local.index()].representation
                        == crate::mir::ValueRepresentation::Borrow
                {
                    let projection =
                        crate::mir::ProjectionPathId::from_index(self.projections.len());
                    self.projections.push(crate::mir::ProjectionPath {
                        id: projection,
                        base: place.local,
                        parent: None,
                        element: crate::mir::ProjectionElement::Dereference,
                        ty,
                        representation: crate::mir::ValueRepresentation::Scalar(scalar),
                        ownership: crate::mir::OwnershipKind::Copy,
                        drop_plan: None,
                    });
                    return Ok(Operand::Copy(Place::projected(place.local, projection)));
                }
                Ok(Operand::Copy(place))
            }
            _ => Err(BuildError::UnsupportedClaimedExpression),
        }
    }
}

fn coercion_source_expression(expression: &Expr) -> &Expr {
    let expression = expression.without_groups();
    match expression {
        Expr::TypeConversion(conversion) => conversion.expression.without_groups(),
        expression => expression,
    }
}

pub(super) fn mir_binary_operator(operator: crate::ast::BinaryOperator) -> Option<BinaryOperator> {
    match operator {
        crate::ast::BinaryOperator::Add => Some(BinaryOperator::Add),
        crate::ast::BinaryOperator::Subtract => Some(BinaryOperator::Subtract),
        crate::ast::BinaryOperator::Multiply => Some(BinaryOperator::Multiply),
        crate::ast::BinaryOperator::Divide => Some(BinaryOperator::Divide),
        crate::ast::BinaryOperator::Remainder => Some(BinaryOperator::Remainder),
        crate::ast::BinaryOperator::ShiftLeft => Some(BinaryOperator::ShiftLeft),
        crate::ast::BinaryOperator::ShiftRight => Some(BinaryOperator::ShiftRight),
        _ => None,
    }
}

pub(super) fn mir_assignment_operator(
    operator: crate::ast::AssignmentOperator,
) -> Option<BinaryOperator> {
    match operator {
        crate::ast::AssignmentOperator::AddAssign => Some(BinaryOperator::Add),
        crate::ast::AssignmentOperator::SubtractAssign => Some(BinaryOperator::Subtract),
        crate::ast::AssignmentOperator::MultiplyAssign => Some(BinaryOperator::Multiply),
        crate::ast::AssignmentOperator::DivideAssign => Some(BinaryOperator::Divide),
        crate::ast::AssignmentOperator::RemainderAssign => Some(BinaryOperator::Remainder),
        crate::ast::AssignmentOperator::Assign => None,
    }
}

pub(super) fn mir_comparison_operator(
    operator: crate::ast::BinaryOperator,
) -> Option<ComparisonOperator> {
    match operator {
        crate::ast::BinaryOperator::Equal => Some(ComparisonOperator::Equal),
        crate::ast::BinaryOperator::NotEqual => Some(ComparisonOperator::NotEqual),
        crate::ast::BinaryOperator::Less => Some(ComparisonOperator::Less),
        crate::ast::BinaryOperator::LessEqual => Some(ComparisonOperator::LessEqual),
        crate::ast::BinaryOperator::Greater => Some(ComparisonOperator::Greater),
        crate::ast::BinaryOperator::GreaterEqual => Some(ComparisonOperator::GreaterEqual),
        _ => None,
    }
}
