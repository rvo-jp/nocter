//! Scalar expression evaluation into MIR places, rvalues, and operands.

use super::BuildError;
use super::context::LoweringContext;
use super::coverage::{known_expression_type, scalar_type, value_representation};
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
pub(super) use control_flow_expressions::lower_conditional_to_place;

impl LoweringContext<'_> {
    pub(super) fn lower_intrinsic_effect(
        &mut self,
        call: &crate::ast::CallExpr,
        origin: crate::mir::Origin,
        scope: ScopeId,
    ) -> Result<bool, BuildError> {
        let Some(intrinsic) = super::coverage::intrinsic_for_call(call, self.semantic) else {
            return Ok(false);
        };
        if !super::coverage::effect_intrinsic_is_supported(intrinsic) {
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
        let Some(intrinsic) = super::coverage::intrinsic_for_call(call, self.semantic) else {
            return Ok(None);
        };
        if !super::coverage::value_intrinsic_is_supported(intrinsic) {
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
        let result_ty = self.locals[self.return_local().index()].ty;
        let contract = super::outcome_contract(result_ty, self.semantic)?
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
            crate::mir::ValueRepresentation::View(kind) => {
                self.lower_view_operand(expression, contract.payload_ty, kind, scope)?
            }
            crate::mir::ValueRepresentation::Aggregate => {
                let origin = self
                    .semantic
                    .typed_hir
                    .expression(expression.span())
                    .ok_or(BuildError::MissingTypedExpression)?
                    .id;
                let local = self.aggregate_temporary(
                    contract.payload_ty,
                    LocalOrigin::Temporary(origin),
                    scope,
                )?;
                self.lower_value_to_place(
                    local,
                    expression,
                    contract.payload_ty,
                    contract.payload_representation,
                    scope,
                )?;
                if self.locals[local.index()].ownership == crate::mir::OwnershipKind::Move {
                    Operand::Move(Place::local(local))
                } else {
                    Operand::Copy(Place::local(local))
                }
            }
            crate::mir::ValueRepresentation::Unit
            | crate::mir::ValueRepresentation::Borrow
            | crate::mir::ValueRepresentation::Error => {
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
        if !super::coverage::failure_value_is_supported(expression, self.semantic) {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        let (code, message) = match expression.without_groups() {
            Expr::Call(call) => {
                let mut operands = Vec::with_capacity(2);
                for argument in &call.arguments {
                    let ty = known_expression_type(argument, self.semantic.typed_hir)
                        .ok_or(BuildError::MissingTypedExpression)?;
                    operands.push(self.lower_view_operand(
                        argument,
                        ty,
                        crate::mir::ViewKind::Str,
                        scope,
                    )?);
                }
                let [code, message] = operands
                    .try_into()
                    .map_err(|_| BuildError::UnsupportedClaimedExpression)?;
                (code, message)
            }
            Expr::Identifier(identifier) => {
                let symbol = self
                    .semantic
                    .resolved
                    .local_symbol_for_identifier(identifier)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let place = *self
                    .places_by_symbol
                    .get(&symbol.id)
                    .ok_or(BuildError::MissingLocalSymbol)?;
                let representation = place.projection.map_or(
                    self.locals[place.local.index()].representation,
                    |projection| self.projections[projection.index()].representation,
                );
                if representation != crate::mir::ValueRepresentation::Error {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                let span = expression.span();
                let str_ty = self
                    .semantic
                    .typed_hir
                    .type_id(&crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
                        span,
                        is_readwrite: false,
                        inner: Box::new(crate::ast::TypeExpr::Reference(
                            crate::ast::TypeReference {
                                span,
                                name: "str".to_string(),
                            },
                        )),
                    }))
                    .ok_or(BuildError::MissingTypedExpression)?;
                let code = super::projections::push_error_field_place(
                    place.local,
                    crate::builtin_types::BuiltinErrorField::Code,
                    str_ty,
                    &mut self.projections,
                );
                let message = super::projections::push_error_field_place(
                    place.local,
                    crate::builtin_types::BuiltinErrorField::Message,
                    str_ty,
                    &mut self.projections,
                );
                (Operand::Copy(code), Operand::Copy(message))
            }
            _ => return Err(BuildError::UnsupportedClaimedExpression),
        };
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
            crate::mir::ValueRepresentation::Scalar(scalar) => {
                self.lower_expression_to_place(destination, expression, ty, scalar, scope)
            }
            crate::mir::ValueRepresentation::View(kind) => {
                self.lower_view_expression_to_place(destination, expression, ty, kind, scope)
            }
            crate::mir::ValueRepresentation::Aggregate => match expression.without_groups() {
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
                Expr::Member(member) => {
                    let source = if super::coverage::aggregate_operand_is_supported(
                        expression,
                        self.semantic.resolved,
                        self.semantic.resolved_sources,
                        self.semantic.typed_hir,
                    ) {
                        let Operand::Copy(source) = self.lower_aggregate_operand(expression)?
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
                Expr::Identifier(_) | Expr::Unary(_) => {
                    let source = self
                        .semantic
                        .typed_hir
                        .expression(expression.span())
                        .ok_or(BuildError::MissingTypedExpression)?
                        .id;
                    let operand = self.lower_aggregate_operand(expression)?;
                    self.control_flow.push_statement(Statement::Assign {
                        destination: Place::local(destination),
                        value: Rvalue::Use(operand),
                        origin: crate::mir::Origin::Expression(source),
                    })
                }
                Expr::Force(force) => outcomes::lower_terminal_stored_outcome_to_place(
                    self,
                    destination,
                    &force.expression,
                    crate::mir::Terminator::Trap,
                    scope,
                ),
                Expr::Propagate(propagate) => outcomes::lower_terminal_stored_outcome_to_place(
                    self,
                    destination,
                    &propagate.expression,
                    crate::mir::Terminator::PropagateFailure,
                    scope,
                ),
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
                _ => super::aggregates::lower_literal(self, destination, expression, scope),
            },
            crate::mir::ValueRepresentation::Borrow | crate::mir::ValueRepresentation::Error => {
                Err(BuildError::UnsupportedClaimedExpression)
            }
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
        if super::coverage::value_representation(root_ty, self.semantic)
            != Some(crate::mir::ValueRepresentation::Aggregate)
        {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        let temporary =
            self.aggregate_temporary(root_ty, LocalOrigin::Temporary(root_expression.id), scope)?;
        self.lower_value_to_place(
            temporary,
            root,
            root_ty,
            crate::mir::ValueRepresentation::Aggregate,
            scope,
        )?;
        let (source, representation) = super::projections::lower_field_place_from_value_root(
            member,
            root_ty,
            Place::local(temporary),
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
        let returns_never = self
            .semantic
            .typed_hir
            .expression(call.span)
            .is_some_and(|expression| expression.diverges);
        if let Some(intrinsic) = super::coverage::intrinsic_for_call(call, self.semantic)
            .filter(|intrinsic| super::coverage::outcome_intrinsic_is_supported(*intrinsic))
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
                        let receiver = self
                            .semantic
                            .typed_hir
                            .type_id(&specialization.self_ty)
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
                PlannedReceiver::Method(member) => {
                    self.lower_method_receiver(call, member, scope)?
                }
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
            arguments.push(self.lower_call_argument(argument, scope)?);
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
        let ty = match argument.without_groups() {
            Expr::Call(call) => super::coverage::call_result_type(call, self.semantic),
            _ => known_expression_type(argument, self.semantic.typed_hir),
        }
        .ok_or(BuildError::MissingTypedExpression)?;
        if let Some(scalar) = scalar_type(ty, self.semantic.typed_hir) {
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
            let ty = super::coverage::intrinsic_expression_type(
                argument.span(),
                self.semantic.typed_hir,
            )
            .filter(|ty| value_representation(*ty, self.semantic) == Some(representation))
            .unwrap_or(ty);
            return Ok(CallArgument {
                operand: self.lower_view_operand(argument, ty, kind, scope)?,
                ty,
                representation,
            });
        }
        if super::coverage::borrow_argument_is_supported(argument, self.semantic) {
            let operand = if super::coverage::borrow_identifier_is_supported(
                argument,
                self.semantic.resolved,
                self.semantic.typed_hir,
            ) {
                self.lower_stored_identifier(argument)?
            } else {
                let typed_expression = self
                    .semantic
                    .typed_hir
                    .expression(argument.span())
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
                    argument,
                    borrow_ty.is_readwrite,
                    scope,
                    crate::mir::LoanLifetime::Call,
                )?;
                if borrow_ty.is_readwrite {
                    Operand::Move(Place::local(local))
                } else {
                    Operand::Copy(Place::local(local))
                }
            };
            return Ok(CallArgument {
                operand,
                ty,
                representation: crate::mir::ValueRepresentation::Borrow,
            });
        }
        if super::aggregates::literal_is_supported(argument, self.semantic) {
            let expression = self
                .semantic
                .typed_hir
                .expression(argument.span())
                .ok_or(BuildError::MissingTypedExpression)?;
            let local =
                self.aggregate_temporary(ty, LocalOrigin::Temporary(expression.id), scope)?;
            super::aggregates::lower_literal(self, local, argument, scope)?;
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
        if matches!(argument.without_groups(), Expr::Call(_))
            && value_representation(ty, self.semantic)
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
            && !super::coverage::aggregate_operand_is_supported(
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
        let operand = self.lower_aggregate_operand(argument)?;
        Ok(CallArgument {
            operand,
            ty,
            representation: crate::mir::ValueRepresentation::Aggregate,
        })
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
            return self.lower_call_argument(&member.object, scope);
        }
        let readwrite = kind == crate::typecheck::TypecheckMethodReceiverKind::ReadwriteBorrow;
        let ty = self
            .semantic
            .typed_hir
            .method_call_receiver_type(member.member_span)
            .ok_or(BuildError::MissingMethodReceiverType)?;
        if let Some(representation @ crate::mir::ValueRepresentation::View(view)) =
            value_representation(ty, self.semantic)
        {
            let source_ty = known_expression_type(&member.object, self.semantic.typed_hir)
                .ok_or(BuildError::MissingTypedExpression)?;
            let source = self.lower_view_operand(&member.object, source_ty, view, scope)?;
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
        )?;
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
    ) -> Result<Operand, BuildError> {
        if !super::coverage::aggregate_operand_is_supported(
            expression,
            self.semantic.resolved,
            self.semantic.resolved_sources,
            self.semantic.typed_hir,
        ) {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        let (expression, moved) = match expression.without_groups() {
            Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
                (unary.operand.without_groups(), true)
            }
            expression => (expression, false),
        };
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
            _ => return Err(BuildError::UnsupportedClaimedExpression),
        };
        Ok(if moved {
            Operand::Move(place)
        } else {
            Operand::Copy(place)
        })
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
        let source = self
            .semantic
            .typed_hir
            .expression(expression.span())
            .ok_or(BuildError::MissingTypedExpression)?
            .id;
        if let Expr::Binary(binary) = expression
            && matches!(
                binary.operator,
                crate::ast::BinaryOperator::LogicalAnd | crate::ast::BinaryOperator::LogicalOr
            )
        {
            return self.lower_short_circuit_to_place(destination, binary, ty, scope, source);
        }
        let value = match expression {
            Expr::Index(index) if super::indexes::view_is_supported(index, self.semantic) => {
                let kind = match self
                    .semantic
                    .typed_hir
                    .index_plan(index.span)
                    .map(|plan| plan.projection)
                {
                    Some(crate::typecheck::TypecheckIndexProjection::Str) => {
                        crate::mir::ViewKind::Str
                    }
                    Some(crate::typecheck::TypecheckIndexProjection::Slice) => {
                        crate::mir::ViewKind::Slice
                    }
                    _ => return Err(BuildError::UnsupportedClaimedExpression),
                };
                let source_ty = known_expression_type(&index.object, self.semantic.typed_hir)
                    .ok_or(BuildError::MissingTypedExpression)?;
                let checked_index_ty = self
                    .semantic
                    .typed_hir
                    .index_plan(index.span)
                    .and_then(|plan| self.semantic.typed_hir.type_id(&plan.index_ty))
                    .ok_or(BuildError::MissingTypedExpression)?;
                let (index_ty, index_scalar) =
                    if scalar_type(checked_index_ty, self.semantic.typed_hir)
                        == Some(ScalarType::Usize)
                    {
                        (checked_index_ty, ScalarType::Usize)
                    } else if matches!(index.index.without_groups(), Expr::IntegerLiteral(_)) {
                        let usize_ty = self
                            .semantic
                            .typed_hir
                            .type_id(&crate::ast::TypeExpr::Reference(
                                crate::ast::TypeReference {
                                    span: index.index.span(),
                                    name: "usize".to_string(),
                                },
                            ))
                            .ok_or(BuildError::MissingTypedExpression)?;
                        (usize_ty, ScalarType::Usize)
                    } else {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    };
                Rvalue::ViewIndex {
                    source: self.lower_view_operand(&index.object, source_ty, kind, scope)?,
                    source_ty,
                    kind,
                    index: self.lower_operand(&index.index, index_ty, index_scalar, scope)?,
                    index_ty,
                    element_ty: ty,
                    element_scalar: scalar,
                }
            }
            Expr::Unary(unary) => Rvalue::Unary {
                operator: match unary.operator {
                    crate::ast::UnaryOperator::Negate => UnaryOperator::Negate,
                    crate::ast::UnaryOperator::LogicalNot => UnaryOperator::LogicalNot,
                    crate::ast::UnaryOperator::Move | crate::ast::UnaryOperator::Spread => {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                },
                operand: self.lower_operand(&unary.operand, ty, scalar, scope)?,
                ty,
            },
            Expr::TypeConversion(conversion) => {
                if let Expr::IntegerLiteral(literal) = conversion.expression.without_groups()
                    && self
                        .semantic
                        .typed_hir
                        .conversion_plan(conversion.span)
                        .is_some_and(|plan| {
                            plan.kind == crate::typecheck::TypecheckConversionKind::LosslessInteger
                        })
                {
                    Rvalue::Use(Operand::Constant(crate::mir::model::Constant {
                        ty,
                        scalar,
                        value: decode_integer_literal_value(&literal.value)
                            .ok_or(BuildError::InvalidScalarConstant)?,
                    }))
                } else {
                    let source_ty =
                        known_expression_type(&conversion.expression, self.semantic.typed_hir)
                            .ok_or(BuildError::MissingTypedExpression)?;
                    let source_scalar = scalar_type(source_ty, self.semantic.typed_hir)
                        .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    Rvalue::Cast {
                        operand: self.lower_operand(
                            &conversion.expression,
                            source_ty,
                            source_scalar,
                            scope,
                        )?,
                        source_ty,
                        source_scalar,
                        target_ty: ty,
                        target_scalar: scalar,
                    }
                }
            }
            Expr::Binary(binary) => {
                if let Some(operator) = mir_binary_operator(binary.operator) {
                    Rvalue::Binary {
                        operator,
                        left: self.lower_operand(&binary.left, ty, scalar, scope)?,
                        right: self.lower_operand(&binary.right, ty, scalar, scope)?,
                        ty,
                    }
                } else {
                    let operator = mir_comparison_operator(binary.operator)
                        .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    let (operand_ty, operand_scalar) =
                        super::coverage::comparison_operand_type(binary, self.semantic.typed_hir)
                            .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    Rvalue::Compare {
                        operator,
                        left: self.lower_operand(
                            &binary.left,
                            operand_ty,
                            operand_scalar,
                            scope,
                        )?,
                        right: self.lower_operand(
                            &binary.right,
                            operand_ty,
                            operand_scalar,
                            scope,
                        )?,
                        operand_ty,
                        operand_scalar,
                        result_ty: ty,
                    }
                }
            }
            Expr::Group(group) => {
                return self.lower_expression_to_place(
                    destination,
                    &group.expression,
                    ty,
                    scalar,
                    scope,
                );
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
                let (callee, arguments, returns_never) = self.lower_call(call, scope)?;
                if returns_never {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                return self.control_flow.emit_returning_call(
                    source,
                    callee,
                    arguments,
                    destination,
                );
            }
            Expr::Force(force) => {
                if let Expr::Call(call) = force.expression.without_groups() {
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
                if let Expr::Call(call) = propagate.expression.without_groups() {
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
            Expr::Member(member) => {
                let (place, field_scalar) =
                    if super::projections::scalar_field_is_supported(member, self.semantic) {
                        let (place, representation) = super::projections::lower_borrow_field_place(
                            member,
                            self.semantic,
                            &self.places_by_symbol,
                            &mut self.projections,
                            &mut self.drop_plans,
                        )?;
                        let crate::mir::ValueRepresentation::Scalar(field_scalar) = representation
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
            Expr::Index(index) => {
                let (place, representation) = super::indexes::lower_place(self, index, scope)?;
                if representation != crate::mir::ValueRepresentation::Scalar(scalar) {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
                Rvalue::Use(Operand::Copy(place))
            }
            _ => Rvalue::Use(self.lower_simple_operand(expression, ty, scalar)?),
        };
        self.control_flow.push_statement(Statement::Assign {
            destination: Place::local(destination),
            value,
            origin: crate::mir::Origin::Expression(source),
        })?;
        Ok(())
    }

    pub(super) fn lower_view_expression_to_place(
        &mut self,
        destination: LocalId,
        expression: &Expr,
        ty: crate::semantic::TyId,
        kind: crate::mir::ViewKind,
        scope: ScopeId,
    ) -> Result<(), BuildError> {
        let source = self
            .semantic
            .typed_hir
            .expression(expression.span())
            .ok_or(BuildError::MissingTypedExpression)?
            .id;
        let source_ty =
            super::coverage::intrinsic_expression_type(expression.span(), self.semantic.typed_hir)
                .filter(|source_ty| {
                    super::coverage::value_representation(*source_ty, self.semantic)
                        == Some(crate::mir::ValueRepresentation::View(kind))
                })
                .unwrap_or(ty);
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
        match expression {
            Expr::StringLiteral(literal) if kind == crate::mir::ViewKind::Str => {
                let bytes = crate::literals::decode_string_literal_bytes(&literal.value)
                    .map_err(|_| BuildError::InvalidScalarConstant)?;
                Ok(Operand::StaticStr { ty, bytes })
            }
            Expr::Identifier(_) => self.lower_stored_identifier(expression),
            Expr::Member(member) if kind == crate::mir::ViewKind::Str => {
                super::projections::lower_error_field_place(
                    member,
                    self.semantic,
                    &self.places_by_symbol,
                    &mut self.projections,
                )
                .map(Operand::Copy)
            }
            Expr::Group(group) => self.lower_view_operand(&group.expression, ty, kind, scope),
            Expr::Call(_) => {
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
        let left = self.lower_operand(&binary.left, ty, ScalarType::Bool, scope)?;
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
        self.lower_expression_to_place(destination, &binary.right, ty, ScalarType::Bool, scope)?;
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
                .is_some_and(|place| place.projection.is_some()),
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
        &self,
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
                Ok(Operand::Copy(
                    *self
                        .places_by_symbol
                        .get(&symbol)
                        .ok_or(BuildError::MissingLocalSymbol)?,
                ))
            }
            _ => Err(BuildError::UnsupportedClaimedExpression),
        }
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
