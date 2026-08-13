//! Scalar expression evaluation into MIR places, rvalues, and operands.

use super::BuildError;
use super::context::LoweringContext;
use super::coverage::{known_expression_type, scalar_type};
use crate::ast::Expr;
use crate::literals::decode_integer_literal_value;
use crate::mir::{
    BinaryOperator, CallArgument, ComparisonOperator, LocalId, LocalOrigin, LocalStorage, Operand,
    Place, Rvalue, ScalarType, ScopeId, Statement, UnaryOperator,
};

mod control_flow_expressions;
mod outcomes;
pub(super) use control_flow_expressions::lower_conditional_to_place;

impl LoweringContext<'_> {
    pub(super) fn lower_call(
        &mut self,
        call: &crate::ast::CallExpr,
        scope: ScopeId,
    ) -> Result<(crate::semantic::DefId, Vec<CallArgument>, bool), BuildError> {
        let Expr::Identifier(callee) = call.callee.without_groups() else {
            return Err(BuildError::UnsupportedClaimedExpression);
        };
        let returns_never = self
            .semantic
            .typed_hir
            .expression(call.span)
            .is_some_and(|expression| expression.diverges);
        let callee = self
            .semantic
            .typed_hir
            .function_call_target(callee.span)
            .map(|definition| {
                self.semantic
                    .resolved
                    .callable_bodies
                    .canonical_definition(definition)
            })
            .ok_or(BuildError::MissingCallTarget)?;
        let arguments = call
            .arguments
            .iter()
            .map(|argument| {
                let ty = known_expression_type(argument, self.semantic.typed_hir)
                    .ok_or(BuildError::MissingTypedExpression)?;
                if let Some(scalar) = scalar_type(ty, self.semantic.typed_hir) {
                    let operand = self.lower_operand(argument, ty, scalar, scope)?;
                    return Ok(CallArgument {
                        operand,
                        ty,
                        representation: crate::mir::ValueRepresentation::Scalar(scalar),
                    });
                }
                if super::coverage::borrow_identifier_is_supported(
                    argument,
                    self.semantic.resolved,
                    self.semantic.typed_hir,
                ) {
                    return Ok(CallArgument {
                        operand: self.lower_stored_identifier(argument)?,
                        ty,
                        representation: crate::mir::ValueRepresentation::Borrow,
                    });
                }
                let operand = self.lower_copy_aggregate_identifier(argument)?;
                Ok(CallArgument {
                    operand,
                    ty,
                    representation: crate::mir::ValueRepresentation::Aggregate,
                })
            })
            .collect::<Result<Vec<_>, BuildError>>()?;
        Ok((callee, arguments, returns_never))
    }

    fn lower_copy_aggregate_identifier(&self, expression: &Expr) -> Result<Operand, BuildError> {
        if !super::coverage::copy_aggregate_identifier_is_supported(
            expression,
            self.semantic.resolved,
            self.semantic.resolved_sources,
            self.semantic.typed_hir,
        ) {
            return Err(BuildError::UnsupportedClaimedExpression);
        }
        self.lower_stored_identifier(expression)
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
        let local = *self
            .locals_by_symbol
            .get(&symbol.id)
            .ok_or(BuildError::MissingLocalSymbol)?;
        Ok(Operand::Copy(Place::local(local)))
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
                    let operand_ty = known_expression_type(&binary.left, self.semantic.typed_hir)
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let operand_scalar = scalar_type(operand_ty, self.semantic.typed_hir)
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
                return lower_conditional_to_place(self, destination, if_, ty, scalar, scope);
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
                return outcomes::lower_discard_catch_to_place(
                    self,
                    destination,
                    catch,
                    ty,
                    scalar,
                    scope,
                );
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
                let Expr::Call(call) = force.expression.without_groups() else {
                    return Err(BuildError::UnsupportedClaimedExpression);
                };
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
            Expr::Propagate(propagate) => {
                let Expr::Call(call) = propagate.expression.without_groups() else {
                    return Err(BuildError::UnsupportedClaimedExpression);
                };
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
            Expr::Member(member) => {
                let (place, field_scalar) = super::projections::lower_scalar_field_place(
                    member,
                    self.semantic,
                    &self.locals_by_symbol,
                    &mut self.projections,
                    &mut self.drop_plans,
                )?;
                if field_scalar != scalar {
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
        if !matches!(
            expression,
            Expr::Unary(_)
                | Expr::TypeConversion(_)
                | Expr::Binary(_)
                | Expr::Call(_)
                | Expr::Force(_)
                | Expr::Propagate(_)
                | Expr::Member(_)
                | Expr::If(_)
                | Expr::Otherwise(_)
                | Expr::Catch(_)
        ) {
            return match expression {
                Expr::Group(group) => self.lower_operand(&group.expression, ty, scalar, scope),
                _ => self.lower_simple_operand(expression, ty, scalar),
            };
        }

        let typed_expression = self
            .semantic
            .typed_hir
            .expression(expression.span())
            .ok_or(BuildError::MissingTypedExpression)?;
        let temporary = LocalId::from_index(self.locals.len());
        self.locals.push(crate::mir::locals::Local::scalar(
            ty,
            scalar,
            LocalStorage::Local,
            LocalOrigin::Temporary(typed_expression.id),
            scope,
        ));
        self.lower_expression_to_place(temporary, expression, ty, scalar, scope)?;
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
                Ok(Operand::Copy(Place::local(
                    *self
                        .locals_by_symbol
                        .get(&symbol)
                        .ok_or(BuildError::MissingLocalSymbol)?,
                )))
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
